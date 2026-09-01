use subtle::ConstantTimeEq;

/// Constant-time equality for equal-length byte strings (token hashes, recovery hashes).
/// Different lengths return false after a dummy compare so the mismatch is not a fast path.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        let _ = a.ct_eq(a);
        return false;
    }
    bool::from(a.ct_eq(b))
}

pub fn ct_eq_hex(a: &str, b: &str) -> bool {
    ct_eq(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_hashes_match() {
        assert!(ct_eq_hex("abcd", "abcd"));
        assert!(!ct_eq_hex("abcd", "abce"));
        assert!(!ct_eq_hex("abcd", "abc"));
    }
}
