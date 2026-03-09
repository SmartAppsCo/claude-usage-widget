use std::path::PathBuf;

use crate::cookies::CookieError;

#[cfg(not(target_os = "windows"))]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// ---------------------------------------------------------------------------
// Firefox default directory
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn firefox_default_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".mozilla/firefox"))
}

#[cfg(target_os = "macos")]
pub fn firefox_default_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join("Library/Application Support/Firefox/Profiles"))
}

#[cfg(target_os = "windows")]
pub fn firefox_default_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Mozilla/Firefox/Profiles"))
}

// ---------------------------------------------------------------------------
// Chrome default directories
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn chrome_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    [
        ".config/google-chrome",
        ".config/chromium",
        "snap/chromium/common/chromium",
        ".var/app/com.google.Chrome/config/google-chrome",
        ".var/app/org.chromium.Chromium/config/chromium",
    ]
    .iter()
    .map(|p| home.join(p))
    .filter(|p| p.is_dir())
    .collect()
}

#[cfg(target_os = "macos")]
pub fn chrome_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    let p = home.join("Library/Application Support/Google/Chrome");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

#[cfg(target_os = "windows")]
pub fn chrome_default_dirs() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return vec![];
    };
    let p = PathBuf::from(local).join("Google").join("Chrome").join("User Data");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Brave default directories
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn brave_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    [
        ".config/BraveSoftware/Brave-Browser",
        "snap/brave/current/.config/BraveSoftware/Brave-Browser",
        ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
    ]
    .iter()
    .map(|p| home.join(p))
    .filter(|p| p.is_dir())
    .collect()
}

#[cfg(target_os = "macos")]
pub fn brave_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    let p = home.join("Library/Application Support/BraveSoftware/Brave-Browser");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

#[cfg(target_os = "windows")]
pub fn brave_default_dirs() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return vec![];
    };
    let p = PathBuf::from(local).join("BraveSoftware").join("Brave-Browser").join("User Data");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Edge default directories
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn edge_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    let p = home.join(".config/microsoft-edge");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

#[cfg(target_os = "macos")]
pub fn edge_default_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    let p = home.join("Library/Application Support/Microsoft Edge");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

#[cfg(target_os = "windows")]
pub fn edge_default_dirs() -> Vec<PathBuf> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return vec![];
    };
    let p = PathBuf::from(local).join("Microsoft").join("Edge").join("User Data");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Chrome/Brave/Edge encryption key (needed on Windows for AES-256-GCM)
// ---------------------------------------------------------------------------

/// On Windows, reads the AES-256-GCM key from Chrome's `Local State` file
/// and decrypts it via DPAPI. On other platforms, the key is derived at
/// decrypt time so this returns `None`.
#[cfg(not(target_os = "windows"))]
pub fn chrome_encryption_key(_db_path: &std::path::Path) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "windows")]
fn dpapi_decrypt(blob: &[u8]) -> Option<Vec<u8>> {
    use windows::Win32::Security::Cryptography::*;

    let input = CRYPT_INTEGER_BLOB {
        cbData: blob.len() as u32,
        pbData: blob.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output).ok()?;
        let key = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = windows::Win32::Foundation::LocalFree(Some(
            windows::Win32::Foundation::HLOCAL(output.pbData as _),
        ));
        Some(key)
    }
}

/// Impersonate SYSTEM by duplicating the token of lsass.exe or winlogon.exe.
/// Returns the duplicated token handle on success.  Requires admin.
#[cfg(target_os = "windows")]
fn impersonate_system() -> Option<windows::Win32::Foundation::HANDLE> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::*;
    use windows::Win32::Security::*;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::Threading::*;

    // Enable SeDebugPrivilege.
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlAdjustPrivilege(
            privilege: i32,
            enable: i32,
            current_thread: i32,
            previous_value: *mut i32,
        ) -> i32;
    }
    let mut prev: i32 = 0;
    // SE_DEBUG_PRIVILEGE = 20
    unsafe { RtlAdjustPrivilege(20, 1, 0, &mut prev) };

    // Find lsass.exe or winlogon.exe.
    let mut pids = vec![0u32; 4096];
    let mut needed: u32 = 0;
    unsafe { EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut needed).ok()? };
    pids.truncate((needed / 4) as usize);

    let mut target_pid = None;
    for &pid in &pids {
        let Ok(h) = (unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) }) else {
            continue;
        };
        let mut buf = [0u16; 260];
        let len = unsafe { K32GetProcessImageFileNameW(h, &mut buf) } as usize;
        let _ = unsafe { CloseHandle(h) };
        if len == 0 { continue; }
        let name = OsString::from_wide(&buf[..len]);
        let name = name.to_string_lossy();
        if name.ends_with("lsass.exe") {
            target_pid = Some(pid);
            break;
        }
        if name.ends_with("winlogon.exe") && target_pid.is_none() {
            target_pid = Some(pid);
        }
    }

    let pid = target_pid?;
    let proc_h = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, pid).ok()? };
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(proc_h, TOKEN_DUPLICATE | TOKEN_QUERY, &mut token).ok()? };
    let _ = unsafe { CloseHandle(proc_h) };

    let mut dup_token = HANDLE::default();
    unsafe {
        DuplicateToken(token, SecurityImpersonation, &mut dup_token).ok()?;
        CloseHandle(token).ok()?;
        ImpersonateLoggedOnUser(dup_token).ok()?;
    }
    Some(dup_token)
}

#[cfg(target_os = "windows")]
fn stop_impersonate(token: windows::Win32::Foundation::HANDLE) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::RevertToSelf;
    unsafe {
        let _ = CloseHandle(token);
        let _ = RevertToSelf();
    }
}

/// Derive the v20 (App-Bound Encryption) key.  Requires admin.
/// Flow: base64 decode → strip "APPB" → DPAPI-as-SYSTEM → DPAPI-as-user → extract key.
#[cfg(target_os = "windows")]
fn appbound_encryption_key(app_bound_key_b64: &str) -> Option<Vec<u8>> {
    use base64::Engine;

    let raw = base64::engine::general_purpose::STANDARD
        .decode(app_bound_key_b64)
        .ok()?;
    let without_prefix = raw.strip_prefix(b"APPB")?;

    // First DPAPI decrypt as SYSTEM.
    let system_token = impersonate_system()?;
    let system_decrypted = dpapi_decrypt(without_prefix);
    stop_impersonate(system_token);
    let system_decrypted = system_decrypted?;

    // Second DPAPI decrypt as user.
    let user_decrypted = dpapi_decrypt(&system_decrypted)?;

    if user_decrypted.len() < 61 {
        return None;
    }

    // The last 32 bytes of the user-decrypted result is the AES key.
    let key = user_decrypted[user_decrypted.len() - 32..].to_vec();
    Some(key)
}

#[cfg(target_os = "windows")]
pub fn chrome_encryption_key(db_path: &std::path::Path) -> Option<Vec<u8>> {
    use base64::Engine;

    // Walk up from the cookie DB to find the Local State file.
    let local_state_path = {
        let mut dir = db_path.parent()?;
        loop {
            let ls = dir.join("Local State");
            if ls.is_file() {
                break ls;
            }
            dir = dir.parent()?;
        }
    };

    let content = std::fs::read_to_string(&local_state_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try App-Bound Encryption key first (Chrome/Edge 127+, v20 cookies).
    if let Some(abk) = json["os_crypt"]["app_bound_encrypted_key"].as_str() {
        if let Some(key) = appbound_encryption_key(abk) {
            return Some(key);
        }
    }

    // Fall back to legacy DPAPI key (v10/v11 cookies).
    let encrypted_key_b64 = json["os_crypt"]["encrypted_key"].as_str()?;
    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .ok()?;
    let dpapi_blob = encrypted_key.strip_prefix(b"DPAPI" as &[u8])?;
    dpapi_decrypt(dpapi_blob)
}

// ---------------------------------------------------------------------------
// Chrome/Brave cookie decryption
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn try_decrypt(ciphertext: &[u8], password: &[u8], iterations: u32) -> Option<String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    use pbkdf2::pbkdf2_hmac;
    use sha1::Sha1;

    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(password, b"saltysalt", iterations, &mut key);
    let iv = [0x20u8; 16];

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
        .ok()?;

    // Chrome 130+ prepends SHA256(host_key) (32 bytes) to the plaintext before
    // encrypting.  If the first 32 bytes look like binary hash data, strip them.
    let payload = if decrypted.len() > 32 && decrypted[..32].iter().any(|&b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r') {
        &decrypted[32..]
    } else {
        decrypted
    };

    String::from_utf8(payload.to_vec()).ok()
}

#[cfg(target_os = "linux")]
fn get_keyring_password() -> Option<&'static [u8]> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    CACHED.get_or_init(|| {
        for app in ["chrome", "chromium", "brave"] {
            let Ok(output) = std::process::Command::new("secret-tool")
                .args(["lookup", "application", app])
                .output()
            else {
                continue;
            };
            if output.status.success() && !output.stdout.is_empty() {
                return Some(output.stdout.trim_ascii().to_vec());
            }
        }
        None
    }).as_deref()
}

#[cfg(target_os = "linux")]
pub fn decrypt_chrome_value(encrypted: &[u8], _key: Option<&[u8]>) -> Result<String, CookieError> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }
    if encrypted.len() < 3 || (encrypted[..3] != *b"v10" && encrypted[..3] != *b"v11") {
        return Ok(String::from_utf8_lossy(encrypted).into_owned());
    }

    let ciphertext = &encrypted[3..];

    // Try hardcoded "peanuts" password first (no-keyring fallback, 1 iteration)
    if let Some(s) = try_decrypt(ciphertext, b"peanuts", 1) {
        return Ok(s);
    }

    // Try keyring password (used when GNOME Keyring / KDE Wallet is available)
    if let Some(password) = get_keyring_password() {
        if let Some(s) = try_decrypt(ciphertext, password, 1) {
            return Ok(s);
        }
    }

    Err(CookieError::Decrypt(
        "Could not decrypt Chrome cookie (tried peanuts + keyring)".into(),
    ))
}

#[cfg(target_os = "macos")]
pub fn decrypt_chrome_value(encrypted: &[u8], _key: Option<&[u8]>) -> Result<String, CookieError> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    use pbkdf2::pbkdf2_hmac;
    use security_framework::passwords::get_generic_password;
    use sha1::Sha1;

    if encrypted.is_empty() {
        return Ok(String::new());
    }
    if encrypted.len() < 3 || (encrypted[..3] != *b"v10" && encrypted[..3] != *b"v11") {
        return Ok(String::from_utf8_lossy(encrypted).into_owned());
    }

    let ciphertext = &encrypted[3..];

    let password = [
        ("Chrome Safe Storage", "Chrome"),
        ("Brave Safe Storage", "Brave"),
        ("Chromium Safe Storage", "Chromium"),
        ("Microsoft Edge Safe Storage", "Microsoft Edge"),
    ]
        .iter()
        .find_map(|(svc, acct)| get_generic_password(svc, acct).ok())
        .ok_or_else(|| CookieError::Decrypt("Keychain lookup failed for all browsers".into()))?;
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(&password, b"saltysalt", 1003, &mut key);
    let iv = [0x20u8; 16];

    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
    let mut buf = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
        .map_err(|e| CookieError::Decrypt(format!("AES decrypt failed: {e}")))?;

    // Chrome 130+ prepends SHA256(host_key) (32 bytes) to the plaintext.
    let payload = if decrypted.len() > 32 && decrypted[..32].iter().any(|&b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r') {
        &decrypted[32..]
    } else {
        decrypted
    };

    String::from_utf8(payload.to_vec())
        .map_err(|e| CookieError::Decrypt(format!("UTF-8 decode failed: {e}")))
}

#[cfg(target_os = "windows")]
pub fn decrypt_chrome_value(encrypted: &[u8], key: Option<&[u8]>) -> Result<String, CookieError> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }
    if encrypted.len() < 3
        || (encrypted[..3] != *b"v10" && encrypted[..3] != *b"v11" && encrypted[..3] != *b"v20")
    {
        return Ok(String::from_utf8_lossy(encrypted).into_owned());
    }

    let key = key.ok_or_else(|| CookieError::Decrypt("No encryption key available".into()))?;

    // v10/v11/v20 prefix (3 bytes), then 12-byte nonce, then ciphertext + 16-byte GCM tag.
    let payload = &encrypted[3..];
    if payload.len() < 12 + 16 {
        return Err(CookieError::Decrypt("Encrypted value too short".into()));
    }
    let (nonce, ciphertext) = payload.split_at(12);

    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CookieError::Decrypt(format!("Invalid AES key: {e}")))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| CookieError::Decrypt(format!("AES-GCM decrypt failed: {e}")))?;

    // Chrome 130+ prepends SHA256(host_key) (32 bytes) to the plaintext.
    let payload = if plaintext.len() > 32 && plaintext[..32].iter().any(|&b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r') {
        &plaintext[32..]
    } else {
        &plaintext
    };

    String::from_utf8(payload.to_vec())
        .map_err(|e| CookieError::Decrypt(format!("UTF-8 decode failed: {e}")))
}

// ---------------------------------------------------------------------------
// Windows: prompt for elevation and re-launch via UAC
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// Elevate to admin if not already elevated.  Needed on Windows because
/// Chrome/Edge hold exclusive locks on cookie databases — the Restart Manager
/// requires admin privileges to release them.
///
/// Returns true if already elevated.  If not, shows an explanation dialog,
/// re-launches via UAC, and exits the current process (never returns).
/// Returns false only if the user cancels.
#[cfg(target_os = "windows")]
pub fn elevate_if_needed() -> bool {
    if is_elevated() {
        return true;
    }

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::*;

    let msg = "Chrome, Edge, and Brave lock their cookie databases and use \
               App-Bound Encryption. Claude Usage needs administrator access \
               to read and decrypt them.\n\n\
               Windows will prompt for permission next.";
    let text: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = "Claude Usage\0".encode_utf16().collect();
    let result = unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OKCANCEL | MB_ICONINFORMATION,
        )
    };
    if result != IDOK {
        return false;
    }

    // Re-launch elevated via UAC.
    let exe = std::env::current_exe().unwrap_or_default();
    let args: String = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        let r = ShellExecuteW(
            None,
            &HSTRING::from("runas"),
            &HSTRING::from(exe.as_os_str()),
            &HSTRING::from(&args),
            None,
            SW_SHOWNORMAL,
        );
        if r.0 as usize > 32 {
            std::process::exit(0); // original process exits
        }
    }
    false // UAC was declined or failed
}

// ---------------------------------------------------------------------------
// macOS native dialogs (via osascript)
// ---------------------------------------------------------------------------

/// Show a native macOS dialog via osascript. Returns true if the user clicked
/// the default (right) button, false if they clicked cancel or dismissed it.
#[cfg(target_os = "macos")]
fn show_macos_dialog(message: &str, buttons: (&str, &str)) -> bool {
    let script = format!(
        "display dialog {:?} buttons {{{:?}, {:?}}} default button {:?} with icon caution with title \"Claude Usage\"",
        message, buttons.0, buttons.1, buttons.1
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Prompt for Safari Full Disk Access. Returns true if user wants to open settings.
#[cfg(target_os = "macos")]
pub fn prompt_full_disk_access() -> bool {
    let clicked = show_macos_dialog(
        "Claude Usage needs Full Disk Access to read Safari cookies.\n\n\
         Go to System Settings → Privacy & Security → Full Disk Access and add this app.",
        ("Cancel", "Open Settings"),
    );
    if clicked {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .spawn();
    }
    clicked
}

/// Warn the user before the macOS Keychain prompt for Chrome/Brave/Edge.
/// Returns true if user wants to continue, false to skip this browser.
#[cfg(target_os = "macos")]
pub fn prompt_keychain_access() -> bool {
    show_macos_dialog(
        "Claude Usage needs to access the Chrome/Edge/Brave keychain to decrypt cookies.\n\n\
         macOS will prompt for your login password next. \
         Click \"Always Allow\" so you only have to do this once.",
        ("Cancel", "Continue"),
    )
}
