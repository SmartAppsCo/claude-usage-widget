pub mod chrome;
pub mod firefox;
pub mod platform;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub type CookieJar = HashMap<String, String>;

pub(crate) fn copy_db(db_path: &Path) -> Result<(TempDir, PathBuf), CookieError> {
    let name = db_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let tmp = TempDir::new().map_err(CookieError::Io)?;
    let tmp_db = tmp.path().join(&name);
    std::fs::copy(db_path, &tmp_db).map_err(CookieError::Io)?;
    for ext in ["-wal", "-shm"] {
        let src = db_path.with_file_name(format!("{name}{ext}"));
        let dst = tmp.path().join(format!("{name}{ext}"));
        let _ = std::fs::copy(&src, &dst);
    }
    Ok((tmp, tmp_db))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserKind {
    Firefox,
    Chrome,
    Brave,
    Edge,
}

impl fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrowserKind::Firefox => write!(f, "firefox"),
            BrowserKind::Chrome => write!(f, "chrome"),
            BrowserKind::Brave => write!(f, "brave"),
            BrowserKind::Edge => write!(f, "edge"),
        }
    }
}

#[derive(Debug)]
pub enum CookieError {
    NoBrowserDir,
    NoCookieDb,
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Decrypt(String),
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CookieError::NoBrowserDir => write!(f, "Browser directory not found"),
            CookieError::NoCookieDb => write!(f, "No cookie database found"),
            CookieError::Sqlite(e) => write!(f, "SQLite error: {e}"),
            CookieError::Io(e) => write!(f, "IO error: {e}"),
            CookieError::Decrypt(e) => write!(f, "Decrypt error: {e}"),
        }
    }
}

pub fn read_cookies(
    browser: BrowserKind,
    domain: &str,
    data_dir: Option<&str>,
) -> Result<CookieJar, CookieError> {
    match browser {
        BrowserKind::Firefox => firefox::read(domain, data_dir),
        BrowserKind::Chrome => chrome::read(domain, data_dir, platform::chrome_default_dirs),
        BrowserKind::Brave => chrome::read(domain, data_dir, platform::brave_default_dirs),
        BrowserKind::Edge => chrome::read(domain, data_dir, platform::edge_default_dirs),
    }
}

pub fn detect_browsers(domain: &str) -> HashMap<BrowserKind, CookieJar> {
    let browsers = [BrowserKind::Firefox, BrowserKind::Chrome, BrowserKind::Brave, BrowserKind::Edge];
    let handles: Vec<_> = browsers
        .iter()
        .map(|&b| {
            let domain = domain.to_owned();
            std::thread::spawn(move || (b, read_cookies(b, &domain, None)))
        })
        .collect();
    let mut found = HashMap::new();
    for handle in handles {
        if let Ok((b, Ok(cookies))) = handle.join()
            && cookies.contains_key("sessionKey")
        {
            found.insert(b, cookies);
        }
    }
    found
}
