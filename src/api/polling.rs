// This module contains polling functionality that was moved to the client.rs file
// Re-export from client for backward compatibility
pub use super::client::poll_verification_status;
