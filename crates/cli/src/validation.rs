use camino::Utf8PathBuf;
use dirs::home_dir;
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

pub fn expand_tilde(path: &String) -> Utf8PathBuf {
    let path = Utf8PathBuf::from(path);

    if path.starts_with("~") {
        if let Some(home) = home_dir() {
            let home_utf8 = Utf8PathBuf::from_path_buf(home).unwrap();
            return home_utf8.join(path.strip_prefix("~").unwrap());
        }
    }

    path
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

    #[test]
    fn test_expand_tilde() {
        let path = String::from("~/cairo_2_4_3");
        let expanded_path: Utf8PathBuf = expand_tilde(&path);
        let expected_path: Utf8PathBuf = Utf8PathBuf::from_path_buf(home_dir().unwrap())
            .unwrap()
            .join("cairo_2_4_3");
        assert_eq!(expanded_path, expected_path);
    }
}
