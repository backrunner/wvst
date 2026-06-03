use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SharedAudioLayoutError {
    DescriptorTooShort {
        min: usize,
        actual: usize,
    },
    InvalidDescriptorMagic {
        actual: u32,
    },
    UnsupportedDescriptorVersion {
        actual: u16,
    },
    InvalidDescriptorField {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    CursorBlockTooShort {
        min: usize,
        actual: usize,
    },
    MemoryTooShort {
        min: u64,
        actual: u64,
    },
    ZeroSampleRate,
    ZeroBlockFrames,
    ZeroCapacityBlocks,
    ChannelCountTooLarge {
        channels: u16,
        max: u16,
    },
    CursorOrderInvalid {
        read_frame: u64,
        write_frame: u64,
    },
    InsufficientReadableFrames {
        requested: u64,
        available: u64,
    },
    InsufficientWritableFrames {
        requested: u64,
        available: u64,
    },
    SampleBufferLengthMismatch {
        expected: usize,
        actual: usize,
    },
    LayoutOverflow,
}

impl Display for SharedAudioLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorTooShort { min, actual } => write!(
                formatter,
                "shared audio descriptor requires at least {min} bytes, got {actual}"
            ),
            Self::InvalidDescriptorMagic { actual } => {
                write!(formatter, "invalid shared audio descriptor magic {actual}")
            }
            Self::UnsupportedDescriptorVersion { actual } => write!(
                formatter,
                "unsupported shared audio descriptor version {actual}"
            ),
            Self::InvalidDescriptorField {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid shared audio descriptor field {field}: expected {expected}, got {actual}"
            ),
            Self::CursorBlockTooShort { min, actual } => write!(
                formatter,
                "shared audio cursor block requires at least {min} bytes, got {actual}"
            ),
            Self::MemoryTooShort { min, actual } => write!(
                formatter,
                "shared audio memory requires at least {min} bytes, got {actual}"
            ),
            Self::ZeroSampleRate => {
                formatter.write_str("shared audio sample rate must be non-zero")
            }
            Self::ZeroBlockFrames => {
                formatter.write_str("shared audio block frames must be non-zero")
            }
            Self::ZeroCapacityBlocks => {
                formatter.write_str("shared audio capacity blocks must be non-zero")
            }
            Self::ChannelCountTooLarge { channels, max } => write!(
                formatter,
                "shared audio channel count {channels} exceeds max {max}"
            ),
            Self::CursorOrderInvalid {
                read_frame,
                write_frame,
            } => write!(
                formatter,
                "shared audio cursor read frame {read_frame} is ahead of write frame {write_frame}"
            ),
            Self::InsufficientReadableFrames {
                requested,
                available,
            } => write!(
                formatter,
                "shared audio read requested {requested} frames but only {available} are available"
            ),
            Self::InsufficientWritableFrames {
                requested,
                available,
            } => write!(
                formatter,
                "shared audio write requested {requested} frames but only {available} are available"
            ),
            Self::SampleBufferLengthMismatch { expected, actual } => write!(
                formatter,
                "shared audio sample buffer length mismatch: expected {expected} samples, got {actual}"
            ),
            Self::LayoutOverflow => formatter.write_str("shared audio layout size overflowed"),
        }
    }
}

impl Error for SharedAudioLayoutError {}
