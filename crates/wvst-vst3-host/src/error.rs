use std::error::Error;
use std::fmt::{Display, Formatter};

pub type HostResult<T> = Result<T, HostError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HostError {
    InvalidChannelCount { input: usize, output: usize },
    InvalidBufferLength { expected: usize, actual: usize },
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChannelCount { input, output } => {
                write!(
                    formatter,
                    "invalid channel count: input={input}, output={output}"
                )
            }
            Self::InvalidBufferLength { expected, actual } => {
                write!(
                    formatter,
                    "invalid buffer length: expected {expected}, got {actual}"
                )
            }
        }
    }
}

impl Error for HostError {}
