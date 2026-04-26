use std::fs;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData};

use super::chromium_version_prefix;
use crate::domain::{DomainError, DomainResult};

const DPAPI_PREFIX: &[u8] = b"DPAPI";

pub fn decrypt_chromium_value(value: &[u8], profile_root: &Path) -> DomainResult<Vec<u8>> {
    match chromium_version_prefix(value) {
        Some("v10" | "v11") => decrypt_v10_v11(value, profile_root),
        // Chrome 127+ App-Bound Encryption: must be unwrapped by the
        // browser's own elevation service. We can't reach it from here.
        Some("v20") => Err(DomainError::CookieDecryptionBlocked),
        _ => dpapi_unprotect(value),
    }
}

fn decrypt_v10_v11(value: &[u8], profile_root: &Path) -> DomainResult<Vec<u8>> {
    let key = read_chromium_master_key(profile_root)?;

    if value.len() < 3 + 12 + 16 {
        return Err(DomainError::CookieStoreUnavailable(
            "encrypted value too short".into(),
        ));
    }
    let nonce = &value[3..15];
    let ct = &value[15..];

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("aes-gcm decrypt: {e}")))
}

fn read_chromium_master_key(profile_root: &Path) -> DomainResult<Vec<u8>> {
    // Local State sits in the user-data dir, one level above the profile dir.
    let user_data_dir = profile_root
        .parent()
        .ok_or_else(|| DomainError::CookieStoreUnavailable("profile has no parent dir".into()))?;
    let local_state = user_data_dir.join("Local State");
    let raw = fs::read_to_string(&local_state).map_err(|e| {
        DomainError::CookieStoreUnavailable(format!("read {}: {e}", local_state.display()))
    })?;
    let json: Value = serde_json::from_str(&raw)
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("parse Local State: {e}")))?;
    let b64 = json
        .pointer("/os_crypt/encrypted_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            DomainError::CookieStoreUnavailable("os_crypt.encrypted_key missing".into())
        })?;
    let mut wrapped = STANDARD
        .decode(b64)
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("base64 decode key: {e}")))?;
    if !wrapped.starts_with(DPAPI_PREFIX) {
        return Err(DomainError::CookieStoreUnavailable(
            "master key missing DPAPI prefix".into(),
        ));
    }
    let dpapi_blob = wrapped.split_off(DPAPI_PREFIX.len());
    dpapi_unprotect(&dpapi_blob)
}

// Wraps Win32 CryptUnprotectData and the matching LocalFree of its output buffer.
#[allow(unsafe_code)]
fn dpapi_unprotect(input: &[u8]) -> DomainResult<Vec<u8>> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &raw const in_blob,
            None,
            None,
            None,
            None,
            0,
            &raw mut out_blob,
        )
        .map_err(|e| DomainError::CookieStoreUnavailable(format!("CryptUnprotectData: {e}")))?;

        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out_blob.pbData.cast())));
        Ok(slice)
    }
}
