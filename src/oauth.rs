use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthCreds>,
}

#[derive(Deserialize)]
struct OAuthCreds {
    #[serde(rename = "accessToken")]
    access_token: String,
}

/// Read the Claude Code OAuth access token.
///
/// If `oauth_dir` is provided, reads from `<oauth_dir>/.credentials.json`.
/// Otherwise defaults to `~/.claude/.credentials.json`.
pub fn read_access_token(oauth_dir: Option<&str>) -> Option<String> {
    let path = match oauth_dir {
        Some(dir) => PathBuf::from(dir).join(".credentials.json"),
        None => home_dir()?.join(".claude/.credentials.json"),
    };
    let contents = std::fs::read_to_string(path).ok()?;
    let file: CredentialsFile = serde_json::from_str(&contents).ok()?;
    Some(file.claude_ai_oauth?.access_token)
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
}
