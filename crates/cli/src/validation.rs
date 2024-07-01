use regex::Regex;

const NORMALIZED_HASH_LENGTH: usize = 66;
const CLASS_HASH_PATTERN: &str = r"^0x[a-fA-F0-9]+$";

pub fn is_class_hash_valid(hash: &str) -> bool {
    let re = Regex::new(CLASS_HASH_PATTERN).unwrap();

    if hash.len() <= NORMALIZED_HASH_LENGTH && re.is_match(hash) {
        return true
    }

    false
}
