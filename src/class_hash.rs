//! # Class Hash Validation
//!
//! This module provides type-safe handling and validation of Starknet class hashes.
//! Class hashes are 256-bit values represented as hexadecimal strings with "0x" prefix.
//!
//! ## Features
//!
//! - **Type Safety**: Strong typing prevents invalid class hashes from being used
//! - **Validation**: Automatic validation of format and length
//! - **Performance**: Compiled regex patterns for efficient validation
//! - **Error Handling**: Detailed error messages with actionable suggestions
//!
//! ## Example Usage
//!
//! ```rust
//! use verifier::class_hash::ClassHash;
//!
//! // Valid class hash
//! let hash = ClassHash::new("0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18")?;
//! println!("Class hash: {}", hash);
//!
//! // Invalid class hash will return an error
//! let invalid = ClassHash::new("invalid_hash");
//! assert!(invalid.is_err());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use lazy_static::lazy_static;
use regex::Regex;
use std::fmt;
use thiserror::Error;

fn get_class_hash_regex() -> Result<&'static Regex, ClassHashError> {
    lazy_static! {
        static ref CLASS_HASH_REGEX: Result<Regex, regex::Error> = Regex::new(r"^0x[a-fA-F0-9]+$");
    }

    match CLASS_HASH_REGEX.as_ref() {
        Ok(regex) => Ok(regex),
        Err(_) => Err(ClassHashError::RegexError),
    }
}

/// A type-safe wrapper for Starknet class hashes.
///
/// Class hashes are 256-bit values represented as hexadecimal strings prefixed with "0x".
/// This type ensures that only valid class hashes can be constructed and used throughout
/// the application.
///
/// ## Format Requirements
///
/// - Must start with "0x"
/// - Must contain only hexadecimal characters (0-9, a-f, A-F)
/// - Must be at most 66 characters long (including "0x" prefix)
///
/// ## Examples
///
/// ```rust
/// use verifier::class_hash::ClassHash;
///
/// // Valid class hash
/// let hash = ClassHash::new("0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18")?;
/// println!("Hash: {}", hash);
///
/// // Shorter hashes are also valid
/// let short_hash = ClassHash::new("0x123")?;
/// println!("Short hash: {}", short_hash);
///
/// // Invalid hashes will return an error
/// assert!(ClassHash::new("invalid").is_err());
/// assert!(ClassHash::new("0xGGG").is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassHash(String);

/// Errors that can occur when validating or creating class hashes.
///
/// Each error variant provides detailed information about what went wrong
/// and includes actionable suggestions for fixing the issue.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ClassHashError {
    #[error("[E010] Invalid class hash format: '{0}'\n\nExpected format: 0x followed by up to 64 hexadecimal characters\nExample: 0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18\n\nSuggestions:\n  • Check that the hash starts with '0x'\n  • Verify all characters are hexadecimal (0-9, a-f, A-F)\n  • Ensure the hash is not longer than 66 characters total")]
    Match(String),
    #[error("[E011] Internal regex compilation error\n\nThis is an internal error. Please report this issue.")]
    RegexError,
}

impl ClassHashError {
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Match(_) => "E010",
            Self::RegexError => "E011",
        }
    }
}

impl ClassHash {
    const NORMALIZED_LENGTH: usize = 66;

    /// Creates a new `ClassHash` from a string.
    ///
    /// Validates that the input string follows the correct format for a Starknet class hash:
    /// - Must start with "0x"
    /// - Must contain only hexadecimal characters (0-9, a-f, A-F)
    /// - Must be at most 66 characters long (including "0x" prefix)
    ///
    /// # Arguments
    ///
    /// * `raw` - The string representation of the class hash
    ///
    /// # Returns
    ///
    /// Returns `Ok(ClassHash)` if the input is valid, or `Err(ClassHashError)` with
    /// detailed error information if the input is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use verifier::class_hash::ClassHash;
    ///
    /// // Valid class hash
    /// let hash = ClassHash::new("0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18")?;
    /// println!("Created hash: {}", hash);
    ///
    /// // Invalid format will return an error
    /// let result = ClassHash::new("invalid_hash");
    /// assert!(result.is_err());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ClassHashError::Match` if the input doesn't match the expected format,
    /// or `ClassHashError::RegexError` if there's an internal regex compilation error.
    pub fn new(raw: &str) -> Result<Self, ClassHashError> {
        let regex = get_class_hash_regex()?;
        if raw.len() <= Self::NORMALIZED_LENGTH && regex.is_match(raw) {
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
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn test_invalid_class_hash_too_long() {
        let invalid_hash =
            "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da1812345";
        assert!(ClassHash::new(invalid_hash).is_err());
    }

    #[test]
    fn test_empty_class_hash() {
        assert!(ClassHash::new("").is_err());
    }

    #[test]
    fn test_class_hash_display() {
        let hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        let class_hash = ClassHash::new(hash).unwrap();
        assert_eq!(format!("{class_hash}"), hash);
    }

    #[test]
    fn test_class_hash_as_ref_str() {
        let hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        let class_hash = ClassHash::new(hash).unwrap();
        let as_str: &str = class_hash.as_ref();
        assert_eq!(as_str, hash);
    }

    #[test]
    fn test_class_hash_as_ref_string() {
        let hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        let class_hash = ClassHash::new(hash).unwrap();
        let expected_string = hash.to_string();
        let as_string: &String = class_hash.as_ref();
        assert_eq!(as_string, &expected_string);
    }

    #[test]
    fn test_class_hash_clone() {
        let hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
        let class_hash = ClassHash::new(hash).unwrap();
        let cloned = class_hash.clone();
        assert_eq!(class_hash, cloned);
    }

    #[test]
    fn test_class_hash_error_display() {
        let error = ClassHashError::Match("invalid_hash".to_string());
        let error_message = format!("{error}");
        assert!(error_message.contains("[E010]"));
        assert!(error_message.contains("Invalid class hash format"));
        assert!(error_message.contains("invalid_hash"));
        assert!(error_message.contains("Expected format: 0x followed by"));
    }
}
