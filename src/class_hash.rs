use regex::Regex;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassHash(String);

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ClassHashError {
    #[error("{0} is not valid class hash")]
    Match(String),
    #[error("Class hash regex error")]
    Regex(#[from] regex::Error),
}

impl ClassHash {
    const NORMALIZED_LENGTH: usize = 66;
    const PATTERN: &str = r"^0x[a-fA-F0-9]+$";

    /// # Errors
    ///
    /// Will fail if the `raw` dosn't match class hash regex, i.e. it
    /// has to start with "0x" followed by 64 hexadecimal digits.
    pub fn new(raw: &str) -> Result<Self, ClassHashError> {
        let re = Regex::new(Self::PATTERN)?;

        if raw.len() <= Self::NORMALIZED_LENGTH && re.is_match(raw) {
            Ok(Self(raw.into()))
        } else {
            Err(ClassHashError::Match(raw.to_string()))
        }
    }
}

impl fmt::Display for ClassHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ClassHash {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<String> for ClassHash {
    fn as_ref(&self) -> &String {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_class_hash_normalized() {
        let valid_hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        assert!(ClassHash::new(valid_hash).is_ok());
    }

    #[test]
    fn test_valid_class_hash_without_leading_zeros() {
        let valid_hash = "0x44dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        assert!(ClassHash::new(valid_hash).is_ok());
    }

    #[test]
    fn test_invalid_class_hash_pattern() {
        let invalid_hash = "0xGHIJKLMNOPQRSTUVWXYZ";
        assert!(ClassHash::new(invalid_hash).is_err());
    }

    #[test]
    fn test_invalid_class_hash_no_prefix() {
        let invalid_hash = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert!(ClassHash::new(invalid_hash).is_err());
    }
}
