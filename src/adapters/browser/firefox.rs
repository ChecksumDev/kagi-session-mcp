use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rusqlite::Connection;

use crate::domain::{BrowserKind, DomainResult, Session, SessionAuth, SessionSource};

pub struct FirefoxLikeSource {
    label: &'static str,
    profiles_roots: Vec<PathBuf>,
}

impl FirefoxLikeSource {
    const fn new(label: &'static str, profiles_roots: Vec<PathBuf>) -> Self {
        Self {
            label,
            profiles_roots,
        }
    }
}

#[async_trait]
impl SessionSource for FirefoxLikeSource {
    fn name(&self) -> &'static str {
        self.label
    }

    async fn is_available(&self) -> bool {
        self.profiles_roots.iter().any(|p| p.exists())
    }

    async fn extract(&self) -> DomainResult<Option<Session>> {
        for root in &self.profiles_roots {
            if !root.exists() {
                continue;
            }
            for profile in enumerate_profiles(root) {
                if let Some(session) = try_profile(&profile, self.label) {
                    return Ok(Some(session));
                }
            }
        }
        Ok(None)
    }
}

fn try_profile(profile: &Path, label: &'static str) -> Option<Session> {
    let cookies_db = profile.join("cookies.sqlite");
    if !cookies_db.exists() {
        return None;
    }
    let tmp = std::env::temp_dir().join(format!(
        "kagi_mcp_{}_{}_cookies.sqlite",
        label,
        profile
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("p")
            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    if fs::copy(&cookies_db, &tmp).is_err() {
        return None;
    }
    let Ok(conn) = Connection::open(&tmp) else {
        let _ = fs::remove_file(&tmp);
        return None;
    };
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT name, value
             FROM moz_cookies
             WHERE host LIKE '%kagi.com'
               AND (name = 'kagi_session' OR name LIKE 'kagi_session%')
             ORDER BY length(value) DESC
             LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let _ = fs::remove_file(&tmp);

    if let Some((name, value)) = row
        && !value.is_empty()
    {
        return Some(Session {
            auth: SessionAuth::Cookie { name, value },
            user_agent: firefox_user_agent(label),
            source: kind_for_label(label),
        });
    }
    None
}

fn enumerate_profiles(root: &Path) -> Vec<PathBuf> {
    let mut priority = Vec::new();
    let mut other = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return priority;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.ends_with(".default-release")
            || name.ends_with(".Default (release)")
            || name.ends_with(".default")
            || name.contains("Default")
        {
            priority.push(p);
        } else {
            other.push(p);
        }
    }
    priority.extend(other);
    priority
}

// Reserved for per-fork BrowserKind variants; today every fork maps to Firefox.
#[allow(clippy::match_same_arms, clippy::needless_pass_by_value)]
fn kind_for_label(label: &'static str) -> BrowserKind {
    match label {
        "firefox" => BrowserKind::Firefox,
        _ => BrowserKind::Firefox,
    }
}

fn firefox_user_agent(label: &'static str) -> String {
    let os = if cfg!(windows) {
        "Windows NT 10.0; Win64; x64; rv:131.0"
    } else if cfg!(target_os = "macos") {
        "Macintosh; Intel Mac OS X 10.15; rv:131.0"
    } else {
        "X11; Linux x86_64; rv:131.0"
    };
    let _ = label;
    format!("Mozilla/5.0 ({os}) Gecko/20100101 Firefox/131.0")
}

pub fn all_firefox_like() -> Vec<FirefoxLikeSource> {
    let mut out = Vec::new();
    for (label, roots) in firefox_like_roots() {
        let existing: Vec<PathBuf> = roots.into_iter().filter(|p| p.exists()).collect();
        if !existing.is_empty() {
            out.push(FirefoxLikeSource::new(label, existing));
        }
    }
    out
}

#[cfg(windows)]
fn firefox_like_roots() -> Vec<(&'static str, Vec<PathBuf>)> {
    let appdata = dirs::data_dir();
    let local = dirs::data_local_dir();
    let join_profiles = |base: &Path, vendor: &str| base.join(vendor).join("Profiles");
    let mut v = Vec::new();
    let push = |v: &mut Vec<(&'static str, Vec<PathBuf>)>,
                label: &'static str,
                paths: Vec<Option<PathBuf>>| {
        v.push((label, paths.into_iter().flatten().collect()));
    };
    push(
        &mut v,
        "firefox",
        vec![
            appdata
                .as_ref()
                .map(|p| join_profiles(p, "Mozilla/Firefox")),
        ],
    );
    push(
        &mut v,
        "zen",
        vec![
            appdata.as_ref().map(|p| p.join("zen").join("Profiles")),
            local.as_ref().map(|p| p.join("zen").join("Profiles")),
        ],
    );
    push(
        &mut v,
        "librewolf",
        vec![
            appdata.as_ref().map(|p| join_profiles(p, "librewolf")),
            appdata.as_ref().map(|p| join_profiles(p, "LibreWolf")),
        ],
    );
    push(
        &mut v,
        "waterfox",
        vec![appdata.as_ref().map(|p| join_profiles(p, "Waterfox"))],
    );
    push(
        &mut v,
        "floorp",
        vec![appdata.as_ref().map(|p| join_profiles(p, "Floorp"))],
    );
    push(
        &mut v,
        "mullvad",
        vec![
            appdata
                .as_ref()
                .map(|p| join_profiles(p, "Mullvad/MullvadBrowser")),
        ],
    );
    push(
        &mut v,
        "tor",
        vec![
            local
                .as_ref()
                .map(|p| join_profiles(p, "Tor Browser/Browser/TorBrowser/Data/Browser")),
        ],
    );
    v
}

#[cfg(target_os = "macos")]
fn firefox_like_roots() -> Vec<(&'static str, Vec<PathBuf>)> {
    let support = dirs::config_dir();
    let join = |base: &Path, vendor: &str| base.join(vendor).join("Profiles");
    let mut v: Vec<(&'static str, Vec<PathBuf>)> = Vec::new();
    let push = |v: &mut Vec<(&'static str, Vec<PathBuf>)>, label, paths: Vec<Option<PathBuf>>| {
        v.push((label, paths.into_iter().flatten().collect()));
    };
    push(
        &mut v,
        "firefox",
        vec![support.as_ref().map(|p| join(p, "Firefox"))],
    );
    push(
        &mut v,
        "zen",
        vec![support.as_ref().map(|p| join(p, "zen"))],
    );
    push(
        &mut v,
        "librewolf",
        vec![support.as_ref().map(|p| join(p, "LibreWolf"))],
    );
    push(
        &mut v,
        "waterfox",
        vec![support.as_ref().map(|p| join(p, "Waterfox"))],
    );
    push(
        &mut v,
        "floorp",
        vec![support.as_ref().map(|p| join(p, "Floorp"))],
    );
    push(
        &mut v,
        "mullvad",
        vec![support.as_ref().map(|p| join(p, "MullvadBrowser"))],
    );
    v
}

#[cfg(target_os = "linux")]
fn firefox_like_roots() -> Vec<(&'static str, Vec<PathBuf>)> {
    let home = dirs::home_dir();
    let config = dirs::config_dir();
    let mut v: Vec<(&'static str, Vec<PathBuf>)> = Vec::new();
    let push = |v: &mut Vec<(&'static str, Vec<PathBuf>)>, label, paths: Vec<Option<PathBuf>>| {
        v.push((label, paths.into_iter().flatten().collect()));
    };
    push(
        &mut v,
        "firefox",
        vec![home.as_ref().map(|p| p.join(".mozilla").join("firefox"))],
    );
    push(
        &mut v,
        "zen",
        vec![
            home.as_ref().map(|p| p.join(".zen")),
            config.as_ref().map(|p| p.join("zen")),
        ],
    );
    push(
        &mut v,
        "librewolf",
        vec![
            home.as_ref().map(|p| p.join(".librewolf")),
            config.as_ref().map(|p| p.join("librewolf")),
        ],
    );
    push(
        &mut v,
        "waterfox",
        vec![home.as_ref().map(|p| p.join(".waterfox"))],
    );
    push(
        &mut v,
        "floorp",
        vec![home.as_ref().map(|p| p.join(".floorp"))],
    );
    v
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn firefox_like_roots() -> Vec<(&'static str, Vec<PathBuf>)> {
    Vec::new()
}
