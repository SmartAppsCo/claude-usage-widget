use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_on_top: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_workspaces: Option<bool>,
    /// Exe path that last showed the Chromium permission explanation dialog.
    /// Stored so we don't re-show the dialog on subsequent launches from the
    /// same binary (macOS ties "Always Allow" to the binary path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chromium_prompted_exe: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("SmartAppsCo/claude-usage-widget/config.json"))
}

#[cfg(target_os = "linux")]
fn config_dir() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| crate::cookies::platform::home_dir().map(|h| h.join(".config")))
}

#[cfg(target_os = "macos")]
fn config_dir() -> Option<PathBuf> {
    crate::cookies::platform::home_dir().map(|h| h.join("Library/Application Support"))
}

#[cfg(target_os = "windows")]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}
