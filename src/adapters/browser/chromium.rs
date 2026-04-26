use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rusqlite::Connection;

use crate::adapters::crypto::decrypt_chromium_value;
use crate::domain::{BrowserKind, DomainError, DomainResult, Session, SessionAuth, SessionSource};

pub struct ChromiumSource {
    kind: BrowserKind,
    user_data_dir: PathBuf,
}

impl ChromiumSource {
    pub fn for_kind(kind: BrowserKind) -> Option<Self> {
        let user_data_dir = user_data_dir(kind)?;
        Some(Self {
            kind,
            user_data_dir,
        })
    }
}

#[async_trait]
impl SessionSource for ChromiumSource {
    fn name(&self) -> &'static str {
        self.kind.as_str()
    }

    async fn is_available(&self) -> bool {
        enumerate_profile_dirs(&self.user_data_dir)
            .into_iter()
            .any(|p| cookies_path(&p).is_some())
    }

    async fn extract(&self) -> DomainResult<Option<Session>> {
        let profiles = enumerate_profile_dirs(&self.user_data_dir);
        let mut last_err: Option<DomainError> = None;

        for profile_dir in profiles {
            match try_extract_from(&profile_dir, self.kind) {
                Ok(Some(s)) => return Ok(Some(s)),
                Ok(None) => {}
                Err(e) => {
                    tracing::debug!(
                        browser = self.kind.as_str(),
                        profile = %profile_dir.display(),
                        error = %e,
                        "profile extraction failed"
                    );
                    last_err = Some(e);
                }
            }
        }

        last_err.map_or(Ok(None), Err)
    }
}

fn try_extract_from(profile_dir: &Path, kind: BrowserKind) -> DomainResult<Option<Session>> {
    let Some(cookies_db) = cookies_path(profile_dir) else {
        return Ok(None);
    };

    // Per-profile temp path so concurrent invocations don't race on one file.
    let tmp = std::env::temp_dir().join(format!(
        "kagi_mcp_{}_{}_cookies",
        kind.as_str(),
        profile_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("p")
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    fs::copy(&cookies_db, &tmp).map_err(|e| {
        DomainError::CookieStoreUnavailable(format!("copy {}: {e}", cookies_db.display()))
    })?;

    let conn =
        Connection::open(&tmp).map_err(|e| DomainError::CookieStoreUnavailable(e.to_string()))?;

    let row: Option<(String, Vec<u8>, Vec<u8>)> = conn
        .query_row(
            "SELECT name, value, encrypted_value
             FROM cookies
             WHERE host_key LIKE '%kagi.com'
               AND (name = 'kagi_session' OR name LIKE 'kagi_session%')
             ORDER BY length(encrypted_value) DESC
             LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, Vec<u8>>(2)?)),
        )
        .ok();

    let _ = fs::remove_file(&tmp);

    let Some((name, value_plain, value_enc)) = row else {
        return Ok(None);
    };

    let value = if value_enc.is_empty() {
        String::from_utf8(value_plain)
            .map_err(|e| DomainError::CookieStoreUnavailable(format!("non-utf8 cookie: {e}")))?
    } else {
        let bytes = decrypt_chromium_value(&value_enc, profile_dir)?;
        String::from_utf8(bytes)
            .map_err(|e| DomainError::CookieStoreUnavailable(format!("non-utf8 cookie: {e}")))?
    };

    if value.is_empty() {
        return Ok(None);
    }

    Ok(Some(Session {
        auth: SessionAuth::Cookie { name, value },
        user_agent: chromium_user_agent(kind),
        source: kind,
    }))
}

fn enumerate_profile_dirs(user_data_dir: &Path) -> Vec<PathBuf> {
    let mut default_first: Vec<PathBuf> = Vec::new();
    let mut numbered: Vec<(u32, PathBuf)> = Vec::new();
    let mut other: Vec<PathBuf> = Vec::new();

    if let Some(d) = Some(user_data_dir.join("Default")).filter(|p| p.exists()) {
        default_first.push(d);
    }
    if let Ok(entries) = fs::read_dir(user_data_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "Default" {
                continue;
            }
            if let Some(rest) = name.strip_prefix("Profile ")
                && let Ok(n) = rest.parse::<u32>()
            {
                numbered.push((n, p));
                continue;
            }
            if name.contains("Profile") {
                other.push(p);
            }
        }
    }
    numbered.sort_by_key(|(n, _)| *n);

    let mut out = default_first;
    out.extend(numbered.into_iter().map(|(_, p)| p));
    out.extend(other);
    out
}

fn cookies_path(profile_dir: &Path) -> Option<PathBuf> {
    let new_path = profile_dir.join("Network").join("Cookies");
    if new_path.exists() {
        return Some(new_path);
    }
    let old_path = profile_dir.join("Cookies");
    old_path.exists().then_some(old_path)
}

#[cfg(windows)]
fn user_data_dir(kind: BrowserKind) -> Option<PathBuf> {
    let local = dirs::data_local_dir()?;
    let roaming = dirs::data_dir()?;
    let p = match kind {
        BrowserKind::Chrome => local.join("Google").join("Chrome").join("User Data"),
        BrowserKind::Edge => local.join("Microsoft").join("Edge").join("User Data"),
        BrowserKind::Brave => local
            .join("BraveSoftware")
            .join("Brave-Browser")
            .join("User Data"),
        BrowserKind::Arc => local.join("Arc").join("User Data"),
        BrowserKind::Vivaldi => local.join("Vivaldi").join("User Data"),
        BrowserKind::Opera => roaming.join("Opera Software").join("Opera Stable"),
        BrowserKind::OperaGx => roaming.join("Opera Software").join("Opera GX Stable"),
        BrowserKind::Chromium => local.join("Chromium").join("User Data"),
        _ => return None,
    };
    p.exists().then_some(p)
}

#[cfg(target_os = "macos")]
fn user_data_dir(kind: BrowserKind) -> Option<PathBuf> {
    let app_support = dirs::config_dir()?;
    let p = match kind {
        BrowserKind::Chrome => app_support.join("Google").join("Chrome"),
        BrowserKind::Edge => app_support.join("Microsoft Edge"),
        BrowserKind::Brave => app_support.join("BraveSoftware").join("Brave-Browser"),
        BrowserKind::Arc => app_support.join("Arc").join("User Data"),
        BrowserKind::Vivaldi => app_support.join("Vivaldi"),
        BrowserKind::Opera => app_support.join("com.operasoftware.Opera"),
        BrowserKind::OperaGx => app_support.join("com.operasoftware.OperaGX"),
        BrowserKind::Chromium => app_support.join("Chromium"),
        _ => return None,
    };
    p.exists().then_some(p)
}

#[cfg(target_os = "linux")]
fn user_data_dir(kind: BrowserKind) -> Option<PathBuf> {
    let config = dirs::config_dir()?;
    let p = match kind {
        BrowserKind::Chrome => config.join("google-chrome"),
        BrowserKind::Edge => config.join("microsoft-edge"),
        BrowserKind::Brave => config.join("BraveSoftware").join("Brave-Browser"),
        BrowserKind::Vivaldi => config.join("vivaldi"),
        BrowserKind::Opera => config.join("opera"),
        BrowserKind::Chromium => config.join("chromium"),
        _ => return None,
    };
    p.exists().then_some(p)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn user_data_dir(_kind: BrowserKind) -> Option<PathBuf> {
    None
}

// Per-browser UA. Wildcard arm covers future variants and unsupported kinds
// with the stock Chrome UA, so a new BrowserKind doesn't ship blank.
#[allow(clippy::match_same_arms)]
fn chromium_user_agent(kind: BrowserKind) -> String {
    let chrome_ver = "131.0.0.0";
    let base_os = os_token();
    match kind {
        BrowserKind::Edge => format!(
            "Mozilla/5.0 ({base_os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_ver} Safari/537.36 Edg/{chrome_ver}"
        ),
        BrowserKind::Brave | BrowserKind::Chrome | BrowserKind::Arc | BrowserKind::Chromium => {
            format!(
                "Mozilla/5.0 ({base_os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_ver} Safari/537.36"
            )
        }
        BrowserKind::Vivaldi => format!(
            "Mozilla/5.0 ({base_os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_ver} Safari/537.36 Vivaldi/6.9"
        ),
        BrowserKind::Opera | BrowserKind::OperaGx => format!(
            "Mozilla/5.0 ({base_os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_ver} Safari/537.36 OPR/116.0.0.0"
        ),
        _ => format!(
            "Mozilla/5.0 ({base_os}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome_ver} Safari/537.36"
        ),
    }
}

const fn os_token() -> &'static str {
    if cfg!(windows) {
        "Windows NT 10.0; Win64; x64"
    } else if cfg!(target_os = "macos") {
        "Macintosh; Intel Mac OS X 10_15_7"
    } else {
        "X11; Linux x86_64"
    }
}
