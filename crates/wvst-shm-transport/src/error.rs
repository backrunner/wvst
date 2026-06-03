use std::error::Error;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SharedAudioLayoutError {
    ZeroSampleRate,
    ZeroBlockFrames,
    ZeroCapacityBlocks,
    ChannelCountTooLarge { channels: u16, max: u16 },
    CursorOrderInvalid { read_frame: u64, write_frame: u64 },
    InsufficientReadableFrames { requested: u64, available: u64 },
    InsufficientWritableFrames { requested: u64, available: u64 },
    LayoutOverflow,
}

impl Display for SharedAudioLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::LayoutOverflow => formatter.write_str("shared audio layout size overflowed"),
        }
    }
}

impl Error for SharedAudioLayoutError {}
