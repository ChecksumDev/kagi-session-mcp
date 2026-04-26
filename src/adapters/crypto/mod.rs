#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
use crate::domain::{DomainError, DomainResult};

#[cfg(target_os = "linux")]
pub use linux::decrypt_chromium_value;
#[cfg(target_os = "macos")]
pub use macos::decrypt_chromium_value;
#[cfg(windows)]
pub use windows::decrypt_chromium_value;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn decrypt_chromium_value(
    _value: &[u8],
    _profile_root: &std::path::Path,
) -> DomainResult<Vec<u8>> {
    Err(DomainError::CookieStoreUnavailable(
        "unsupported platform for chromium cookie decryption".into(),
    ))
}

pub fn chromium_version_prefix(value: &[u8]) -> Option<&str> {
    if value.len() < 3 {
        return None;
    }
    std::str::from_utf8(&value[..3]).ok()
}
