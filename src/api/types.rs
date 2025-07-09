use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::Display;

#[derive(Clone, Debug, Deserialize_repr, Eq, PartialEq, Serialize_repr)]
#[repr(u8)]
pub enum VerifyJobStatus {
    Submitted = 0,
    Compiled = 1,
    CompileFailed = 2,
    Fail = 3,
    Success = 4,
    Processing = 5,
    #[serde(other)]
    Unknown,
}

impl Display for VerifyJobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submitted => write!(f, "Submitted"),
            Self::Compiled => write!(f, "Compiled"),
            Self::CompileFailed => write!(f, "CompileFailed"),
            Self::Fail => write!(f, "Fail"),
            Self::Success => write!(f, "Success"),
            Self::Processing => write!(f, "Processing"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

pub type JobStatus = VerifyJobStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    InProgress,
    Completed,
}
