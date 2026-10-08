use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const MAX_TOTAL: u64 = 512 * 1024 * 1024;
const MAX_JSON: u64 = 32 * 1024 * 1024;
const MAX_IMAGE: u64 = 8 * 1024 * 1024;
const DATA_FILES: [&str; 5] = [
    "watched.json",
    "white_list.json",
    "ai_workspace.json",
    "snapshot_preferences.json",
    "config.json",
];
fn invalid(s: impl Into<String>) -> ApiError {
    ApiError::Invalid(s.into())
}
fn zip_error(e: zip::result::ZipError) -> ApiError {
    invalid(format!("Snapshot ZIP: {e}"))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Serialize, Deserialize, Clone)]
struct FileInfo {
    sha256: String,
    bytes: u64,
}
#[derive(Serialize, Deserialize, Clone)]
struct Thumbnail {
    path: String,
    file: String,
    mime: String,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    app_version: String,
    created_at: String,
    files: BTreeMap<String, FileInfo>,
    thumbnails: Vec<Thumbnail>,
    missing_thumbnails: Vec<String>,
}
struct Snapshot {
    manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
    data: Vec<Value>,
    digest: String,
}
fn valid_image(bytes: &[u8], mime: &str) -> bool {
    match mime {
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}
fn validate_data(data: &[Value]) -> Result<(), ApiError> {
    let watched: Vec<WatchedItem> = serde_json::from_value(data[0].clone())?;
    let waitlist: Vec<WhiteListItem> = serde_json::from_value(data[1].clone())?;
    let ws: AiWorkspace = serde_json::from_value(data[2].clone())?;
    let mut ids = BTreeSet::new();
    for (kind, id) in watched.iter().map(|i| (&i.content_type, i.id)) {
        validate_type(kind)?;
        if id == 0 || !ids.insert((kind, id)) {
            return Err(invalid("Duplicate or invalid watched title"));
        }
    }
    ids.clear();
    for (kind, id) in waitlist.iter().map(|i| (&i.content_type, i.id)) {
        validate_type(kind)?;
        if id == 0 || !ids.insert((kind, id)) {
            return Err(invalid("Duplicate or invalid waitlist title"));
        }
    }
    let mut proposal_ids = BTreeSet::new();
    for p in &ws.proposals {
        validate_type(&p.item.content_type)?;
        if p.item.id == 0 || p.proposal_id.is_empty() || !proposal_ids.insert(&p.proposal_id) {
            return Err(invalid("Invalid pending proposal"));
        }
    }
    let mut suggestion_ids = BTreeSet::new();
    for s in &ws.suggestions {
        validate_type(s.item["content_type"].as_str().unwrap_or(""))?;
        if !s.item["id"]
            .as_u64()
            .is_some_and(|id| id > 0 && id <= u32::MAX as u64)
            || !["pending", "approved"].contains(&s.status.as_str())
            || s.suggestion_id.is_empty()
            || !suggestion_ids.insert(&s.suggestion_id)
        {
            return Err(invalid("Invalid suggestion record"));
        }
        let mut item = s.item.clone();
        item["white_list_date"] = json!("");
        let _: WhiteListItem = serde_json::from_value(item)?;
    }
    if !data[3]["zoom"]
        .as_u64()
        .is_some_and(|n| (75..=175).contains(&n))
        || !data[3]["offline_enabled"].is_boolean()
        || !data[3]["offline_limit_gb"]
            .as_u64()
            .is_some_and(|n| (1..=5).contains(&n))
    {
        return Err(invalid("Invalid snapshot preferences"));
    }
    let config = data[4]
        .as_object()
        .ok_or_else(|| invalid("Invalid snapshot connection settings"))?;
    if config.keys().any(|k| k != "api_key")
        || config.get("api_key").is_some_and(|k| {
            !k.as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 512)
        })
    {
        return Err(invalid("Invalid TMDB connection settings"));
    }
    Ok(())
}
fn read_data(dir: &Path, preferences: Value) -> Result<Vec<Value>, ApiError> {
    let data = vec![
        storage::read(dir, "watched.json")?.unwrap_or(json!([])),
        storage::read(dir, "white_list.json")?.unwrap_or(json!([])),
        storage::read(dir, "ai_workspace.json")?
            .unwrap_or(serde_json::to_value(AiWorkspace::default())?),
        preferences,
        json!({}),
    ];
    validate_data(&data)?;
    Ok(data)
}
fn poster_paths(data: &[Value]) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for item in data[0]
        .as_array()
        .into_iter()
        .flatten()
        .chain(data[1].as_array().into_iter().flatten())
        .chain(
            data[2]["proposals"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| &p["item"]),
        )
        .chain(
            data[2]["suggestions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| &s["item"]),
        )
    {
        if let Some(path) = item["poster_path"].as_str().filter(|s| !s.is_empty()) {
            paths.insert(path.into());
        }
    }
    paths
}
pub(crate) fn saved_image(dir: &Path, path: &str) -> Result<Option<String>, ApiError> {
    offline::image_key(path, "w342")?;
    let index: BTreeMap<String, Thumbnail> =
        storage::read(&dir.join("snapshot_images"), "index.json")?.unwrap_or_default();
    let Some(image) = index.get(path) else {
        return Ok(None);
    };
    let filename = Path::new(&image.file)
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| invalid("Invalid saved image"))?;
    if !safe_thumbnail(&image.file) {
        return Err(invalid("Invalid saved image filename"));
    }
    let bytes = fs::read(dir.join("snapshot_images").join(filename))?;
    if bytes.len() as u64 > MAX_IMAGE || !valid_image(&bytes, &image.mime) {
        return Err(invalid("Invalid saved thumbnail"));
    }
    Ok(Some(format!(
        "data:{};base64,{}",
        image.mime,
        STANDARD.encode(bytes)
    )))
}
fn safe_thumbnail(name: &str) -> bool {
    let Some(file) = name.strip_prefix("thumbnails/") else {
        return false;
    };
    let Some((digest, ext)) = file.rsplit_once('.') else {
        return false;
    };
    digest.len() == 64
        && digest.bytes().all(|b| b.is_ascii_hexdigit())
        && ["jpg", "png", "webp"].contains(&ext)
}
async fn thumbnail(
    state: &AppState,
    dir: &Path,
    path: &str,
) -> Result<(Vec<u8>, String), ApiError> {
    let key = offline::image_key(path, "w342")?;
    if let Some(data) = saved_image(dir, path)?.or_else(|| state.http.offline.image(&key)) {
        let (prefix, encoded) = data
            .split_once(",")
            .ok_or_else(|| invalid("Invalid image data"))?;
        let mime = prefix
            .strip_prefix("data:")
            .and_then(|s| s.strip_suffix(";base64"))
            .ok_or_else(|| invalid("Invalid image data"))?
            .to_owned();
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| invalid("Invalid image data"))?;
        if valid_image(&bytes, &mime) {
            return Ok((bytes, mime));
        }
    }
    let mut response = state
        .http
        .image_client
        .get(format!("https://image.tmdb.org/t/p/w342{path}"))
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
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_IMAGE as usize {
            return Err(invalid("Thumbnail is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !valid_image(&bytes, &mime) {
        return Err(invalid("Unsupported thumbnail"));
    }
    Ok((bytes, mime))
}
fn write_archive(
    path: &Path,
    data: &[Value],
    images: Vec<(String, Vec<u8>, String)>,
    missing: Vec<String>,
) -> Result<Value, ApiError> {
    let mut manifest = Manifest {
        format: "movinight-snapshot".into(),
        version: 1,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        files: BTreeMap::new(),
        thumbnails: vec![],
        missing_thumbnails: missing,
    };
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Choose a snapshot destination"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut writer = ZipWriter::new(temp.as_file_mut());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut total = 0u64;
    let mut add = |name: &str, bytes: &[u8]| -> Result<(), ApiError> {
        if name.starts_with("data/") && bytes.len() as u64 > MAX_JSON {
            return Err(invalid("Snapshot data file exceeds 32 MB"));
        }
        total += bytes.len() as u64;
        if total > MAX_TOTAL {
            return Err(invalid("Snapshot exceeds the 512 MB transfer limit"));
        }
        writer.start_file(name, options).map_err(zip_error)?;
        writer.write_all(bytes)?;
        manifest.files.insert(
            name.into(),
            FileInfo {
                sha256: hash(bytes),
                bytes: bytes.len() as u64,
            },
        );
        Ok(())
    };
    for (i, name) in DATA_FILES.iter().enumerate() {
        add(
            &format!("data/{name}"),
            &serde_json::to_vec_pretty(&data[i])?,
        )?;
    }
    for (path, bytes, mime) in images {
        let ext = match mime.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            _ => "webp",
        };
        let file = format!("thumbnails/{}.{}", hash(path.as_bytes()), ext);
        add(&file, &bytes)?;
        manifest.thumbnails.push(Thumbnail { path, file, mime });
    }
    add("README.txt",b"MoviNight portable snapshot, format version 1. Read manifest.json and data/*.json in any ZIP/JSON-capable tool. IDs use TMDB and content_type distinguishes movies from TV. Thumbnails map TMDB poster paths to local files. No browsing cache, MCP tokens or device paths are included. data/config.json may contain a TMDB API key if the exporter opted in; treat that ZIP as private. Import through MoviNight Settings > Data transfer; other apps need to support this format or convert the JSON. See SNAPSHOT_FORMAT.md in the project repository.")?;
    writer
        .start_file("manifest.json", options)
        .map_err(zip_error)?;
    writer.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    writer.finish().map_err(zip_error)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(
        json!({"path":path,"thumbnails":manifest.thumbnails.len(),"missing_thumbnails":manifest.missing_thumbnails.len(),"counts":counts(data)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<Value> {
        let item = json!({"id":42,"content_type":"tv","title":"Series","overview":"Plot","poster_path":"/poster.jpg","release_date":"2000-01-01","vote_average":8,"watched_date":"2020-01-02","watched_seasons":[1,2],"total_seasons_known":3,"personal_note":"Keep me"});
        let mut waitlist = item.clone();
        waitlist["id"] = json!(43);
        waitlist["content_type"] = json!("movie");
        waitlist.as_object_mut().unwrap().remove("watched_date");
        waitlist["white_list_date"] = json!("2020-02-03");
        let research = json!({"id":"batch","text":"Original table","instructions":"Match precisely","updated_at":"2020-01-01","agent_note":"Research done"});
        let mut proposal_item = waitlist.clone();
        proposal_item["id"] = json!(44);
        let ws = json!({"research":research,"research_archive":[],"proposals":[{"proposal_id":"p","research_id":"batch","requested_title":"Original title","reason":"Verified year","item":proposal_item,"created_at":"2020-01-01"}],"suggestions":[{"suggestion_id":"s","source":"discovery","reason":"Season 3","item":item,"created_at":"2020-01-01","status":"pending","recommended_seasons":[3]},{"suggestion_id":"a","source":"waitlist","reason":"Good fit","item":waitlist,"created_at":"2020-01-01","status":"approved","recommended_seasons":[]}]});
        vec![
            json!([item]),
            json!([waitlist]),
            ws,
            json!({"zoom":125,"offline_enabled":false,"offline_limit_gb":2}),
            json!({}),
        ]
    }
    fn archive(dir: &Path, data: &[Value]) -> Snapshot {
        let path = dir.join("test.zip");
        write_archive(
            &path,
            data,
            vec![(
                "/poster.jpg".into(),
                vec![0xff, 0xd8, 0xff, 0xd9],
                "image/jpeg".into(),
            )],
            vec![],
        )
        .unwrap();
        load_archive(&path).unwrap()
    }
    #[test]
    fn roundtrip_preserves_all_progress_and_images_without_cache_or_secrets() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let data = fixture();
        fs::create_dir(source.path().join("offline")).unwrap();
        fs::write(
            source.path().join("offline/private-cache"),
            b"cache must stay out",
        )
        .unwrap();
        let snap = archive(source.path(), &data);
        assert_eq!(snap.data, data);
        assert!(snap
            .files
            .keys()
            .all(|s| !s.contains("offline") && !s.contains("backups")));
        install(target.path(), &snap, "replace", false).unwrap();
        for i in 0..3 {
            assert_eq!(
                storage::read::<Value>(target.path(), DATA_FILES[i])
                    .unwrap()
                    .unwrap(),
                data[i]
            );
        }
        assert_eq!(
            storage::read::<Value>(target.path(), "config.json")
                .unwrap()
                .unwrap(),
            json!({})
        );
        assert!(saved_image(target.path(), "/poster.jpg")
            .unwrap()
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        assert!(!target.path().join("offline").exists());
    }
    #[test]
    fn merge_is_idempotent_unions_seasons_keeps_dates_and_archives_research() {
        let dir = tempfile::tempdir().unwrap();
        let mut old = fixture();
        let mut incoming = fixture();
        old[0][0]["watched_seasons"] = json!([1]);
        old[0][0]["watched_date"] = json!("2019-02-03");
        old[2]["research"]["id"] = json!("existing");
        old[2]["suggestions"][0]["status"] = json!("approved");
        let mut dup = incoming[0][0].clone();
        dup["white_list_date"] = json!("2022-01-01");
        incoming[1].as_array_mut().unwrap().push(dup);
        for i in 0..3 {
            storage::write(dir.path(), DATA_FILES[i], &old[i]).unwrap();
        }
        let snap = archive(tempfile::tempdir().unwrap().path(), &incoming);
        install(dir.path(), &snap, "merge", false).unwrap();
        let once = read_data(dir.path(), old[3].clone()).unwrap();
        install(dir.path(), &snap, "merge", false).unwrap();
        let twice = read_data(dir.path(), old[3].clone()).unwrap();
        assert_eq!(once, twice);
        assert_eq!(once[0][0]["watched_date"], "2019-02-03");
        assert_eq!(once[0][0]["watched_seasons"], json!([1, 2]));
        assert_eq!(once[0][0]["personal_note"], "Keep me");
        assert_eq!(once[1].as_array().unwrap().len(), 1);
        assert_eq!(once[2]["suggestions"][0]["status"], "approved");
        assert_eq!(once[2]["research_archive"][0]["id"], "batch");
    }
    #[test]
    fn included_key_requires_import_opt_in_and_replace_is_exact() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let mut data = fixture();
        data[4] = json!({"api_key":"exported-test-key"});
        let snap = archive(source.path(), &data);
        storage::write(
            target.path(),
            "config.json",
            &json!({"api_key":"local-test-key","custom_setting":42}),
        )
        .unwrap();
        storage::write(target.path(), "watched.json", &json!([])).unwrap();
        install(target.path(), &snap, "replace", false).unwrap();
        assert_eq!(
            storage::read::<Value>(target.path(), "config.json")
                .unwrap()
                .unwrap()["api_key"],
            "local-test-key"
        );
        install(target.path(), &snap, "replace", true).unwrap();
        let config: Value = storage::read(target.path(), "config.json")
            .unwrap()
            .unwrap();
        assert_eq!(config["api_key"], "exported-test-key");
        assert_eq!(config["custom_setting"], 42);
        assert_eq!(
            storage::read::<Value>(target.path(), "watched.json")
                .unwrap()
                .unwrap(),
            data[0]
        );
    }
    fn rewrite(path: &Path, files: &BTreeMap<String, Vec<u8>>) {
        let mut zip = ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in files {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    #[test]
    fn rejects_tampered_traversal_unknown_version_and_invalid_records() {
        let dir = tempfile::tempdir().unwrap();
        let original = archive(dir.path(), &fixture());
        let path = dir.path().join("bad.zip");
        let mut files = original.files.clone();
        files.insert("data/watched.json".into(), b"[]".to_vec());
        rewrite(&path, &files);
        assert!(load_archive(&path).is_err());
        let mut files = original.files.clone();
        files.insert("../escape.json".into(), b"{}".to_vec());
        rewrite(&path, &files);
        assert!(load_archive(&path).is_err());
        let mut files = original.files.clone();
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["version"] = json!(99);
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        rewrite(&path, &files);
        assert!(load_archive(&path).is_err());
        let mut bad = fixture();
        bad[2]["suggestions"][0]["item"]["title"] = Value::Null;
        assert!(validate_data(&bad).is_err());
    }
    #[test]
    fn interrupted_import_recovers_exact_original_files_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let mut old = fixture();
        old[0][0]["watched_date"] = json!("2018-04-05");
        storage::write(dir.path(), "watched.json", &old[0]).unwrap();
        let before = fs::read(dir.path().join("watched.json")).unwrap();
        let snap = archive(source.path(), &fixture());
        let result = install(dir.path(), &snap, "replace", false).unwrap();
        let backup = PathBuf::from(result["backup"].as_str().unwrap());
        let mut originals = BTreeMap::<String, Option<FileInfo>>::new();
        for file in DATA_FILES.into_iter().chain(["snapshot_images/index.json"]) {
            let bytes = fs::read(backup.join(file)).ok();
            originals.insert(
                file.into(),
                bytes.map(|b| FileInfo {
                    sha256: hash(&b),
                    bytes: b.len() as u64,
                }),
            );
        }
        storage::write(
            dir.path(),
            "snapshot_import_journal.json",
            &json!({"backup":backup.file_name().unwrap().to_str().unwrap(),"originals":originals}),
        )
        .unwrap();
        recover(dir.path()).unwrap();
        assert_eq!(fs::read(dir.path().join("watched.json")).unwrap(), before);
        assert!(!dir.path().join("white_list.json").exists());
        assert!(!dir.path().join("snapshot_import_journal.json").exists());
    }
    #[test]
    fn corrupt_destination_and_missing_recovery_backup_do_not_destroy_progress() {
        let dir = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let snap = archive(source.path(), &fixture());
        fs::write(dir.path().join("watched.json"), b"broken").unwrap();
        assert!(install(dir.path(), &snap, "replace", false).is_err());
        assert_eq!(
            fs::read(dir.path().join("watched.json")).unwrap(),
            b"broken"
        );
        storage::write(
            dir.path(),
            "snapshot_import_journal.json",
            &json!({"backup":"snapshot-import-missing","originals":{}}),
        )
        .unwrap();
        assert!(recover(dir.path()).is_err());
        assert_eq!(
            fs::read(dir.path().join("watched.json")).unwrap(),
            b"broken"
        );
    }
    #[test]
    fn merging_old_proposal_does_not_restore_an_already_approved_match() {
        let mut old = fixture();
        let incoming = fixture();
        old[2]["proposals"] = json!([]);
        old[2]["suggestions"][1]["suggestion_id"] = json!("p");
        let merged = merge_data(&old, &incoming);
        assert!(merged[2]["proposals"].as_array().unwrap().is_empty());
    }
}
fn counts(data: &[Value]) -> Value {
    json!({"watched":data[0].as_array().map_or(0,Vec::len),"waitlist":data[1].as_array().map_or(0,Vec::len),
        "pending_matches":data[2]["proposals"].as_array().map_or(0,Vec::len),
        "pending_suggestions":data[2]["suggestions"].as_array().into_iter().flatten().filter(|s| s["status"].as_str().unwrap_or("pending")=="pending").count(),
        "approved_suggestions":data[2]["suggestions"].as_array().into_iter().flatten().filter(|s| s["status"]=="approved").count(),
        "research_batches":usize::from(!data[2]["research"]["id"].as_str().unwrap_or("").is_empty())+data[2]["research_archive"].as_array().map_or(0,Vec::len)})
}
#[tauri::command]
pub async fn export_snapshot(
    zoom: u64,
    include_api_key: bool,
    destination: Option<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    #[cfg(debug_assertions)]
    let mut selected = destination
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MOVINIGHT_QA_SNAPSHOT_EXPORT").map(PathBuf::from));
    #[cfg(not(debug_assertions))]
    let mut selected: Option<PathBuf> = {
        let _ = destination;
        None
    };
    if selected.is_none() {
        selected = rfd::AsyncFileDialog::new()
            .set_title("Export MoviNight snapshot")
            .add_filter("MoviNight snapshot ZIP", &["zip"])
            .set_file_name(format!(
                "MoviNight-snapshot-{}.zip",
                chrono::Utc::now().format("%Y-%m-%d")
            ))
            .save_file()
            .await
            .map(|f| f.path().to_owned());
    }
    let Some(path) = selected else {
        return Ok(json!({"cancelled":true}));
    };
    let dir = get_config_dir()?;
    let data = {
        let _guard = state.data_lock.lock().unwrap();
        let mut preferences = state.http.offline.preferences()?;
        preferences["zoom"] = json!(zoom);
        let mut data = read_data(&dir, preferences)?;
        if include_api_key {
            let config: Value = storage::read(&dir, "config.json")?.unwrap_or(json!({}));
            if let Some(key) = config["api_key"].as_str().filter(|s| !s.trim().is_empty()) {
                data[4]["api_key"] = json!(key);
            }
        }
        validate_data(&data)?;
        data
    };
    let paths = poster_paths(&data);
    let mut downloads = stream::iter(paths.into_iter().map(|path| {
        let state = &state;
        let dir = &dir;
        async move {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(12),
                thumbnail(state, dir, &path),
            )
            .await;
            (path, result)
        }
    }))
    .buffer_unordered(8);
    let mut images = vec![];
    let mut missing = vec![];
    let mut total = data
        .iter()
        .map(|v| serde_json::to_vec(v).map(|b| b.len() as u64))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<u64>();
    while let Some((path, result)) = downloads.next().await {
        match result {
            Ok(Ok((bytes, mime))) => {
                total += bytes.len() as u64;
                if total > MAX_TOTAL - 1024 * 1024 {
                    return Err(invalid("Snapshot exceeds the 512 MB transfer limit"));
                }
                images.push((path, bytes, mime))
            }
            _ => missing.push(path),
        }
    }
    write_archive(&path, &data, images, missing)
}
fn load_archive(path: &Path) -> Result<Snapshot, ApiError> {
    if fs::metadata(path)?.len() > MAX_TOTAL {
        return Err(invalid("Snapshot ZIP exceeds 512 MB"));
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > MAX_TOTAL {
        return Err(invalid("Snapshot ZIP exceeds 512 MB"));
    }
    let digest = hash(&bytes);
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).map_err(zip_error)?;
    if archive.len() > 20000 {
        return Err(invalid("Too many snapshot entries"));
    }
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_error)?;
        let name = entry.name().to_owned();
        let allowed = name == "manifest.json"
            || name == "README.txt"
            || DATA_FILES.iter().any(|f| name == format!("data/{f}"))
            || safe_thumbnail(&name);
        if !allowed
            || entry.is_dir()
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || files.contains_key(&name)
        {
            return Err(invalid(
                "Snapshot has unexpected, duplicate or unsafe entries",
            ));
        }
        let limit = if name.starts_with("thumbnails/") {
            MAX_IMAGE
        } else {
            MAX_JSON
        };
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| invalid("Snapshot is too large"))?;
        if entry.size() > limit || total > MAX_TOTAL {
            return Err(invalid("Snapshot exceeds its extraction limits"));
        }
        let mut contents = Vec::new();
        (&mut entry).take(limit + 1).read_to_end(&mut contents)?;
        if contents.len() as u64 > limit || contents.len() as u64 != entry.size() {
            return Err(invalid("Snapshot entry has invalid size"));
        }
        files.insert(name, contents);
    }
    let manifest: Manifest = serde_json::from_slice(
        files
            .get("manifest.json")
            .ok_or_else(|| invalid("Not a MoviNight snapshot"))?,
    )?;
    if manifest.format != "movinight-snapshot" || manifest.version != 1 {
        return Err(invalid("Unsupported snapshot format version"));
    }
    if manifest.files.len() + 1 != files.len() {
        return Err(invalid("Snapshot file inventory does not match"));
    }
    for (name, info) in &manifest.files {
        let bytes = files
            .get(name)
            .ok_or_else(|| invalid("Snapshot is missing a file"))?;
        if hash(bytes) != info.sha256 || bytes.len() as u64 != info.bytes {
            return Err(invalid(format!("Snapshot checksum mismatch: {name}")));
        }
    }
    let mut image_paths = BTreeSet::new();
    let mut image_files = BTreeSet::new();
    for image in &manifest.thumbnails {
        offline::image_key(&image.path, "w342")?;
        if !safe_thumbnail(&image.file)
            || !image_paths.insert(&image.path)
            || !image_files.insert(&image.file)
            || !files
                .get(&image.file)
                .is_some_and(|b| valid_image(b, &image.mime))
        {
            return Err(invalid("Snapshot thumbnail index is invalid"));
        }
    }
    if image_files.len()
        != files
            .keys()
            .filter(|f| f.starts_with("thumbnails/"))
            .count()
    {
        return Err(invalid("Snapshot contains unindexed thumbnails"));
    }
    let data = DATA_FILES
        .iter()
        .map(|f| {
            files
                .get(&format!("data/{f}"))
                .ok_or_else(|| invalid("Snapshot data is incomplete"))
                .and_then(|b| serde_json::from_slice(b).map_err(ApiError::from))
        })
        .collect::<Result<Vec<Value>, _>>()?;
    validate_data(&data)?;
    if !manifest
        .thumbnails
        .iter()
        .all(|i| poster_paths(&data).contains(&i.path))
    {
        return Err(invalid("Snapshot includes unrelated artwork"));
    }
    Ok(Snapshot {
        manifest,
        files,
        data,
        digest,
    })
}
#[tauri::command]
pub async fn preview_snapshot(
    path: Option<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    #[cfg(debug_assertions)]
    let mut selected = path
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("MOVINIGHT_QA_SNAPSHOT_IMPORT").map(PathBuf::from));
    #[cfg(not(debug_assertions))]
    let mut selected: Option<PathBuf> = {
        let _ = path;
        None
    };
    if selected.is_none() {
        selected = rfd::AsyncFileDialog::new()
            .set_title("Import MoviNight snapshot")
            .add_filter("MoviNight snapshot ZIP", &["zip"])
            .pick_file()
            .await
            .map(|f| f.path().to_owned());
    }
    let Some(path) = selected else {
        return Ok(json!({"cancelled":true}));
    };
    let snapshot = load_archive(&path)?;
    let _guard = state.data_lock.lock().unwrap();
    let mut prefs = state.http.offline.preferences()?;
    prefs["zoom"] = json!(100);
    let existing = read_data(&get_config_dir()?, prefs)?;
    Ok(
        json!({"path":path,"digest":snapshot.digest,"created_at":snapshot.manifest.created_at,"app_version":snapshot.manifest.app_version,
        "counts":counts(&snapshot.data),"existing":counts(&existing),"contains_api_key":snapshot.data[4]["api_key"].is_string(),"thumbnails":snapshot.manifest.thumbnails.len(),"missing_thumbnails":snapshot.manifest.missing_thumbnails.len()}),
    )
}
fn identity(v: &Value) -> (String, u64) {
    (
        v["content_type"].as_str().unwrap_or("").into(),
        v["id"].as_u64().unwrap_or(0),
    )
}
#[tauri::command]
pub fn apply_snapshot_preferences(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let mut prefs: Value = storage::read(&dir, "snapshot_preferences.json")?.unwrap_or(json!({}));
    if prefs["requires_apply"] != true {
        return Ok(Value::Null);
    }
    let enabled = prefs["offline_enabled"]
        .as_bool()
        .ok_or_else(|| invalid("Invalid imported cache preference"))?;
    let limit = prefs["offline_limit_gb"]
        .as_u64()
        .filter(|n| (1..=5).contains(n))
        .ok_or_else(|| invalid("Invalid imported cache limit"))?;
    state.http.offline.apply_preferences(enabled, limit)?;
    prefs["requires_apply"] = json!(false);
    storage::write(&dir, "snapshot_preferences.json", &prefs)?;
    Ok(prefs)
}
fn merge_items(existing: &Value, incoming: &Value, watched: bool) -> Value {
    let mut result = existing.as_array().unwrap().clone();
    for item in incoming.as_array().unwrap() {
        if let Some(current) = result.iter_mut().find(|c| identity(c) == identity(item)) {
            let mut merged = item.as_object().unwrap().clone();
            merged.extend(current.as_object().unwrap().clone());
            if watched {
                let seasons = current["watched_seasons"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(item["watched_seasons"].as_array().into_iter().flatten())
                    .filter_map(Value::as_u64)
                    .collect::<BTreeSet<_>>();
                if current["watched_seasons"].is_array() || item["watched_seasons"].is_array() {
                    merged.insert("watched_seasons".into(), json!(seasons));
                }
                if let Some(known) = current["total_seasons_known"]
                    .as_u64()
                    .into_iter()
                    .chain(item["total_seasons_known"].as_u64())
                    .max()
                {
                    merged.insert("total_seasons_known".into(), json!(known));
                }
            }
            *current = Value::Object(merged);
        } else {
            result.push(item.clone());
        }
    }
    json!(result)
}
fn merge_data(existing: &[Value], incoming: &[Value]) -> Vec<Value> {
    let watched = merge_items(&existing[0], &incoming[0], true);
    let mut waitlist = merge_items(&existing[1], &incoming[1], false);
    waitlist.as_array_mut().unwrap().retain(|i| {
        !watched
            .as_array()
            .unwrap()
            .iter()
            .any(|w| identity(w) == identity(i))
    });
    let mut ws = existing[2].clone();
    for (field, key) in [
        ("proposals", "proposal_id"),
        ("suggestions", "suggestion_id"),
    ] {
        let mut records = ws[field].as_array().cloned().unwrap_or_default();
        for item in incoming[2][field].as_array().into_iter().flatten() {
            if let Some(current) = records.iter_mut().find(|c| c[key] == item[key]) {
                // An approved record must never return to pending on a merge.
                if field == "suggestions" && item["status"] == "approved" {
                    current["status"] = json!("approved");
                }
            } else {
                records.push(item.clone());
            }
        }
        ws[field] = json!(records);
    }
    let mut research = ws["research_archive"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for batch in incoming[2]["research_archive"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(std::iter::once(&incoming[2]["research"]))
    {
        if !batch["id"].as_str().unwrap_or("").is_empty()
            && batch["id"] != ws["research"]["id"]
            && !research.iter().any(|r| r["id"] == batch["id"])
        {
            research.push(batch.clone());
        }
    }
    if ws["research"]["id"].as_str().unwrap_or("").is_empty() {
        ws["research"] = incoming[2]["research"].clone();
        research.retain(|r| r["id"] != ws["research"]["id"]);
    }
    ws["research_archive"] = json!(research);
    let approved = ws["suggestions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["status"] == "approved")
        .map(|s| s["suggestion_id"].clone())
        .collect::<Vec<_>>();
    ws["proposals"]
        .as_array_mut()
        .unwrap()
        .retain(|p| !approved.contains(&p["proposal_id"]));
    vec![
        watched,
        waitlist,
        ws,
        incoming[3].clone(),
        incoming[4].clone(),
    ]
}
// Journal covers the complete progress transaction. Recovery runs before app commands.
pub(crate) fn recover(dir: &Path) -> Result<(), ApiError> {
    let Some(journal): Option<Value> = storage::read(dir, "snapshot_import_journal.json")? else {
        return Ok(());
    };
    let name = journal["backup"]
        .as_str()
        .filter(|s| {
            s.starts_with("snapshot-import-")
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        .ok_or_else(|| invalid("Invalid snapshot recovery journal"))?;
    let backup = dir.join("backups").join(name);
    if !backup.is_dir() {
        return Err(invalid(
            "Snapshot recovery backup is missing; progress was left untouched",
        ));
    }
    let originals: BTreeMap<String, Option<FileInfo>> =
        serde_json::from_value(journal["originals"].clone())?;
    let expected = DATA_FILES
        .into_iter()
        .chain(["snapshot_images/index.json"])
        .collect::<BTreeSet<_>>();
    if originals
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err(invalid("Invalid snapshot recovery inventory"));
    }
    // Validate every recovery file before touching any current progress.
    for (file, info) in &originals {
        if let Some(info) = info {
            let bytes = fs::read(backup.join(file))?;
            if bytes.len() as u64 != info.bytes || hash(&bytes) != info.sha256 {
                return Err(invalid("Snapshot recovery backup failed verification"));
            }
        }
    }
    for file in DATA_FILES.into_iter().chain(["snapshot_images/index.json"]) {
        let path = dir.join(file);
        if originals[file].is_some() {
            fs::create_dir_all(path.parent().unwrap())?;
            let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
            temp.write_all(&fs::read(backup.join(file))?)?;
            temp.as_file().sync_all()?;
            temp.persist(&path).map_err(|e| e.error)?;
        } else if path.exists() {
            fs::remove_file(&path)?;
        }
    }
    fs::remove_file(dir.join("snapshot_import_journal.json"))?;
    Ok(())
}
fn install(
    dir: &Path,
    snapshot: &Snapshot,
    mode: &str,
    import_api_key: bool,
) -> Result<Value, ApiError> {
    if !["merge", "replace"].contains(&mode) {
        return Err(invalid("Choose merge or replace"));
    }
    let existing = read_data(dir, snapshot.data[3].clone())?;
    let mut data = if mode == "merge" {
        merge_data(&existing, &snapshot.data)
    } else {
        snapshot.data.clone()
    };
    // Watched always wins over Waitlist, including old snapshots containing both.
    let watched_keys = data[0]
        .as_array()
        .unwrap()
        .iter()
        .map(identity)
        .collect::<BTreeSet<_>>();
    data[1]
        .as_array_mut()
        .unwrap()
        .retain(|i| !watched_keys.contains(&identity(i)));
    validate_data(&data)?;
    let mut config: Value = storage::read(dir, "config.json")?.unwrap_or(json!({}));
    if !config.is_object() {
        return Err(invalid(
            "Current TMDB configuration is invalid; progress was left untouched",
        ));
    }
    if import_api_key && data[4]["api_key"].is_string() {
        config["api_key"] = data[4]["api_key"].clone();
    }
    data[4] = config;
    let name = format!("snapshot-import-{}", uuid::Uuid::new_v4());
    let backup = dir.join("backups").join(&name);
    fs::create_dir_all(&backup)?;
    let mut originals = BTreeMap::<String, Option<FileInfo>>::new();
    for file in DATA_FILES.into_iter().chain(["snapshot_images/index.json"]) {
        if dir.join(file).exists() {
            let bytes = fs::read(dir.join(file))?;
            let target = backup.join(file);
            fs::create_dir_all(target.parent().unwrap())?;
            let mut out = fs::File::create(&target)?;
            out.write_all(&bytes)?;
            out.sync_all()?;
            originals.insert(
                file.into(),
                Some(FileInfo {
                    sha256: hash(&bytes),
                    bytes: bytes.len() as u64,
                }),
            );
        } else {
            originals.insert(file.into(), None);
        }
    }
    let mut index: BTreeMap<String, Thumbnail> = if mode == "merge" {
        storage::read(&dir.join("snapshot_images"), "index.json")?.unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    fs::create_dir_all(dir.join("snapshot_images"))?;
    for image in &snapshot.manifest.thumbnails {
        let filename = Path::new(&image.file).file_name().unwrap();
        // Address by content rather than TMDB path, so pre-import images remain recoverable.
        let bytes = &snapshot.files[&image.file];
        let ext = Path::new(filename).extension().unwrap().to_string_lossy();
        let file = format!("thumbnails/{}.{ext}", hash(bytes));
        let target = dir
            .join("snapshot_images")
            .join(Path::new(&file).file_name().unwrap());
        if !target.exists() {
            let mut temp = tempfile::NamedTempFile::new_in(dir.join("snapshot_images"))?;
            temp.write_all(bytes)?;
            temp.as_file().sync_all()?;
            temp.persist(&target).map_err(|e| e.error)?;
        }
        index.insert(
            image.path.clone(),
            Thumbnail {
                path: image.path.clone(),
                file,
                mime: image.mime.clone(),
            },
        );
    }
    storage::write(
        dir,
        "snapshot_import_journal.json",
        &json!({"backup":name,"originals":originals}),
    )?;
    data[3]["requires_apply"] = json!(true);
    let transaction = (|| -> Result<(), ApiError> {
        for (i, file) in DATA_FILES.iter().enumerate() {
            storage::write(dir, file, &data[i])?;
        }
        storage::write(&dir.join("snapshot_images"), "index.json", &index)?;
        Ok(())
    })();
    if let Err(error) = transaction {
        recover(dir)?;
        return Err(error);
    }
    fs::remove_file(dir.join("snapshot_import_journal.json"))?;
    Ok(json!({"backup":backup,"counts":counts(&data),"preferences":data[3],"mode":mode}))
}
#[tauri::command]
pub async fn import_snapshot(
    path: String,
    digest: String,
    mode: String,
    import_api_key: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Value, ApiError> {
    let snapshot = load_archive(Path::new(&path))?;
    if snapshot.digest != digest {
        return Err(invalid(
            "Snapshot changed after preview. Choose the file again.",
        ));
    }
    let _guard = state.data_lock.lock().unwrap();
    let dir = get_config_dir()?;
    let result = install(&dir, &snapshot, &mode, import_api_key)?;
    let config: Value = storage::read(&dir, "config.json")?.unwrap_or(json!({}));
    *state.api_key.lock().unwrap() = config["api_key"].as_str().map(str::to_owned);
    state.http.clear();
    Ok(result)
}
