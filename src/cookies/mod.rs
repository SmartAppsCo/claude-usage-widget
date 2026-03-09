pub mod chrome;
pub mod firefox;
pub mod platform;
#[cfg(target_os = "macos")]
pub mod safari;

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
    #[cfg(target_os = "macos")]
    Safari,
}

impl fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrowserKind::Firefox => write!(f, "firefox"),
            BrowserKind::Chrome => write!(f, "chrome"),
            BrowserKind::Brave => write!(f, "brave"),
            BrowserKind::Edge => write!(f, "edge"),
            #[cfg(target_os = "macos")]
            BrowserKind::Safari => write!(f, "safari"),
        }
    }
}

#[derive(Debug)]
pub enum CookieError {
    NoBrowserDir,
    NoCookieDb,
    #[cfg(target_os = "macos")]
    PermissionDenied,
    Sqlite(rusqlite::Error),
    Decrypt(String),
}

impl fmt::Display for CookieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CookieError::NoBrowserDir => write!(f, "Browser directory not found"),
            CookieError::NoCookieDb => write!(f, "No cookie database found"),
            #[cfg(target_os = "macos")]
            CookieError::PermissionDenied => write!(f, "Permission denied"),
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
        #[cfg(target_os = "macos")]
        BrowserKind::Safari => safari::read(domain),
    }
}

/// Try browsers in priority order, return the first one with a valid session.
/// On macOS/Windows, shows explanatory dialogs before permission prompts.
pub fn detect_browser(domain: &str) -> Option<BrowserKind> {
    // Order: prompt-free browsers first, then browsers that may prompt.
    // Safari last on macOS: it requires Full Disk Access (multi-step grant),
    // while the keychain prompt for Chromium is just a password entry.
    #[cfg(target_os = "macos")]
    let browsers = &[
        BrowserKind::Firefox,
        BrowserKind::Chrome,
        BrowserKind::Brave,
        BrowserKind::Edge,
        BrowserKind::Safari,
    ];
    #[cfg(not(target_os = "macos"))]
    let browsers = &[
        BrowserKind::Firefox,
        BrowserKind::Chrome,
        BrowserKind::Brave,
        BrowserKind::Edge,
    ];

    #[cfg(target_os = "macos")]
    let mut prompted_keychain = false;
    #[cfg(target_os = "windows")]
    let mut prompted_elevation = false;

    for &b in browsers {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let is_chromium = matches!(b, BrowserKind::Chrome | BrowserKind::Brave | BrowserKind::Edge);

        // On macOS, handle special prompts before attempting reads.
        #[cfg(target_os = "macos")]
        {
            if b == BrowserKind::Safari {
                match safari::read(domain) {
                    Ok(cookies) if cookies.contains_key("sessionKey") => return Some(b),
                    Err(CookieError::PermissionDenied) => {
                        platform::prompt_full_disk_access();
                    }
                    _ => {}
                }
                continue;
            }
            // Show keychain explanation once before the first Chromium browser,
            // but skip if we already prompted from this same binary path
            // (macOS ties "Always Allow" to the binary, so same path = safe).
            if !prompted_keychain && is_chromium {
                prompted_keychain = true;
                let exe_path = std::env::current_exe().ok()
                    .and_then(|p| p.to_str().map(String::from));
                let config = crate::config::Config::load();
                let already_prompted = exe_path.is_some()
                    && config.chromium_prompted_exe.as_ref() == exe_path.as_ref();
                if !already_prompted {
                    if !platform::prompt_keychain_access() {
                        return None; // user cancelled
                    }
                    // Save that we prompted from this exe path.
                    let mut config = crate::config::Config::load();
                    config.chromium_prompted_exe = exe_path;
                    config.save();
                }
            }
        }

        // On Windows, prompt for elevation before trying Chromium browsers
        // (only if v20 App-Bound Encryption is detected).
        #[cfg(target_os = "windows")]
        if !prompted_elevation && is_chromium {
            prompted_elevation = true;
            if !platform::prompt_and_elevate_if_needed(domain) {
                return None; // user cancelled
            }
        }

        if let Ok(cookies) = read_cookies(b, domain, None) {
            if cookies.contains_key("sessionKey") {
                return Some(b);
            }
        }
    }
    None
}
