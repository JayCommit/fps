use rand::Rng;
use sha2::{Digest, Sha256};

use crate::ct::ct_eq_hex;

const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const CODE_LEN: usize = 10;
const CODE_COUNT: usize = 10;

pub fn generate_recovery_codes() -> Vec<String> {
    let mut rng = rand::rngs::OsRng;
    (0..CODE_COUNT)
        .map(|_| {
            let mut code = String::with_capacity(CODE_LEN + 1);
            for i in 0..CODE_LEN {
                if i == 5 {
                    code.push('-');
                }
                let idx = rng.gen_range(0..ALPHABET.len());
                code.push(ALPHABET[idx] as char);
            }
            code
        })
        .collect()
}

pub fn hash_recovery_code(code: &str) -> String {
    let normalized = normalize(code);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn verify_recovery_code(code: &str, hashes: &[String]) -> Option<usize> {
    let candidate = hash_recovery_code(code);
    let mut found = None;
    for (i, h) in hashes.iter().enumerate() {
        if ct_eq_hex(&candidate, h) {
            found = Some(i);
        }
    }
    found
}

fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_round_trip() {
        let codes = generate_recovery_codes();
        assert_eq!(codes.len(), 10);
        let hashes: Vec<_> = codes.iter().map(|c| hash_recovery_code(c)).collect();
        assert_eq!(verify_recovery_code(&codes[3], &hashes), Some(3));
        assert_eq!(verify_recovery_code("nope", &hashes), None);
    }
}
