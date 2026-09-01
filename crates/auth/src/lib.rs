//! Authentication primitives. No HTTP or database types live here.

pub mod crypto;
pub mod ct;
pub mod password;
pub mod recovery;
pub mod session;
pub mod totp;

pub use crypto::{decrypt_secret, encrypt_secret, MasterKey};
pub use ct::{ct_eq, ct_eq_hex};
pub use password::{hash_password, verify_password, Argon2Params};
pub use recovery::{generate_recovery_codes, hash_recovery_code, verify_recovery_code};
pub use session::{generate_token, hash_token, TokenHash};
pub use totp::{
    decrypt_totp_secret, encrypt_totp_secret, generate_totp_secret, totp_otpauth_url, verify_totp,
    TotpSecret,
};
