use crate::ApiError;
use serde::{de::DeserializeOwned, Serialize};
use std::{fs, io::Write, path::Path};

pub fn read<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<Option<T>, ApiError> {
    match fs::read(dir.join(name)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<(), ApiError> {
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    // Never overwrite unreadable JSON. Keep a once-per-version original plus last-good copy.
    if path.exists() {
        let bytes = fs::read(&path)?;
        let _: serde_json::Value = serde_json::from_slice(&bytes)?;
        let backup_dir = dir.join("backups").join("before-1.1.0");
        fs::create_dir_all(&backup_dir)?;
        let original = backup_dir.join(name);
        if !original.exists() {
            fs::copy(&path, original)?;
        }
        let mut backup = tempfile::NamedTempFile::new_in(dir)?;
        backup.write_all(&bytes)?;
        backup.as_file().sync_all()?;
        backup
            .persist(dir.join(format!("{name}.bak")))
            .map_err(|e| e.error)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(&path).map_err(|e| e.error)?;
    Ok(())
}
pub fn backup(dir: &Path) -> Result<String, ApiError> {
    let destination = dir.join("backups").join(format!(
        "manual-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&destination)?;
    for name in ["watched.json", "white_list.json", "ai_workspace.json"] {
        if dir.join(name).exists() {
            fs::copy(dir.join(name), destination.join(name))?;
        }
    }
    Ok(destination.display().to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_history_survives_atomic_update_and_backups() {
        let dir = tempfile::tempdir().unwrap();
        let old = br#"[{"id":1,"title":"Legacy","overview":"","poster_path":null,"release_date":"2000-01-01","vote_average":8,"content_type":"tv","watched_date":"2020-02-03","personal_note":"Keep this custom field"}]"#;
        fs::write(dir.path().join("watched.json"), old).unwrap();
        let mut items: Vec<crate::WatchedItem> = read(dir.path(), "watched.json").unwrap().unwrap();
        assert_eq!(items[0].watched_date, "2020-02-03");
        assert!(items[0].watched_seasons.is_none());
        items[0].watched_seasons = Some(vec![1, 2]);
        write(dir.path(), "watched.json", &items).unwrap();
        let reread: Vec<crate::WatchedItem> = read(dir.path(), "watched.json").unwrap().unwrap();
        assert_eq!(reread[0].watched_date, "2020-02-03");
        assert_eq!(reread[0].extra["personal_note"], "Keep this custom field");
        assert_eq!(reread[0].watched_seasons, Some(vec![1, 2]));
        assert_eq!(
            fs::read(dir.path().join("backups/before-1.1.0/watched.json")).unwrap(),
            old
        );
        write(dir.path(), "watched.json", &items).unwrap();
        assert_eq!(
            fs::read(dir.path().join("backups/before-1.1.0/watched.json")).unwrap(),
            old
        );
    }
    #[test]
    fn corrupt_data_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("white_list.json"), b"broken JSON").unwrap();
        assert!(write(dir.path(), "white_list.json", &Vec::<u32>::new()).is_err());
        assert_eq!(
            fs::read(dir.path().join("white_list.json")).unwrap(),
            b"broken JSON"
        );
    }
}
