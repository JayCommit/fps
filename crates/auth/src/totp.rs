use rand::RngCore;
use totp_rs::{Algorithm, Secret, TOTP};
use zeroize::Zeroize;

use crate::crypto::{decrypt_secret, encrypt_secret, MasterKey};
use fps_domain::{ErrorCode, PlatformError};

const ISSUER: &str = "FPS";

#[derive(Clone)]
pub struct TotpSecret(String);

impl TotpSecret {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn generate_totp_secret() -> TotpSecret {
    let mut bytes = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let secret = Secret::Raw(bytes.to_vec()).to_encoded().to_string();
    bytes.zeroize();
    TotpSecret(secret)
}

pub fn encrypt_totp_secret(key: &MasterKey, secret: &TotpSecret) -> Result<String, PlatformError> {
    encrypt_secret(key, secret.as_str())
        .map_err(|_| PlatformError::new(ErrorCode::Internal, "failed to encrypt TOTP secret"))
}

pub fn decrypt_totp_secret(key: &MasterKey, packed: &str) -> Result<TotpSecret, PlatformError> {
    decrypt_secret(key, packed)
        .map(TotpSecret)
        .map_err(|_| PlatformError::new(ErrorCode::Internal, "failed to decrypt TOTP secret"))
}

fn totp(secret: &TotpSecret, account: &str) -> Result<TOTP, PlatformError> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(secret.0.clone())
            .to_bytes()
            .map_err(|_| PlatformError::new(ErrorCode::Internal, "invalid TOTP secret"))?,
        Some(ISSUER.to_string()),
        account.to_string(),
    )
    .map_err(|e| PlatformError::new(ErrorCode::Internal, e.to_string()))
}

pub fn totp_otpauth_url(secret: &TotpSecret, account: &str) -> Result<String, PlatformError> {
    Ok(totp(secret, account)?.get_url())
}

/// Verifies a 6-digit TOTP. totp-rs `skew` is 1, so the previous and next 30s windows are accepted.
pub fn verify_totp(secret: &TotpSecret, account: &str, code: &str) -> Result<bool, PlatformError> {
    let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 6 {
        return Ok(false);
    }
    totp(secret, account)?
        .check_current(&digits)
        .map_err(|e| PlatformError::new(ErrorCode::Internal, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_code_verifies() {
        let secret = generate_totp_secret();
        let t = totp(&secret, "owner@example.test").unwrap();
        let code = t.generate_current().unwrap();
        assert!(verify_totp(&secret, "owner@example.test", &code).unwrap());
        assert!(!verify_totp(&secret, "owner@example.test", "000000").unwrap());
        let previous = t.generate(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .saturating_sub(30),
        );
        assert!(
            verify_totp(&secret, "owner@example.test", &previous).unwrap(),
            "TOTP must accept the previous 30s window (skew=1)"
        );
    }
}
