use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use hex::FromHex;
use rand::RngCore;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("master key must be 32 bytes (64 hex characters)")]
    InvalidKey,
    #[error("encrypted payload is malformed")]
    Malformed,
    #[error("decryption failed")]
    Decrypt,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; KEY_LEN]);

impl MasterKey {
    pub fn from_hex(value: &str) -> Result<Self, CryptoError> {
        let bytes = <[u8; KEY_LEN]>::from_hex(value).map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self(bytes))
    }

    pub fn generate() -> (Self, String) {
        let mut bytes = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut bytes);
        let hex = hex::encode(bytes);
        (Self(bytes), hex)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

/// Encrypt UTF-8 secrets with AES-256-GCM. Output is `nonce || ciphertext` hex.
pub fn encrypt_secret(key: &MasterKey, plaintext: &str) -> Result<String, CryptoError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::Decrypt)?;
    let mut packed = nonce.to_vec();
    packed.extend_from_slice(&ciphertext);
    Ok(hex::encode(packed))
}

pub fn decrypt_secret(key: &MasterKey, packed_hex: &str) -> Result<String, CryptoError> {
    let packed = hex::decode(packed_hex).map_err(|_| CryptoError::Malformed)?;
    if packed.len() < 12 + 16 {
        return Err(CryptoError::Malformed);
    }
    let (nonce_bytes, ciphertext) = packed.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_bytes()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| CryptoError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let (key, _) = MasterKey::generate();
        let enc = encrypt_secret(&key, "s3cret").unwrap();
        assert_eq!(decrypt_secret(&key, &enc).unwrap(), "s3cret");
    }

    #[test]
    fn rejects_wrong_key() {
        let (key_a, _) = MasterKey::generate();
        let (key_b, _) = MasterKey::generate();
        let enc = encrypt_secret(&key_a, "s3cret").unwrap();
        assert!(decrypt_secret(&key_b, &enc).is_err());
    }
}
