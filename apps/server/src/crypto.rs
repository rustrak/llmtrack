//! Virtual keys and provider secrets.
//!
//! A virtual key is `sk-` plus 48 hex characters (192 random bits). Only its
//! SHA-256 is stored: with that much entropy a plain hash is as strong as a
//! slow one, and it keeps the lookup on the hot path a single index probe.
//!
//! Provider API keys have to be read back to be sent upstream, so they are
//! encrypted with AES-256-GCM under a key derived from `SECRET_KEY`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngExt;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub const KEY_PREFIX: &str = "sk-";

/// A freshly minted virtual key: the raw value (shown once), its hash and the
/// last four characters the dashboard shows instead.
pub struct NewKey {
    pub raw: String,
    pub hash: String,
    pub last4: String,
}

pub fn generate_key() -> NewKey {
    let bytes: [u8; 24] = rand::rng().random();
    let raw = format!("{KEY_PREFIX}{}", hex::encode(bytes));
    NewKey {
        hash: hash_key(&raw),
        last4: raw[raw.len() - 4..].to_string(),
        raw,
    }
}

pub fn hash_key(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// Encrypts and decrypts provider secrets.
#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
}

impl SecretBox {
    /// Derives the encryption key from the 64-byte master secret, under its
    /// own label so it never equals the cookie key.
    pub fn new(master: &[u8]) -> Self {
        let key = Sha256::new()
            .chain_update(b"llmtrack:provider-secrets:")
            .chain_update(master)
            .finalize();
        Self {
            cipher: Aes256Gcm::new_from_slice(&key).expect("SHA-256 is 32 bytes"),
        }
    }

    /// `hex(nonce || ciphertext)`.
    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let nonce: [u8; 12] = rand::rng().random();
        let ciphertext = self
            .cipher
            .encrypt(&Nonce::from(nonce), plaintext.as_bytes())
            .map_err(|_| AppError::Internal("encryption failed".into()))?;
        Ok(hex::encode([nonce.as_slice(), &ciphertext].concat()))
    }

    pub fn decrypt(&self, sealed: &str) -> AppResult<String> {
        let unreadable = || {
            AppError::Internal(
                "a stored secret cannot be decrypted; was SECRET_KEY changed?".into(),
            )
        };
        let bytes = hex::decode(sealed).map_err(|_| unreadable())?;
        if bytes.len() < 12 {
            return Err(unreadable());
        }
        let (nonce, ciphertext) = bytes.split_at(12);
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| unreadable())?;
        let plaintext = self
            .cipher
            .decrypt(&Nonce::from(nonce), ciphertext)
            .map_err(|_| unreadable())?;
        String::from_utf8(plaintext).map_err(|_| unreadable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_key_has_the_prefix_and_hashes_to_its_hash() {
        let key = generate_key();
        assert!(key.raw.starts_with("sk-"));
        assert_eq!(key.raw.len(), 3 + 48);
        assert_eq!(hash_key(&key.raw), key.hash);
        assert!(key.raw.ends_with(&key.last4));
    }

    #[test]
    fn two_keys_differ() {
        assert_ne!(generate_key().raw, generate_key().raw);
    }

    #[test]
    fn a_secret_roundtrips() {
        let sealed_box = SecretBox::new(&[7u8; 64]);
        let sealed = sealed_box.encrypt("sk-openai-123").unwrap();
        assert!(!sealed.contains("sk-openai"));
        assert_eq!(sealed_box.decrypt(&sealed).unwrap(), "sk-openai-123");
    }

    #[test]
    fn the_same_secret_encrypts_differently_each_time() {
        let sealed_box = SecretBox::new(&[7u8; 64]);
        assert_ne!(
            sealed_box.encrypt("x").unwrap(),
            sealed_box.encrypt("x").unwrap()
        );
    }

    #[test]
    fn another_master_key_cannot_decrypt() {
        let sealed = SecretBox::new(&[7u8; 64]).encrypt("secret").unwrap();
        assert!(SecretBox::new(&[8u8; 64]).decrypt(&sealed).is_err());
        assert!(SecretBox::new(&[7u8; 64]).decrypt("zz").is_err());
    }
}
