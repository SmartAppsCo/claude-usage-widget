use std::path::PathBuf;

use crate::cookies::platform::home_dir;
use crate::cookies::{CookieError, CookieJar};

fn cookie_db_path() -> Option<PathBuf> {
    let home = home_dir()?;
    let paths = [
        home.join("Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies"),
        home.join("Library/Cookies/Cookies.binarycookies"),
    ];
    paths.into_iter().find(|p| p.is_file())
}

/// Read a null-terminated string starting at `offset` within `data`.
fn read_cstring(data: &[u8], offset: usize) -> Option<&str> {
    let start = data.get(offset..)?;
    let end = start.iter().position(|&b| b == 0)?;
    std::str::from_utf8(&start[..end]).ok()
}

fn u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// Parse a `.binarycookies` file and return cookies matching `domain`.
fn parse_binary_cookies(data: &[u8], domain: &str) -> Result<CookieJar, CookieError> {
    if data.len() < 8 || &data[..4] != b"cook" {
        return Err(CookieError::NoCookieDb);
    }

    let num_pages = u32_be(data, 4) as usize;
    if data.len() < 8 + num_pages * 4 {
        return Err(CookieError::NoCookieDb);
    }

    // Read page sizes.
    let mut page_sizes = Vec::with_capacity(num_pages);
    for i in 0..num_pages {
        page_sizes.push(u32_be(data, 8 + i * 4) as usize);
    }

    let mut jar = CookieJar::new();
    let mut page_offset = 8 + num_pages * 4;

    for &page_size in &page_sizes {
        if page_offset + page_size > data.len() {
            break;
        }
        let page = &data[page_offset..page_offset + page_size];

        // Page header: 0x00000100, then cookie count and offsets.
        if page.len() < 8 {
            page_offset += page_size;
            continue;
        }
        let num_cookies = u32_le(page, 4) as usize;
        if page.len() < 8 + num_cookies * 4 {
            page_offset += page_size;
            continue;
        }

        for i in 0..num_cookies {
            let record_offset = u32_le(page, 8 + i * 4) as usize;
            if record_offset + 56 > page.len() {
                continue;
            }
            let record = &page[record_offset..];

            let url_offset = u32_le(record, 16) as usize;
            let name_offset = u32_le(record, 20) as usize;
            let value_offset = u32_le(record, 28) as usize;

            let url = match read_cstring(record, url_offset) {
                Some(s) => s,
                None => continue,
            };

            if !url.contains(domain) {
                continue;
            }

            let name = match read_cstring(record, name_offset) {
                Some(s) => s,
                None => continue,
            };
            let value = match read_cstring(record, value_offset) {
                Some(s) => s,
                None => continue,
            };

            if !value.is_empty() {
                jar.insert(name.to_owned(), value.to_owned());
            }
        }

        page_offset += page_size;
    }

    Ok(jar)
}

pub fn read(domain: &str) -> Result<CookieJar, CookieError> {
    let db_path = cookie_db_path().ok_or(CookieError::NoCookieDb)?;
    let data = std::fs::read(&db_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            CookieError::PermissionDenied
        } else {
            CookieError::NoCookieDb
        }
    })?;
    parse_binary_cookies(&data, domain)
}
