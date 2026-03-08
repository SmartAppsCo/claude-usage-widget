use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::cookies::platform::{chrome_encryption_key, decrypt_chrome_value};
use crate::cookies::{CookieError, CookieJar};

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

    // Read the encryption key once (only needed on Windows; returns None elsewhere).
    let key = chrome_encryption_key(db_path);

    let (_tmp, tmp_db) = super::copy_db(db_path)?;
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
        match decrypt_chrome_value(&encrypted, key.as_deref()) {
            Ok(val) if !val.is_empty() => {
                jar.insert(name, val);
            }
            _ => {}
        }
    }
    Ok(jar)
}
