use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

pub type TokenHash = String;

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> TokenHash {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Binding hash used so a stolen session cookie without the CSRF token is not enough
/// for state-changing cookie-authenticated requests.
pub fn bind_csrf(session_token: &str, csrf: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(session_token.as_bytes())
        .expect("HMAC accepts 32-byte keys");
    mac.update(csrf.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_hex_sha256() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert_eq!(hash_token(&token).len(), 64);
        assert_ne!(hash_token(&token), token);
    }
}
