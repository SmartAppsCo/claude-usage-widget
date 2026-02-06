pub mod chrome;
pub mod firefox;
pub mod platform;

use std::collections::HashMap;
use std::fmt;

pub type CookieJar = HashMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserKind {
    Firefox,
    Chrome,
    Brave,
}

impl fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrowserKind::Firefox => write!(f, "firefox"),
            BrowserKind::Chrome => write!(f, "chrome"),
            BrowserKind::Brave => write!(f, "brave"),
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
    }
}

pub fn detect_browsers(domain: &str) -> HashMap<BrowserKind, CookieJar> {
    let mut found = HashMap::new();
    for browser in [BrowserKind::Firefox, BrowserKind::Chrome, BrowserKind::Brave] {
        if let Ok(cookies) = read_cookies(browser, domain, None)
            && cookies.contains_key("sessionKey")
        {
            found.insert(browser, cookies);
        }
    }
    found
}
