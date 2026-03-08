use std::path::PathBuf;

use crate::cookies::CookieError;

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
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
    let p = PathBuf::from(local).join("Google/Chrome/User Data");
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
    let p = PathBuf::from(local).join("BraveSoftware/Brave-Browser/User Data");
    if p.is_dir() {
        vec![p]
    } else {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Chrome/Brave encryption key (needed on Windows for AES-256-GCM)
// ---------------------------------------------------------------------------

/// On Windows, reads the AES-256-GCM key from Chrome's `Local State` file
/// and decrypts it via DPAPI. On other platforms, the key is derived at
/// decrypt time so this returns `None`.
#[cfg(not(target_os = "windows"))]
pub fn chrome_encryption_key(_db_path: &std::path::Path) -> Option<Vec<u8>> {
    None
}

#[cfg(target_os = "windows")]
pub fn chrome_encryption_key(db_path: &std::path::Path) -> Option<Vec<u8>> {
    use base64::Engine;
    use windows::Win32::Security::Cryptography::*;

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
    let encrypted_key_b64 = json["os_crypt"]["encrypted_key"].as_str()?;
    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .ok()?;

    // Strip "DPAPI" prefix (5 bytes).
    let dpapi_blob = encrypted_key.strip_prefix(b"DPAPI" as &[u8])?;

    // Decrypt via DPAPI.
    let input = CRYPT_INTEGER_BLOB {
        cbData: dpapi_blob.len() as u32,
        pbData: dpapi_blob.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output).ok()?;
        let key = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        // Free the DPAPI-allocated buffer (ignore errors on cleanup).
        let _ = windows::Win32::Foundation::LocalFree(output.pbData as _);
        Some(key)
    }
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

    let password = ["Chrome Safe Storage", "Brave Safe Storage", "Chromium Safe Storage"]
        .iter()
        .find_map(|svc| get_generic_password(None, svc).ok())
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
    if encrypted.len() < 3 || (encrypted[..3] != *b"v10" && encrypted[..3] != *b"v11") {
        return Ok(String::from_utf8_lossy(encrypted).into_owned());
    }

    let key = key.ok_or_else(|| CookieError::Decrypt("No encryption key available".into()))?;

    // v10/v11 prefix (3 bytes), then 12-byte nonce, then ciphertext + 16-byte GCM tag.
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

    String::from_utf8(plaintext)
        .map_err(|e| CookieError::Decrypt(format!("UTF-8 decode failed: {e}")))
}
