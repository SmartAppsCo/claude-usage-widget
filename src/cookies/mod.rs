pub mod chrome;
pub mod firefox;
pub mod platform;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub type CookieJar = HashMap<String, String>;

/// Open a SQLite database read-only.  On Windows, Chrome/Edge hold an
/// exclusive OS-level lock on the Cookies file; if the initial open fails
/// we use the Restart Manager API to briefly release that lock.
pub(crate) fn open_db(db_path: &Path) -> Result<rusqlite::Connection, CookieError> {
    use rusqlite::{Connection, OpenFlags};

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    match Connection::open_with_flags(db_path, flags) {
        Ok(conn) => Ok(conn),
        #[cfg(windows)]
        Err(_) => {
            release_file_lock(db_path);
            Connection::open_with_flags(db_path, flags).map_err(CookieError::Sqlite)
        }
        #[cfg(not(windows))]
        Err(e) => Err(CookieError::Sqlite(e)),
    }
}

/// Use the Windows Restart Manager API to release a file lock held by another
/// process (e.g. Chrome/Edge holding the Cookies database).  The browser
/// subprocess that held the lock will restart automatically.
#[cfg(windows)]
fn release_file_lock(path: &Path) {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::*;

    unsafe {
        let file_path = HSTRING::from(path.as_os_str());
        let mut session: u32 = 0;
        let mut session_key_buf = [0u16; (CCH_RM_SESSION_KEY as usize) + 1];
        let session_key = PWSTR(session_key_buf.as_mut_ptr());

        if RmStartSession(&mut session, None, session_key) != ERROR_SUCCESS {
            return;
        }

        if RmRegisterResources(
            session,
            Some(&[PCWSTR(file_path.as_ptr())]),
            None,
            None,
        ) != ERROR_SUCCESS
        {
            let _ = RmEndSession(session);
            return;
        }

        let mut needed: u32 = 0;
        let mut info = [RM_PROCESS_INFO::default()];
        let mut reasons: u32 = 0;
        let mut count: u32 = 0;
        let result = RmGetList(
            session,
            &mut needed,
            &mut count,
            Some(info.as_mut_ptr()),
            &mut reasons,
        );

        if (result == ERROR_SUCCESS || result == ERROR_MORE_DATA) && needed > 0 {
            let _ = RmShutdown(session, RmForceShutdown.0 as u32, None);
        }

        let _ = RmEndSession(session);
    }
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
