//! AES-256-GCM token cipher for encrypting bank provider access tokens.
//!
//! Tokens are stored as `<base64(nonce)>:<base64(ciphertext)>` in the DB.
//! The key is loaded from `BANK_FEEDS_ENCRYPTION_KEY` (base64, 32 bytes).

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

/// Wraps an AES-256-GCM key for sealing/opening opaque blobs.
pub struct TokenCipher {
    key: [u8; 32],
}

impl TokenCipher {
    /// Build from a 32-byte raw key.
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Build from a base64-encoded key string (must decode to 32 bytes).
    pub fn from_base64(b64: &str) -> anyhow::Result<Self> {
        let raw = B64.decode(b64.trim())?;
        if raw.len() != 32 {
            anyhow::bail!(
                "BANK_FEEDS_ENCRYPTION_KEY must decode to 32 bytes, got {}",
                raw.len()
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&raw);
        Ok(Self { key })
    }

    /// Encrypt `plaintext`. Returns `"<b64_nonce>:<b64_ciphertext>"`.
    pub fn seal(&self, plaintext: &str) -> String {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        // AES-256-GCM encrypt is infallible for well-formed keys/nonces.
        let ct = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .expect("invariant: AES-GCM encrypt failed with valid key");
        format!("{}:{}", B64.encode(nonce), B64.encode(ct))
    }

    /// Decrypt a blob produced by [`seal`]. Returns `None` on any error.
    pub fn open(&self, blob: &str) -> Option<String> {
        let (nonce_b64, ct_b64) = blob.split_once(':')?;
        let nonce_bytes = B64.decode(nonce_b64).ok()?;
        let ct_bytes = B64.decode(ct_b64).ok()?;
        if nonce_bytes.len() != 12 {
            return None;
        }
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let pt = cipher.decrypt(nonce, ct_bytes.as_slice()).ok()?;
        String::from_utf8(pt).ok()
    }
}

// ─── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn test_seal_open_roundtrip() {
        let cipher = TokenCipher::new(test_key());
        let plain = "access-abc-secret-token";
        let sealed = cipher.seal(plain);
        let opened = cipher.open(&sealed).expect("should open");
        assert_eq!(opened, plain);
    }

    #[test]
    fn test_seal_does_not_expose_plaintext() {
        let cipher = TokenCipher::new(test_key());
        let plain = "access-abc";
        let sealed = cipher.seal(plain);
        assert!(
            !sealed.contains(plain),
            "ciphertext should not contain plaintext; got: {sealed}"
        );
    }

    #[test]
    fn test_open_bad_blob_returns_none() {
        let cipher = TokenCipher::new(test_key());
        assert!(cipher.open("not-valid-base64:nope").is_none());
        assert!(cipher.open("").is_none());
    }

    /// Property: for 1000 random plaintexts, open(seal(p)) == p.
    #[test]
    fn prop_token_cipher_is_bijective() {
        let cipher = TokenCipher::new(test_key());
        let cases: Vec<String> = (0u32..1000)
            .map(|i| format!("token-{i}-secret-payload-{}", i * 7))
            .collect();
        for plain in &cases {
            let sealed = cipher.seal(plain);
            let opened = cipher.open(&sealed).expect("round-trip failed");
            assert_eq!(&opened, plain);
        }
    }
}
