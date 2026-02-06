use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use crate::cookies::platform::decrypt_chrome_value;
use crate::cookies::{CookieError, CookieJar};

fn copy_db(db_path: &Path) -> Result<(TempDir, PathBuf), CookieError> {
    let tmp = TempDir::new().map_err(CookieError::Io)?;
    let name = db_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let tmp_db = tmp.path().join(&name);
    fs::copy(db_path, &tmp_db).map_err(CookieError::Io)?;
    for ext in ["-wal", "-shm"] {
        let src = db_path.with_file_name(format!("{name}{ext}"));
        if src.exists() {
            let dst = tmp.path().join(format!("{name}{ext}"));
            let _ = fs::copy(&src, &dst);
        }
    }
    Ok((tmp, tmp_db))
}

pub fn read(domain: &str, data_dir: Option<&str>, default_dirs: fn() -> Vec<PathBuf>) -> Result<CookieJar, CookieError> {
    let base_dirs: Vec<PathBuf> = match data_dir {
        Some(d) => vec![PathBuf::from(d)],
        None => default_dirs(),
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    for base_dir in &base_dirs {
        if !base_dir.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(base_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == "Default" || name_str.starts_with("Profile ") {
                for sub in ["Network/Cookies", "Cookies"] {
                    let path = entry.path().join(sub);
                    if path.is_file() {
                        candidates.push(path);
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return Err(CookieError::NoCookieDb);
    }

    // Pick the most recently modified
    candidates.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    let db_path = candidates.last().unwrap();

    let (_tmp, tmp_db) = copy_db(db_path)?;
    let conn = Connection::open(&tmp_db).map_err(CookieError::Sqlite)?;
    let mut stmt = conn
        .prepare("SELECT name, encrypted_value FROM cookies WHERE host_key LIKE ?1")
        .map_err(CookieError::Sqlite)?;
    let pattern = format!("%{domain}%");
    let rows = stmt
        .query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })
        .map_err(CookieError::Sqlite)?;

    let mut jar = CookieJar::new();
    for row in rows {
        let (name, encrypted) = row.map_err(CookieError::Sqlite)?;
        match decrypt_chrome_value(&encrypted) {
            Ok(val) if !val.is_empty() => {
                jar.insert(name, val);
            }
            _ => {}
        }
    }
    Ok(jar)
}
