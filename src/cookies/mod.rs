pub mod chrome;
pub mod firefox;
pub mod platform;
#[cfg(target_os = "macos")]
pub mod safari;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub type CookieJar = HashMap<String, String>;

/// Open a SQLite database read-only.
///
/// On Unix we use `immutable=1` URI mode to bypass WAL/file locks (browsers
/// like Firefox hold WAL locks that block normal readers).
///
/// On Windows, browsers hold mandatory exclusive file locks.  If the initial
/// open fails we use the Restart Manager API to briefly release the lock,
/// then retry.  The browser subprocess that held the lock restarts automatically.
pub(crate) fn open_db(db_path: &Path) -> Result<rusqlite::Connection, CookieError> {
    use rusqlite::{Connection, OpenFlags};

    #[cfg(not(windows))]
    {
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

    #[cfg(windows)]
    {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        match Connection::open_with_flags(db_path, flags) {
            Ok(conn) => Ok(conn),
            Err(_) => {
                release_file_lock(db_path);
                Connection::open_with_flags(db_path, flags).map_err(CookieError::Sqlite)
            }
        }
    }
}

/// Use the Windows Restart Manager API to release a file lock held by another
/// process (e.g. Chrome/Edge holding the Cookies database).  The browser
/// subprocess that held the lock restarts automatically.
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

        // On Windows, elevate before trying Chromium browsers — the Restart
        // Manager needs admin to release the exclusive cookie DB lock.
        #[cfg(target_os = "windows")]
        if !prompted_elevation && is_chromium {
            prompted_elevation = true;
            if !platform::elevate_if_needed() {
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
