use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use crate::cookies::platform::firefox_default_dir;
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

pub fn read(domain: &str, data_dir: Option<&str>) -> Result<CookieJar, CookieError> {
    let ff_dir = match data_dir {
        Some(d) => PathBuf::from(d),
        None => firefox_default_dir().ok_or(CookieError::NoBrowserDir)?,
    };
    if !ff_dir.is_dir() {
        return Err(CookieError::NoBrowserDir);
    }

    let mut dbs: Vec<PathBuf> = Vec::new();

    // Scan profile subdirectories for cookies.sqlite
    if let Ok(entries) = fs::read_dir(&ff_dir) {
        for entry in entries.flatten() {
            let path = entry.path().join("cookies.sqlite");
            if path.is_file() {
                dbs.push(path);
            }
        }
    }

    // Also check for cookies.sqlite directly in the directory
    let direct = ff_dir.join("cookies.sqlite");
    if direct.is_file() {
        dbs.push(direct);
    }

    if dbs.is_empty() {
        return Err(CookieError::NoCookieDb);
    }

    // Pick the most recently modified
    dbs.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    let db_path = dbs.last().unwrap();

    let (_tmp, tmp_db) = copy_db(db_path)?;
    let conn = Connection::open(&tmp_db).map_err(CookieError::Sqlite)?;
    let mut stmt = conn
        .prepare("SELECT name, value FROM moz_cookies WHERE host LIKE ?1")
        .map_err(CookieError::Sqlite)?;
    let pattern = format!("%{domain}%");
    let rows = stmt
        .query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CookieError::Sqlite)?;

    let mut jar = CookieJar::new();
    for row in rows {
        let (name, value) = row.map_err(CookieError::Sqlite)?;
        jar.insert(name, value);
    }
    Ok(jar)
}
