pub mod chrome;
pub mod firefox;
pub mod platform;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub type CookieJar = HashMap<String, String>;

/// Open a SQLite database in immutable mode, bypassing all file locking.
/// We never write to cookie databases, so this is safe and avoids conflicts
/// with browsers holding WAL or exclusive locks.
pub(crate) fn open_db(db_path: &Path) -> Result<rusqlite::Connection, CookieError> {
    use rusqlite::{Connection, OpenFlags};

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let encoded = db_path.display().to_string()
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('?', "%3F")
        .replace('#', "%23");
    let uri = format!("file:{encoded}?immutable=1");
    Connection::open_with_flags(uri, flags).map_err(CookieError::Sqlite)
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
    Decrypt(String),
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CookieError::NoBrowserDir => write!(f, "Browser directory not found"),
            CookieError::NoCookieDb => write!(f, "No cookie database found"),
            CookieError::Sqlite(e) => write!(f, "SQLite error: {e}"),
            CookieError::Decrypt(e) => write!(f, "Decrypt error: {e}"),
        }
    }
}

/// Check whether any Chromium browser has v20-encrypted cookies for the
/// given domain.  This is a lightweight check that opens the DB and peeks at
/// the raw `encrypted_value` prefix without decrypting anything.
#[cfg(windows)]
pub fn needs_elevation(domain: &str) -> bool {
    let dirs: Vec<fn() -> Vec<std::path::PathBuf>> = vec![
        platform::chrome_default_dirs,
        platform::brave_default_dirs,
        platform::edge_default_dirs,
    ];
    for dir_fn in dirs {
        if chrome::has_v20_cookies(domain, dir_fn) {
            return true;
        }
    }
    false
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
