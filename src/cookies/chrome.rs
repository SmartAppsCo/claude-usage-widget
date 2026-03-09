use std::fs;
use std::path::PathBuf;

use crate::cookies::platform::{chrome_encryption_key, decrypt_chrome_value};
use crate::cookies::{CookieError, CookieJar};

/// Check if any cookie DB from `default_dirs` contains v20-encrypted cookies
/// for the given domain.  Does not decrypt anything.
#[cfg(windows)]
pub fn has_v20_cookies(domain: &str, default_dirs: fn() -> Vec<PathBuf>) -> bool {
    let db_path = match find_cookie_db(None, default_dirs) {
        Some(p) => p,
        None => return false,
    };
    let conn = match super::open_db(&db_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let mut stmt = match conn.prepare(
        "SELECT encrypted_value FROM cookies WHERE host_key LIKE ?1 LIMIT 1",
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pattern = format!("%{domain}%");
    let Ok(mut rows) = stmt.query([&pattern]) else {
        return false;
    };
    if let Ok(Some(row)) = rows.next() {
        if let Ok(val) = row.get::<_, Vec<u8>>(0) {
            return val.len() >= 3 && &val[..3] == b"v20";
        }
    }
    false
}

fn find_cookie_db(data_dir: Option<&str>, default_dirs: fn() -> Vec<PathBuf>) -> Option<PathBuf> {
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
                for sub in [&["Network", "Cookies"] as &[&str], &["Cookies"]] {
                    let path = sub.iter().fold(entry.path(), |p, s| p.join(s));
                    if path.is_file() {
                        candidates.push(path);
                    }
                }
            }
        }
    }

    candidates.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    candidates.pop()
}

pub fn read(domain: &str, data_dir: Option<&str>, default_dirs: fn() -> Vec<PathBuf>) -> Result<CookieJar, CookieError> {
    let db_path = find_cookie_db(data_dir, default_dirs).ok_or(CookieError::NoCookieDb)?;

    // Read the encryption key once (only needed on Windows; returns None elsewhere).
    let key = chrome_encryption_key(&db_path);

    let conn = super::open_db(&db_path)?;
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
