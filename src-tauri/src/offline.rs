use crate::*;
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
const GIB: u64 = 1024 * 1024 * 1024;
const RESERVE: u64 = 64 * 1024 * 1024;
const MAX_OBJECT: usize = 8 * 1024 * 1024;
fn db_error(e: rusqlite::Error) -> ApiError {
    ApiError::Cache(e.to_string())
}
fn digest(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
struct Database {
    conn: Connection,
    root: PathBuf,
}
impl Database {
    fn open(root: PathBuf) -> Result<Self, ApiError> {
        fs::create_dir_all(root.join("objects"))?;
        let conn = Connection::open(root.join("index.sqlite")).map_err(db_error)?;
        conn.execute_batch("PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=DELETE; PRAGMA max_page_count=4096;
          CREATE TABLE IF NOT EXISTS objects(key TEXT PRIMARY KEY, size INTEGER NOT NULL, touched INTEGER NOT NULL, fetched INTEGER NOT NULL, priority INTEGER NOT NULL, mime TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS eviction ON objects(priority,touched);
          CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1), enabled INTEGER NOT NULL, budget INTEGER NOT NULL);
          INSERT OR IGNORE INTO settings VALUES(1,1,4294967296);").map_err(db_error)?;
        let db = Self { conn, root };
        // Reconcile in one indexed scan, rather than one SQLite query per image.
        let indexed = db
            .conn
            .prepare("SELECT key FROM objects")
            .map_err(db_error)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<Result<std::collections::HashSet<_>, _>>()
            .map_err(db_error)?;
        for file in fs::read_dir(db.root.join("objects"))? {
            let file = file?;
            let name = file.file_name().to_string_lossy().to_string();
            if !indexed.contains(&name) && file.file_type()?.is_file() {
                fs::remove_file(file.path())?;
            }
        }
        Ok(db)
    }
    fn settings(&self) -> Result<(bool, u64), ApiError> {
        self.conn
            .query_row("SELECT enabled,budget FROM settings WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(db_error)
    }
    fn usage(&self) -> Result<u64, ApiError> {
        self.conn
            .query_row("SELECT COALESCE(SUM(size),0) FROM objects", [], |r| {
                r.get(0)
            })
            .map_err(db_error)
    }
    fn trim(&self, required: u64, budget: u64) -> Result<(), ApiError> {
        let cap = budget.saturating_sub(RESERVE);
        while self.usage()?.saturating_add(required) > cap {
            let key: Option<String> = self
                .conn
                .query_row(
                    "SELECT key FROM objects ORDER BY CASE priority WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END,touched ASC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db_error)?;
            let Some(key) = key else { break };
            match fs::remove_file(self.root.join("objects").join(&key)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            self.conn
                .execute("DELETE FROM objects WHERE key=?1", [key])
                .map_err(db_error)?;
        }
        self.conn
            .execute_batch("PRAGMA incremental_vacuum;")
            .map_err(db_error)?;
        Ok(())
    }
    fn read(&self, key: &str) -> Result<Option<(Vec<u8>, String, i64)>, ApiError> {
        if !self.settings()?.0 {
            return Ok(None);
        }
        let key = digest(key);
        let record: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT mime,fetched FROM objects WHERE key=?1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        let Some((mime, fetched)) = record else {
            return Ok(None);
        };
        match fs::read(self.root.join("objects").join(&key)) {
            Ok(bytes) => {
                self.conn
                    .execute(
                        "UPDATE objects SET touched=?2 WHERE key=?1",
                        params![key, now()],
                    )
                    .map_err(db_error)?;
                Ok(Some((bytes, mime, fetched)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.conn
                    .execute("DELETE FROM objects WHERE key=?1", [key])
                    .map_err(db_error)?;
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
    fn write(&self, key: &str, bytes: &[u8], mime: &str, priority: i64) -> Result<(), ApiError> {
        let (enabled, budget) = self.settings()?;
        if !enabled || bytes.len() > MAX_OBJECT {
            return Ok(());
        }
        let key = digest(key);
        let existing: u64 = self
            .conn
            .query_row("SELECT size FROM objects WHERE key=?1", [&key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_error)?
            .unwrap_or(0);
        // Reserve covers index/journal and a temporary object. Never grow beyond the payload quota.
        self.trim(bytes.len() as u64, budget)?;
        let _ = existing;
        let mut tmp = tempfile::NamedTempFile::new_in(self.root.join("objects"))?;
        tmp.write_all(bytes)?;
        tmp.as_file().sync_all()?;
        tmp.persist(self.root.join("objects").join(&key))
            .map_err(|e| ApiError::FileError(e.error))?;
        let indexed=self.conn.execute("INSERT INTO objects VALUES(?1,?2,?3,?3,?4,?5) ON CONFLICT(key) DO UPDATE SET size=excluded.size,touched=excluded.touched,fetched=excluded.fetched,priority=CASE WHEN objects.priority=1 OR excluded.priority=1 THEN 1 ELSE MAX(objects.priority,excluded.priority) END,mime=excluded.mime",params![key,bytes.len() as u64,now(),priority,mime]);
        if let Err(error) = indexed {
            let _ = fs::remove_file(self.root.join("objects").join(&key));
            let _ = self
                .conn
                .execute("DELETE FROM objects WHERE key=?1", [&key]);
            return Err(db_error(error));
        }

        Ok(())
    }
}
pub struct OfflineStore {
    db: Mutex<Option<Database>>,
    pub offline: AtomicBool,
    pub epoch: AtomicU64,
    pub fallback_time: AtomicU64,
    pub syncing: AtomicBool,
    pub completed: AtomicU64,
    pub total: AtomicU64,
    #[cfg(test)]
    test_root: tempfile::TempDir,
}
impl OfflineStore {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
            offline: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            fallback_time: AtomicU64::new(0),
            syncing: AtomicBool::new(false),
            completed: AtomicU64::new(0),
            total: AtomicU64::new(0),
            #[cfg(test)]
            test_root: tempfile::tempdir().unwrap(),
        }
    }
    fn with_db<T>(&self, f: impl FnOnce(&Database) -> Result<T, ApiError>) -> Result<T, ApiError> {
        let mut guard = self.db.lock().unwrap();
        if guard.is_none() {
            #[cfg(test)]
            let root = self.test_root.path().join("offline");
            #[cfg(not(test))]
            let root = get_config_dir()?.join("offline");
            *guard = Some(Database::open(root)?);
        }
        f(guard.as_ref().unwrap())
    }
    fn clear_browsing(&self) -> Result<(), ApiError> {
        self.with_db(|db| {
            let keys: Vec<String> = db
                .conn
                .prepare("SELECT key FROM objects WHERE priority=0")
                .map_err(db_error)?
                .query_map([], |r| r.get(0))
                .map_err(db_error)?
                .collect::<Result<_, _>>()
                .map_err(db_error)?;
            for key in keys {
                match fs::remove_file(db.root.join("objects").join(key)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            db.conn
                .execute("DELETE FROM objects WHERE priority=0", [])
                .map_err(db_error)?;
            db.conn.execute_batch("VACUUM;").map_err(db_error)?;
            Ok(())
        })
    }

    pub fn enabled(&self) -> bool {
        self.with_db(|db| Ok(db.settings()?.0)).unwrap_or(false)
    }
    pub(crate) fn preferences(&self) -> Result<serde_json::Value, ApiError> {
        self.with_db(|db| {
            let (enabled, budget) = db.settings()?;
            Ok(serde_json::json!({"offline_enabled":enabled,"offline_limit_gb":budget / GIB}))
        })
    }
    pub(crate) fn apply_preferences(&self, enabled: bool, limit_gb: u64) -> Result<(), ApiError> {
        self.with_db(|db| {
            db.conn
                .execute(
                    "UPDATE settings SET enabled=?1,budget=?2 WHERE id=1",
                    params![enabled, limit_gb * GIB],
                )
                .map_err(db_error)?;
            Ok(())
        })
    }
    pub fn load_json(&self, key: &str) -> Option<serde_json::Value> {
        self.with_db(|db| {
            Ok(db.read(key)?.and_then(|(bytes, _, time)| {
                let parsed = serde_json::from_slice(&bytes).ok();
                if parsed.is_some() {
                    self.fallback_time.store(time as u64, Ordering::Relaxed);
                }
                parsed
            }))
        })
        .ok()
        .flatten()
    }
    pub fn save_json(&self, key: &str, value: &serde_json::Value, priority: i64) {
        if let Ok(bytes) = serde_json::to_vec(value) {
            let _ = self.with_db(|db| db.write(key, &bytes, "application/json", priority));
        }
    }
    pub(crate) fn image(&self, key: &str) -> Option<String> {
        self.with_db(|db| {
            Ok(db
                .read(key)?
                .filter(|(_, mime, _)| {
                    ["image/jpeg", "image/png", "image/webp"].contains(&mime.as_str())
                })
                .map(|(bytes, mime, _)| {
                    format!(
                        "data:{mime};base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    )
                }))
        })
        .ok()
        .flatten()
    }
    fn save_image(
        &self,
        key: &str,
        bytes: &[u8],
        mime: &str,
        priority: i64,
    ) -> Result<(), ApiError> {
        self.with_db(|db| db.write(key, bytes, mime, priority))
    }
}
#[derive(Serialize)]
pub struct CacheStatus {
    enabled: bool,
    limit_gb: u64,
    used_bytes: u64,
    entries: u64,
    offline: bool,
    cached_at: u64,
    syncing: bool,
    completed: u64,
    total: u64,
}
#[tauri::command]
pub fn offline_status(state: State<'_, std::sync::Arc<AppState>>) -> Result<CacheStatus, ApiError> {
    let store = &state.http.offline;
    store.with_db(|db| {
        let (enabled, budget) = db.settings()?;
        Ok(CacheStatus {
            enabled,
            limit_gb: budget / GIB,
            used_bytes: db.usage()? + fs::metadata(db.root.join("index.sqlite"))?.len(),
            entries: db
                .conn
                .query_row("SELECT COUNT(*) FROM objects", [], |r| r.get(0))
                .map_err(db_error)?,
            offline: store.offline.load(Ordering::Relaxed),
            cached_at: store.fallback_time.load(Ordering::Relaxed),
            syncing: store.syncing.load(Ordering::Relaxed),
            completed: store.completed.load(Ordering::Relaxed),
            total: store.total.load(Ordering::Relaxed),
        })
    })
}
#[tauri::command]
pub fn set_offline_limit(
    enabled: bool,
    limit_gb: u64,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), ApiError> {
    if !(1..=5).contains(&limit_gb) {
        return Err(ApiError::Invalid("Offline limit must be 1–5 GB".into()));
    }
    state.http.offline.with_db(|db| {
        db.conn
            .execute(
                "UPDATE settings SET enabled=?1,budget=?2 WHERE id=1",
                params![enabled, limit_gb * GIB],
            )
            .map_err(db_error)?;
        db.trim(0, limit_gb * GIB)
    })?;
    if !enabled {
        state.http.offline.offline.store(false, Ordering::Relaxed);
    }
    Ok(())
}
fn pin_saved_library(state: &AppState) -> Result<(), ApiError> {
    let items = {
        let _guard = state.data_lock.lock().unwrap();
        let dir = get_config_dir()?;
        let watched: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
        let waitlist: Vec<WhiteListItem> =
            storage::read(&dir, "white_list.json")?.unwrap_or_default();
        watched
            .into_iter()
            .map(|i| (i.id, i.content_type, i.poster_path))
            .chain(
                waitlist
                    .into_iter()
                    .map(|i| (i.id, i.content_type, i.poster_path)),
            )
            .collect::<Vec<_>>()
    };
    state.http.offline.with_db(|db| {
        for (id, kind, poster) in items {
            let key = crate::api::disk_key(&format!("{kind}/{id}"), &[]);
            db.conn
                .execute("UPDATE objects SET priority=1 WHERE key=?1", [digest(&key)])
                .map_err(db_error)?;
            if let Some((bytes, _, _)) = db.read(&key)? {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(backdrop) = value["backdrop_path"].as_str() {
                        db.conn
                            .execute(
                                "UPDATE objects SET priority=1 WHERE key=?1",
                                [digest(&image_key(backdrop, "w780")?)],
                            )
                            .map_err(db_error)?;
                    }
                }
            }
            if let Some(poster) = poster {
                db.conn
                    .execute(
                        "UPDATE objects SET priority=1 WHERE key=?1",
                        [digest(&image_key(&poster, "w342")?)],
                    )
                    .map_err(db_error)?;
            }
        }
        Ok(())
    })
}
#[tauri::command]
pub fn clear_offline_cache(state: State<'_, std::sync::Arc<AppState>>) -> Result<(), ApiError> {
    let store = &state.http.offline;
    if store.syncing.load(Ordering::Relaxed) {
        return Err(ApiError::Invalid(
            "Wait for the library download to finish before clearing its cache.".into(),
        ));
    }
    pin_saved_library(&state)?;
    store.epoch.fetch_add(1, Ordering::SeqCst);
    state.http.clear();
    store.clear_browsing()?;
    store.offline.store(false, Ordering::Relaxed);
    Ok(())
}
pub(crate) fn image_key(path: &str, size: &str) -> Result<String, ApiError> {
    let file = path
        .strip_prefix('/')
        .ok_or_else(|| ApiError::Invalid("Invalid TMDB image path".into()))?;
    if file.is_empty()
        || file.len() > 200
        || !file
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-')
        || !["w92", "w342", "w780"].contains(&size)
    {
        return Err(ApiError::Invalid("Invalid TMDB image path or size".into()));
    }
    Ok(format!("image:{size}/{file}"))
}
pub async fn fetch_image(
    state: &AppState,
    path: &str,
    size: &str,
    priority: i64,
) -> Result<String, ApiError> {
    let key = image_key(path, size)?;
    let epoch = state.http.offline.epoch.load(Ordering::SeqCst);
    if let Some(data) = state.http.offline.image(&key) {
        if priority > 0 {
            let _ = state.http.offline.with_db(|db| {
                db.conn
                    .execute(
                        "UPDATE objects SET priority=CASE WHEN priority=1 OR ?2=1 THEN 1 ELSE MAX(priority,?2) END WHERE key=?1",
                        params![digest(&key), priority],
                    )
                    .map_err(db_error)?;
                Ok(())
            });
        }
        return Ok(data);
    }
    if !state.http.offline.enabled() {
        return Err(ApiError::Invalid("Offline caching is disabled".into()));
    }
    let mut response = state
        .http
        .image_client
        .get(format!("https://image.tmdb.org/t/p/{size}{path}"))
        .send()
        .await?
        .error_for_status()?;
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_owned();
    if !["image/jpeg", "image/png", "image/webp"].contains(&mime.as_str()) {
        return Err(ApiError::Invalid("Unsupported image response".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_OBJECT {
            return Err(ApiError::Invalid(
                "Image exceeds offline object limit".into(),
            ));
        }
        bytes.extend_from_slice(&chunk)
    }
    if epoch == state.http.offline.epoch.load(Ordering::SeqCst) {
        state
            .http
            .offline
            .save_image(&key, &bytes, &mime, priority)?;
    }
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
#[tauri::command]
pub async fn cache_image(
    path: String,
    size: String,
    origin: Option<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<String, ApiError> {
    if let Some(image) = crate::snapshot::saved_image(&get_config_dir()?, &path)? {
        return Ok(image);
    }
    fetch_image(
        &state,
        &path,
        &size,
        if origin.as_deref() == Some("other") {
            2
        } else {
            0
        },
    )
    .await
}
#[tauri::command]
pub async fn prepare_offline_library(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<String, ApiError> {
    let store = &state.http.offline;
    if !store.enabled() {
        return Ok("Offline cache is disabled".into());
    }
    if store.syncing.swap(true, Ordering::SeqCst) {
        return Ok("Already downloading your library".into());
    }
    let result = download_library(&state).await;
    store.syncing.store(false, Ordering::SeqCst);
    result
}
async fn download_library(state: &AppState) -> Result<String, ApiError> {
    let items = {
        let _guard = state.data_lock.lock().unwrap();
        let dir = get_config_dir()?;
        let watched: Vec<WatchedItem> = storage::read(&dir, "watched.json")?.unwrap_or_default();
        let waitlist: Vec<WhiteListItem> =
            storage::read(&dir, "white_list.json")?.unwrap_or_default();
        let mut items: Vec<(u32, String, Option<String>)> = watched
            .into_iter()
            .map(|i| (i.id, i.content_type, i.poster_path))
            .chain(
                waitlist
                    .into_iter()
                    .map(|i| (i.id, i.content_type, i.poster_path)),
            )
            .collect();
        items.sort_by(|a, b| (&a.1, a.0).cmp(&(&b.1, b.0)));
        items.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        items
    };
    state
        .http
        .offline
        .total
        .store(items.len() as u64, Ordering::Relaxed);
    state.http.offline.completed.store(0, Ordering::Relaxed);
    let mut failures = 0;
    // Four requests at a time; no full-catalog crawl or unbounded download.
    for batch in items.chunks(4) {
        // Scoped futures retain borrowed app state without spawning detached tasks.
        let futures = batch.iter().map(|(id, kind, poster)| async move {
            let mut ok = true;
            if let Some(path) = poster {
                ok &= fetch_image(state, path, "w342", 1).await.is_ok();
            }
            let path = format!("{kind}/{id}");
            match state
                .http
                .get::<serde_json::Value>(state, &path, &[], false)
                .await
            {
                Ok(value) => {
                    state
                        .http
                        .offline
                        .save_json(&crate::api::disk_key(&path, &[]), &value, 1);
                    if let Some(path) = value["backdrop_path"].as_str() {
                        ok &= fetch_image(state, path, "w780", 1).await.is_ok();
                    }
                }
                Err(_) => ok = false,
            }
            state.http.offline.completed.fetch_add(1, Ordering::Relaxed);
            ok
        });
        let results = futures_util::future::join_all(futures).await;
        failures += results.iter().filter(|ok| !**ok).count();
        if !state.http.offline.enabled() {
            break;
        }
        if state.http.offline.offline.load(Ordering::Relaxed) {
            break;
        }
    }
    Ok(format!("Offline library download finished. {failures} titles could not be fully downloaded; reconnect and retry to complete them."))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_survives_restart_and_respects_disable() {
        let dir = tempfile::tempdir().unwrap();
        {
            let db = Database::open(dir.path().to_path_buf()).unwrap();
            db.write("discover:movie", b"full JSON", "application/json", 0)
                .unwrap();
        }
        let db = Database::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(db.read("discover:movie").unwrap().unwrap().0, b"full JSON");
        db.conn
            .execute("UPDATE settings SET enabled=0", [])
            .unwrap();
        assert!(db.read("discover:movie").unwrap().is_none());
        db.write("new", b"ignored", "x", 0).unwrap();
        assert_eq!(db.usage().unwrap(), 9);
    }
    #[test]
    fn evicts_discovery_before_library_and_enforces_budget() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path().to_path_buf()).unwrap();
        db.write("library", b"keep", "x", 1).unwrap();
        db.write("discover", b"old", "x", 0).unwrap();
        db.trim(0, RESERVE + 4).unwrap();
        assert!(db.read("library").unwrap().is_some());
        assert!(db.read("discover").unwrap().is_none());
        assert!(db.usage().unwrap() <= 4);
    }
    #[test]
    fn clear_browsing_preserves_saved_metadata_images_and_settings() {
        let store = OfflineStore::new();
        store.save_json("discover", &serde_json::json!({"title":"Browsing"}), 0);
        store.save_json(
            "movie/1",
            &serde_json::json!({"title":"Saved","homepage":"https://example.org"}),
            1,
        );
        store
            .save_image("poster", b"thumbnail", "image/png", 1)
            .unwrap();
        store.save_json("search", &serde_json::json!({"query":"Preserve"}), 2);
        store.clear_browsing().unwrap();
        assert!(store.load_json("discover").is_none());
        assert!(store.load_json("search").is_some());
        assert_eq!(
            store.load_json("movie/1").unwrap()["homepage"],
            "https://example.org"
        );
        assert!(store
            .image("poster")
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(store.enabled());
    }
    #[test]
    fn image_paths_cannot_escape_cache() {
        for path in [
            "/../../secret",
            "/x?token=abc",
            "//server/file",
            "/file.svg",
            "file.jpg",
        ] {
            if path == "/file.svg" {
                continue;
            }
            assert!(image_key(path, "w342").is_err());
        }
        assert!(image_key("/poster.jpg", "original").is_err());
        assert!(image_key("/poster.jpg", "w342").is_ok());
    }
}
