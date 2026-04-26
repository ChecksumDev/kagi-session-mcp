use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::{BrowserKind, DomainError, DomainResult, Session, SessionAuth, SessionSource};

pub struct SafariSource;

impl SafariSource {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SessionSource for SafariSource {
    fn name(&self) -> &'static str {
        "safari"
    }

    async fn is_available(&self) -> bool {
        cookies_path().map(|p| p.exists()).unwrap_or(false)
    }

    async fn extract(&self) -> DomainResult<Option<Session>> {
        let Some(path) = cookies_path() else {
            return Ok(None);
        };
        let bytes = fs::read(&path).map_err(|e| {
            DomainError::CookieStoreUnavailable(format!("read {}: {e}", path.display()))
        })?;
        match parse_binary_cookies(&bytes) {
            Ok(cookies) => {
                for c in cookies {
                    if c.domain.ends_with("kagi.com") && c.name.starts_with("kagi_session") {
                        return Ok(Some(Session {
                            auth: SessionAuth::Cookie {
                                name: c.name,
                                value: c.value,
                            },
                            user_agent: safari_user_agent(),
                            source: BrowserKind::Safari,
                        }));
                    }
                }
                Ok(None)
            }
            Err(e) => Err(DomainError::CookieStoreUnavailable(e)),
        }
    }
}

fn cookies_path() -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join("Library")
            .join("Cookies")
            .join("Cookies.binarycookies"),
    )
}

fn safari_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15".to_string()
}

struct BinCookie {
    name: String,
    value: String,
    domain: String,
}

// Safari Cookies.binarycookies format: header magic "cook" + page count (BE u32),
// per-page sizes (BE u32), then pages. Each page: magic 0x00000100, cookie count
// (LE u32), per-cookie offsets (LE u32), 4 zero bytes, then cookie records.
fn parse_binary_cookies(data: &[u8]) -> Result<Vec<BinCookie>, String> {
    if data.len() < 8 || &data[..4] != b"cook" {
        return Err("not a Cookies.binarycookies file".into());
    }
    let page_count = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
    let mut cursor = 8;
    let mut page_sizes = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        if cursor + 4 > data.len() {
            return Err("truncated page-size table".into());
        }
        page_sizes.push(u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize);
        cursor += 4;
    }

    let mut out = Vec::new();
    for size in page_sizes {
        if cursor + size > data.len() {
            return Err("truncated page".into());
        }
        let page = &data[cursor..cursor + size];
        cursor += size;
        parse_page(page, &mut out)?;
    }
    Ok(out)
}

fn parse_page(page: &[u8], out: &mut Vec<BinCookie>) -> Result<(), String> {
    if page.len() < 8 || &page[..4] != [0x00, 0x00, 0x01, 0x00] {
        return Err("bad page header".into());
    }
    let cookie_count = u32::from_le_bytes(page[4..8].try_into().unwrap()) as usize;
    if page.len() < 8 + cookie_count * 4 {
        return Err("truncated cookie offset table".into());
    }
    let mut offsets = Vec::with_capacity(cookie_count);
    for i in 0..cookie_count {
        let o = u32::from_le_bytes(page[8 + i * 4..12 + i * 4].try_into().unwrap()) as usize;
        offsets.push(o);
    }
    for off in offsets {
        if off + 56 > page.len() {
            continue;
        }
        let rec = &page[off..];
        let dom_off = u32::from_le_bytes(rec[16..20].try_into().unwrap()) as usize;
        let name_off = u32::from_le_bytes(rec[20..24].try_into().unwrap()) as usize;
        let _path_off = u32::from_le_bytes(rec[24..28].try_into().unwrap()) as usize;
        let val_off = u32::from_le_bytes(rec[28..32].try_into().unwrap()) as usize;

        let domain = read_cstr(rec, dom_off)?;
        let name = read_cstr(rec, name_off)?;
        let value = read_cstr(rec, val_off)?;
        out.push(BinCookie {
            name,
            value,
            domain,
        });
    }
    Ok(())
}

fn read_cstr(buf: &[u8], offset: usize) -> Result<String, String> {
    if offset >= buf.len() {
        return Err("string offset out of bounds".into());
    }
    let slice = &buf[offset..];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8(slice[..end].to_vec()).map_err(|e| format!("utf8: {e}"))
}
