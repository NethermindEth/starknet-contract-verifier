use regex::Regex;

const NORMALIZED_HASH_LENGTH: usize = 66;
const CLASS_HASH_PATTERN: &str = r"^0x[a-fA-F0-9]+$";

pub fn is_class_hash_valid(hash: &str) -> bool {
    let re = Regex::new(CLASS_HASH_PATTERN).unwrap();

    if hash.len() <= NORMALIZED_HASH_LENGTH && re.is_match(hash) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_class_hash_normalized() {
        let valid_hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        assert!(is_class_hash_valid(valid_hash));
    }

    #[test]
    fn test_valid_class_hash_without_leading_zeros() {
        let valid_hash = "0x44dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        assert!(is_class_hash_valid(valid_hash));
    }

    #[test]
    fn test_invalid_class_hash_pattern() {
        let invalid_hash = "0xGHIJKLMNOPQRSTUVWXYZ";
        assert!(!is_class_hash_valid(invalid_hash));
    }

    #[test]
    fn test_invalid_class_hash_no_prefix() {
        let invalid_hash = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(!is_class_hash_valid(invalid_hash));
    }
}
