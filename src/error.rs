use std::{fmt, io};

/// Expected runtime failures are values, never panics.
#[derive(Debug)]
pub enum Error {
    Io {
        context: String,
        source: io::Error,
    },
    Corruption {
        file: String,
        offset: u64,
        reason: String,
    },
    InvalidConfig(String),
    InvalidInput(String),
    Conflict {
        expected: u64,
        actual: u64,
    },
    Locked,
    UnsupportedFormat(u32),
    ResourceLimit(String),
    Poisoned,
    Invariant(String),
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub(crate) fn corrupt(file: impl Into<String>, offset: u64, reason: impl Into<String>) -> Self {
        Self::Corruption {
            file: file.into(),
            offset,
            reason: reason.into(),
        }
    }
    pub(crate) fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::io("filesystem operation", e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Corruption {
                file,
                offset,
                reason,
            } => write!(f, "corruption {file} offset {offset}: {reason}"),
            Self::InvalidConfig(s) => write!(f, "invalid configuration: {s}"),
            Self::InvalidInput(s) => write!(f, "invalid input: {s}"),
            Self::Conflict { expected, actual } => {
                write!(f, "transaction conflict: base {expected}, current {actual}")
            }
            Self::Locked => write!(f, "database already locked"),
            Self::UnsupportedFormat(v) => write!(f, "unsupported format version {v}"),
            Self::ResourceLimit(s) => write!(f, "resource limit: {s}"),
            Self::Poisoned => write!(
                f,
                "handle poisoned by uncertain I/O outcome; close and reopen"
            ),
            Self::Invariant(s) => write!(f, "internal invariant: {s}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
