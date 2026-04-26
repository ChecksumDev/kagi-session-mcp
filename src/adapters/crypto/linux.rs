use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Key, Nonce};

use super::chromium_version_prefix;
use crate::domain::{DomainError, DomainResult};

// Linux Chromium with `--password-store=basic` uses the hard-coded
// password "peanuts". libsecret-backed v11 isn't yet supported here
// because adding a libsecret link would pull in another C dep; users
// in that situation should provide a Session Link via env var instead.
pub fn decrypt_chromium_value(value: &[u8], _profile_root: &Path) -> DomainResult<Vec<u8>> {
    match chromium_version_prefix(value) {
        Some("v10") => decrypt_with_password(value, b"peanuts"),
        Some("v11") => Err(DomainError::CookieStoreUnavailable(
            "linux v11 requires libsecret (gnome-keyring/kwallet); use a Session Link instead"
                .into(),
        )),
        _ => Err(DomainError::CookieStoreUnavailable(
            "unknown chromium cookie format".into(),
        )),
    }
}

fn decrypt_with_password(value: &[u8], password: &[u8]) -> DomainResult<Vec<u8>> {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1, &mut key);
    if value.len() < 3 + 12 + 16 {
        return Err(DomainError::CookieStoreUnavailable(
            "encrypted value too short".into(),
        ));
    }
    let nonce = &value[3..15];
    let ct = &value[15..];
    let cipher = Aes128Gcm::new(Key::<Aes128Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("aes-gcm: {e}")))
}
