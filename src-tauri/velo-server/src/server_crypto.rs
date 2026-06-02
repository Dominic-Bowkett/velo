//! Server-side secret encryption for stored IMAP/SMTP passwords.
//!
//! Credentials are encrypted with a key that lives ONLY on the server
//! (`VELO_SECRET_KEY` as base64, or a generated `velo-secret.key` file next to
//! the control DB). The browser never receives the key or the plaintext
//! password — the server decrypts only when running a mail operation.
//!
//! Format matches the desktop `crypto.ts`: AES-256-GCM, output `iv:ciphertext`
//! where both parts are base64 (standard) and the GCM tag is appended to the
//! ciphertext.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use std::path::Path;
use std::sync::OnceLock;

static KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Load the server secret key once: from VELO_SECRET_KEY (base64, 32 bytes) or
/// a key file (created with a fresh random key if absent).
pub fn init_key(key_file: &Path) {
    let key = if let Ok(b64) = std::env::var("VELO_SECRET_KEY") {
        let bytes = B64.decode(b64.trim()).expect("VELO_SECRET_KEY must be base64");
        assert_eq!(bytes.len(), 32, "VELO_SECRET_KEY must decode to 32 bytes");
        let mut k = [0u8; 32];
        k.copy_from_slice(&bytes);
        k
    } else if key_file.exists() {
        let b64 = std::fs::read_to_string(key_file).expect("failed to read key file");
        let bytes = B64.decode(b64.trim()).expect("key file must be base64");
        let mut k = [0u8; 32];
        k.copy_from_slice(&bytes);
        k
    } else {
        let mut k = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut k);
        std::fs::write(key_file, B64.encode(k)).expect("failed to write key file");
        tracing::info!("Generated new server secret key at {}", key_file.display());
        k
    };
    let _ = KEY.set(key);
}

fn key() -> &'static [u8; 32] {
    KEY.get().expect("server crypto key not initialized")
}

/// Encrypt plaintext → "iv_b64:ciphertext_b64".
pub fn encrypt(plaintext: &str) -> String {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key()));
    let mut iv = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("encryption failed");
    format!("{}:{}", B64.encode(iv), B64.encode(ciphertext))
}

/// Decrypt "iv_b64:ciphertext_b64" → plaintext.
pub fn decrypt(encoded: &str) -> Result<String, String> {
    let (iv_b64, ct_b64) = encoded
        .split_once(':')
        .ok_or_else(|| "invalid encrypted value".to_string())?;
    let iv = B64.decode(iv_b64).map_err(|e| e.to_string())?;
    let ct = B64.decode(ct_b64).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key()));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&iv), ct.as_ref())
        .map_err(|_| "decryption failed".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        // Use an explicit key so the test doesn't touch the filesystem.
        std::env::set_var("VELO_SECRET_KEY", B64.encode([7u8; 32]));
        let dummy = std::env::temp_dir().join("never-created.key");
        init_key(&dummy);
        let enc = encrypt("hunter2");
        assert!(enc.contains(':'));
        assert_eq!(decrypt(&enc).unwrap(), "hunter2");
    }
}
