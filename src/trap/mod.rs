use std::error::Error;
use std::fmt;

pub mod stage2;
pub mod stage2_model;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrapErrorKind {
    InvalidAddress,
    InvalidPageSize,
    Misaligned,
    Overlap,
    NotMapped,
    UnsupportedCapability,
    InvalidState,
    PolicyDenied,
    MalformedInput,
}

impl TrapErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAddress => "invalid_address",
            Self::InvalidPageSize => "invalid_page_size",
            Self::Misaligned => "misaligned",
            Self::Overlap => "overlap",
            Self::NotMapped => "not_mapped",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::InvalidState => "invalid_state",
            Self::PolicyDenied => "policy_denied",
            Self::MalformedInput => "malformed_input",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapError {
    kind: TrapErrorKind,
    detail: String,
}

impl TrapError {
    pub fn new(kind: TrapErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn kind(&self) -> TrapErrorKind {
        self.kind
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for TrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.as_str(), self.detail)
    }
}

impl Error for TrapError {}
