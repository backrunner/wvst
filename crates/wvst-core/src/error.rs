use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CoreError {
    InvalidSampleRate(u32),
    InvalidFrameCount(u16),
    InvalidChannelCount(u16),
    PayloadTooLarge,
}

impl Display for CoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSampleRate(value) => write!(formatter, "invalid sample rate: {value}"),
            Self::InvalidFrameCount(value) => write!(formatter, "invalid frame count: {value}"),
            Self::InvalidChannelCount(value) => {
                write!(formatter, "invalid channel count: {value}")
            }
            Self::PayloadTooLarge => formatter.write_str("audio payload is too large"),
        }
    }
}

impl Error for CoreError {}
