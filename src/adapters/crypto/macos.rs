use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Key, Nonce};
use security_framework::passwords::get_generic_password;

use super::chromium_version_prefix;
use crate::domain::{DomainError, DomainResult};

// Chromium on macOS encrypts with AES-128-GCM using a key derived
// (PBKDF2-HMAC-SHA1, 1003 iters, salt "saltysalt") from a per-browser
// password stored in the login keychain.
pub fn decrypt_chromium_value(value: &[u8], _profile_root: &Path) -> DomainResult<Vec<u8>> {
    match chromium_version_prefix(value) {
        Some("v10") => {
            let pw = keychain_password("Chrome Safe Storage")
                .or_else(|_| keychain_password("Chromium Safe Storage"))
                .or_else(|_| keychain_password("Brave Safe Storage"))
                .or_else(|_| keychain_password("Microsoft Edge Safe Storage"))
                .or_else(|_| keychain_password("Arc Safe Storage"))
                .or_else(|_| keychain_password("Vivaldi Safe Storage"))
                .or_else(|_| keychain_password("Opera Safe Storage"))?;
            let key = derive_key(&pw);
            decrypt_gcm(&key, value)
        }
        _ => Err(DomainError::CookieStoreUnavailable(
            "unsupported chromium cookie format on macOS".into(),
        )),
    }
}

fn keychain_password(service: &str) -> DomainResult<Vec<u8>> {
    get_generic_password(service, "Chrome")
        .or_else(|_| get_generic_password(service, ""))
        .map(|s| s.to_vec())
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("keychain {service}: {e}")))
}

fn derive_key(password: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1003, &mut key);
    key
}

fn decrypt_gcm(key: &[u8; 16], value: &[u8]) -> DomainResult<Vec<u8>> {
    if value.len() < 3 + 12 + 16 {
        return Err(DomainError::CookieStoreUnavailable(
            "encrypted value too short".into(),
        ));
    }
    let nonce = &value[3..15];
    let ct = &value[15..];
    let cipher = Aes128Gcm::new(Key::<Aes128Gcm>::from_slice(key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("aes-gcm: {e}")))
}
