use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use fps_domain::{ErrorCode, PlatformError};
use password_hash::rand_core::OsRng;
use serde::{Deserialize, Serialize};

/// Documented Argon2id parameters for the Fry-class control-plane host
/// (Xeon E3-1270 v3, 32 GiB). See docs/operations/authentication.md.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory in KiB. Default 19456 (19 MiB) matches OWASP 2024 minimum.
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        }
    }
}

impl Argon2Params {
    pub fn for_tests() -> Self {
        Self {
            memory_kib: 8_192,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn engine(self) -> Result<Argon2<'static>, PlatformError> {
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, None)
            .map_err(|e| PlatformError::new(ErrorCode::Internal, e.to_string()))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

pub fn hash_password(password: &str, params: Argon2Params) -> Result<String, PlatformError> {
    validate_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let hash = params
        .engine()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PlatformError::new(ErrorCode::Internal, e.to_string()))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, encoded: &str) -> Result<bool, PlatformError> {
    let parsed = PasswordHash::new(encoded)
        .map_err(|_| PlatformError::new(ErrorCode::Internal, "stored password hash is invalid"))?;
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PlatformError::new(ErrorCode::Internal, e.to_string())),
    }
}

pub fn validate_password(password: &str) -> Result<(), PlatformError> {
    if password.len() < 12 {
        return Err(
            PlatformError::validation("Password must be at least 12 characters.").field("password"),
        );
    }
    if password.len() > 1024 {
        return Err(PlatformError::validation("Password is too long.").field("password"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let hash =
            hash_password("correct horse battery staple", Argon2Params::for_tests()).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong password!!", &hash).unwrap());
    }

    #[test]
    fn rejects_short_passwords() {
        assert!(hash_password("short", Argon2Params::for_tests()).is_err());
    }
}
