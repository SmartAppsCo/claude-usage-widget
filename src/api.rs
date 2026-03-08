use std::collections::HashMap;

use serde::Deserialize;

use crate::cookies::CookieJar;

#[derive(Deserialize, Clone, Debug)]
pub struct UsageBucket {
    pub utilization: Option<f64>,
    pub resets_at: Option<String>,
}

pub type UsageResponse = HashMap<String, UsageBucket>;
type RawUsageResponse = HashMap<String, Option<UsageBucket>>;

const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:147.0) Gecko/20100101 Firefox/147.0";

fn cookie_header(cookies: &CookieJar) -> String {
    cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn fetch_with_cookies(cookies: &CookieJar) -> Result<UsageResponse, String> {
    if !cookies.contains_key("sessionKey") {
        return Err("No claude.ai session found".into());
    }
    let org_id = cookies
        .get("lastActiveOrg")
        .ok_or("No organization ID in cookies")?;

    let url = format!("https://claude.ai/api/organizations/{org_id}/usage");
    let response = ureq::get(&url)
        .header("Cookie", &cookie_header(cookies))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "*/*")
        .header("Referer", "https://claude.ai/settings/usage")
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Read error: {e}"))?;

    let raw: RawUsageResponse =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;
    Ok(raw.into_iter().filter_map(|(k, v)| v.map(|b| (k, b))).collect())
}

pub fn fetch_account_name(cookies: &CookieJar) -> Result<String, String> {
    let response = ureq::get("https://claude.ai/api/account")
        .header("Cookie", &cookie_header(cookies))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "*/*")
        .header("Referer", "https://claude.ai/settings/general")
        .call()
        .map_err(|e| format!("HTTP error: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Read error: {e}"))?;

    #[derive(Deserialize)]
    struct Account {
        display_name: Option<String>,
        full_name: Option<String>,
    }

    let account: Account =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;
    account
        .display_name
        .filter(|s| !s.is_empty())
        .or(account.full_name.filter(|s| !s.is_empty()))
        .ok_or_else(|| "No name in account response".into())
}
