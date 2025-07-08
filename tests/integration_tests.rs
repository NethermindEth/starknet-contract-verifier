#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use verifier::api::{VerificationError, VerifyJobStatus};
use verifier::class_hash::{ClassHash, ClassHashError};
use verifier::resolver;
use verifier::voyager::{self, Voyager};

#[test]
fn test_class_hash_integration() {
    // Test valid class hash creation and usage
    let valid_hash = "0x044dc2b3239382230d8b1e943df23b96f52eebcac93efe6e8bde92f9a2f1da18";
    let class_hash = ClassHash::new(valid_hash).unwrap();

    // Test that it can be used in various contexts
    assert_eq!(class_hash.to_string(), valid_hash);
    let as_str: &str = class_hash.as_ref();
    assert_eq!(as_str, valid_hash);

    // Test invalid class hash
    let invalid_hash = "invalid_hash";
    let result = ClassHash::new(invalid_hash);
    assert!(result.is_err());

    match result.unwrap_err() {
        ClassHashError::Match(hash) => assert_eq!(hash, invalid_hash),
        _ => panic!("Expected Match error"),
    }
}

#[test]
fn test_verify_job_status_serialization() {
    // Test that VerifyJobStatus can be serialized/deserialized
    let statuses = vec![
        VerifyJobStatus::Submitted,
        VerifyJobStatus::Compiled,
        VerifyJobStatus::CompileFailed,
        VerifyJobStatus::Fail,
        VerifyJobStatus::Success,
        VerifyJobStatus::Processing,
        VerifyJobStatus::Unknown,
    ];

    for status in statuses {
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: VerifyJobStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_verification_error_display() {
    let compilation_error = VerificationError::CompilationFailure("Test error".to_string());
    assert_eq!(
        format!("{compilation_error}"),
        "Compilation failed: Test error"
    );

    let verification_error = VerificationError::VerificationFailure("Test error".to_string());
    assert_eq!(
        format!("{verification_error}"),
        "Compilation failed: Test error"
    );
}

#[test]
fn test_file_system_integration() {
    // Create a temporary directory structure for testing
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Create test files
    let src_dir = temp_path.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.cairo"), "// Cairo source code").unwrap();
    std::fs::write(src_dir.join("contract.cairo"), "// Contract code").unwrap();

    // Create test files that should be excluded
    let tests_dir = temp_path.join("tests");
    std::fs::create_dir_all(&tests_dir).unwrap();
    std::fs::write(tests_dir.join("test.cairo"), "// Test code").unwrap();

    // Test file discovery logic (this would be integration with resolver)
    let cairo_files: Vec<PathBuf> = std::fs::read_dir(&src_dir)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()?.to_str()? == "cairo" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(cairo_files.len(), 2);
    assert!(cairo_files
        .iter()
        .any(|p| p.file_name().unwrap() == "lib.cairo"));
    assert!(cairo_files
        .iter()
        .any(|p| p.file_name().unwrap() == "contract.cairo"));
}

#[test]
fn test_voyager_integration() {
    // Test the voyager data structure integration
    let mut contract_map = HashMap::new();
    contract_map.insert(
        "contract1".to_string(),
        Voyager {
            path: PathBuf::from("/test/path/contract1.cairo"),
            address: Some("0x123456789abcdef".to_string()),
        },
    );

    contract_map.insert(
        "contract2".to_string(),
        Voyager {
            path: PathBuf::from("/test/path/contract2.cairo"),
            address: None,
        },
    );

    // Test that the data structure works as expected
    assert_eq!(contract_map.len(), 2);
    assert!(contract_map.contains_key("contract1"));
    assert!(contract_map.contains_key("contract2"));

    let contract1 = contract_map.get("contract1").unwrap();
    assert_eq!(contract1.path, PathBuf::from("/test/path/contract1.cairo"));
    assert_eq!(contract1.address, Some("0x123456789abcdef".to_string()));

    let contract2 = contract_map.get("contract2").unwrap();
    assert_eq!(contract2.path, PathBuf::from("/test/path/contract2.cairo"));
    assert_eq!(contract2.address, None);
}

#[test]
fn test_error_handling_integration() {
    // Test that errors can be properly chained and displayed
    let class_hash_error = ClassHashError::Match("invalid".to_string());
    let error_message = format!("{class_hash_error}");
    assert_eq!(error_message, "invalid is not valid class hash");

    // Test resolver errors
    let resolver_error = resolver::Error::DependencyPath {
        name: "test_dep".to_string(),
        path: "/invalid/path".to_string(),
    };
    let error_message = format!("{resolver_error}");
    assert_eq!(error_message, "Couldn't parse test_dep path: /invalid/path");

    // Test voyager errors
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let voyager_error = voyager::Error::Deserialization(json_error);
    let error_message = format!("{voyager_error}");
    assert!(error_message.contains("expected value"));
}

#[test]
fn test_path_handling_integration() {
    use camino::Utf8PathBuf;
    use verifier::resolver::biggest_common_prefix;

    // Test realistic path scenarios
    let paths = vec![
        Utf8PathBuf::from("/home/user/project/src/lib.cairo"),
        Utf8PathBuf::from("/home/user/project/src/contract.cairo"),
        Utf8PathBuf::from("/home/user/project/tests/test.cairo"),
    ];

    let first_guess = Utf8PathBuf::from("/home/user/project/src/lib.cairo");
    let common_prefix = biggest_common_prefix(&paths, first_guess);
    assert_eq!(common_prefix, Utf8PathBuf::from("/home/user/project"));

    // Test edge case with no common prefix
    let paths = vec![
        Utf8PathBuf::from("/home/user1/project/src/lib.cairo"),
        Utf8PathBuf::from("/home/user2/project/src/lib.cairo"),
    ];
    let first_guess = Utf8PathBuf::from("/home/user1/project/src/lib.cairo");
    let common_prefix = biggest_common_prefix(&paths, first_guess);
    assert_eq!(common_prefix, Utf8PathBuf::from("/home"));
}

#[test]
fn test_status_display_integration() {
    let statuses = vec![
        VerifyJobStatus::Submitted,
        VerifyJobStatus::Compiled,
        VerifyJobStatus::CompileFailed,
        VerifyJobStatus::Fail,
        VerifyJobStatus::Success,
        VerifyJobStatus::Processing,
        VerifyJobStatus::Unknown,
    ];

    let expected_displays = vec![
        "Submitted",
        "Compiled",
        "CompileFailed",
        "Fail",
        "Success",
        "Processing",
        "Unknown",
    ];

    for (status, expected) in statuses.into_iter().zip(expected_displays.into_iter()) {
        assert_eq!(format!("{status}"), expected);
    }
}
