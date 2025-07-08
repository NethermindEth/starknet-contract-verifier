#![allow(clippy::unwrap_used)]

use reqwest::StatusCode;
use url::Url;
use verifier::api::{ApiClientError, VerificationError};
use verifier::class_hash::{ClassHash, ClassHashError};
use verifier::errors::{MissingContract, RequestFailure};
use verifier::resolver;

#[test]
fn test_missing_contract_error_with_suggestions() {
    let missing_contract = MissingContract::new(
        "my_contrct".to_string(), // Intentional typo
        vec!["my_contract".to_string(), "other_contract".to_string()],
    );

    let error_message = format!("{missing_contract}");

    // Check that error code is included
    assert!(error_message.contains("[E003]"));
    // Check that it provides suggestions
    assert!(error_message.contains("Available contracts:"));
    assert!(error_message.contains("my_contract"));
    assert!(error_message.contains("other_contract"));
    // Check fuzzy matching suggestion
    assert!(error_message.contains("Did you mean 'my_contract'?"));
    // Check actionable suggestions
    assert!(error_message.contains("Use --contract-name"));
}

#[test]
fn test_missing_contract_error_with_no_available() {
    let missing_contract = MissingContract::new("any_contract".to_string(), vec![]);

    let error_message = format!("{missing_contract}");

    // Check that it handles empty list properly
    assert!(error_message.contains("No contracts are defined"));
    assert!(error_message.contains("Add a [tool.voyager] section"));
}

#[test]
fn test_request_failure_error_with_status_specific_suggestions() {
    let url = Url::parse("https://api.example.com/verify").unwrap();

    // Test 404 error
    let request_failure = RequestFailure::new(
        url.clone(),
        StatusCode::NOT_FOUND,
        "Resource not found".to_string(),
    );

    let error_message = format!("{request_failure}");

    assert!(error_message.contains("[E002]"));
    assert!(error_message.contains("404"));
    assert!(error_message.contains("Check that the URL is correct"));
    assert!(error_message.contains("Server response: Resource not found"));

    // Test 429 error
    let rate_limit_failure = RequestFailure::new(
        url,
        StatusCode::TOO_MANY_REQUESTS,
        "Rate limited".to_string(),
    );

    let rate_limit_message = format!("{rate_limit_failure}");
    assert!(rate_limit_message.contains("Wait a moment before retrying"));
    assert!(rate_limit_message.contains("reducing request frequency"));
}

#[test]
fn test_class_hash_error_with_detailed_format_info() {
    let invalid_hash = "invalid_hash";
    let class_hash_result = ClassHash::new(invalid_hash);

    assert!(class_hash_result.is_err());
    let error = class_hash_result.unwrap_err();
    let error_message = format!("{error}");

    assert!(error_message.contains("[E010]"));
    assert!(error_message.contains("Expected format: 0x followed by up to 64 hexadecimal"));
    assert!(error_message.contains("Example: 0x044dc2b3239382230d8b1e943df23b96"));
    assert!(error_message.contains("Check that the hash starts with '0x'"));

    // Test proper error structure
    match error {
        ClassHashError::Match(_) => {
            // Error has correct variant
        }
        _ => panic!("Expected Match error"),
    }
}

#[test]
fn test_verification_error_messages() {
    let compilation_error = VerificationError::CompilationFailure("Missing import".to_string());
    let verification_error = VerificationError::VerificationFailure("Hash mismatch".to_string());

    // Test compilation error
    let comp_message = format!("{compilation_error}");
    assert!(comp_message.contains("[E004]"));
    assert!(comp_message.contains("Compilation failed"));

    // Test verification error
    let verif_message = format!("{verification_error}");
    assert!(verif_message.contains("[E005]"));
    assert!(verif_message.contains("Verification failed"));
}

#[test]
fn test_api_client_error_messages() {
    let job_not_found = ApiClientError::JobNotFound("12345".to_string());
    let in_progress = ApiClientError::InProgress;

    // Test job not found
    let job_message = format!("{job_not_found}");
    assert!(job_message.contains("[E008]"));
    assert!(job_message.contains("Job '12345' not found"));
    assert!(job_message.contains("Check that the job ID is correct"));

    // Test in progress
    let progress_message = format!("{in_progress}");
    assert!(progress_message.contains("[E007]"));
    assert!(progress_message.contains("still in progress"));
    assert!(progress_message.contains("Use --wait to automatically wait"));
}

#[test]
fn test_resolver_error_with_suggestions() {
    let dependency_error = resolver::Error::DependencyPath {
        name: "my_lib".to_string(),
        path: "invalid:path".to_string(),
    };

    let error_message = format!("{dependency_error}");

    assert!(error_message.contains("[E012]"));
    assert!(error_message.contains("Invalid dependency path"));
    assert!(error_message.contains("Check that the path exists"));
    assert!(error_message.contains("Example: path:../my-dependency"));
}

#[test]
fn test_fuzzy_matching_edge_cases() {
    // Test very different strings (should not suggest)
    let missing_contract = MissingContract::new(
        "xyz".to_string(),
        vec!["completely_different_name".to_string()],
    );

    let error_message = format!("{missing_contract}");
    // Should not suggest when strings are too different
    assert!(!error_message.contains("Did you mean"));

    // Test close match
    let close_match = MissingContract::new(
        "contrct".to_string(), // Missing 'a'
        vec!["contract".to_string(), "contact".to_string()],
    );

    let close_message = format!("{close_match}");
    // Should suggest the closest match
    assert!(close_message.contains("Did you mean 'contract'?"));
}

#[test]
fn test_error_message_structure() {
    // Test that error messages follow consistent format
    let missing_contract = MissingContract::new("test".to_string(), vec!["available".to_string()]);

    let error_message = format!("{missing_contract}");

    // Should have error code
    assert!(error_message.contains("[E"));
    // Should have suggestions section
    assert!(error_message.contains("Suggestions:"));
    // Should use bullet points
    assert!(error_message.contains("•"));
}
