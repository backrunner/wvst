use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolError {
    BufferTooSmall { min: usize, actual: usize },
    InvalidMagic(u32),
    InvalidWorkerAudioIpcMagic(u32),
    UnsupportedWorkerAudioIpcVersion(u16),
    UnsupportedAudioFrameVersion(u16),
    InvalidHeaderLength(u16),
    InvalidSampleFormat(u8),
    InvalidWorkerAudioMessageKind(u16),
    InvalidMidiEventKind(u8),
    InvalidMidiChannel(u8),
    InvalidMidiData { field: &'static str, value: u8 },
    InvalidParameterValue(f64),
    InvalidPayloadLength { expected: u32, actual: u32 },
    PayloadTooLarge,
    InvalidCoreValue(String),
}

impl Display for ProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferTooSmall { min, actual } => {
                write!(
                    formatter,
                    "buffer too small: need {min} bytes, got {actual}"
                )
            }
            Self::InvalidMagic(value) => write!(formatter, "invalid audio frame magic: {value:#x}"),
            Self::InvalidWorkerAudioIpcMagic(value) => {
                write!(formatter, "invalid worker audio IPC magic: {value:#x}")
            }
            Self::UnsupportedWorkerAudioIpcVersion(value) => {
                write!(formatter, "unsupported worker audio IPC version: {value}")
            }
            Self::UnsupportedAudioFrameVersion(value) => {
                write!(formatter, "unsupported audio frame version: {value}")
            }
            Self::InvalidHeaderLength(value) => write!(formatter, "invalid header length: {value}"),
            Self::InvalidSampleFormat(value) => write!(formatter, "invalid sample format: {value}"),
            Self::InvalidWorkerAudioMessageKind(value) => {
                write!(formatter, "invalid worker audio message kind: {value}")
            }
            Self::InvalidMidiEventKind(value) => {
                write!(formatter, "invalid MIDI event kind: {value}")
            }
            Self::InvalidMidiChannel(value) => {
                write!(formatter, "invalid MIDI channel: {value}")
            }
            Self::InvalidMidiData { field, value } => {
                write!(formatter, "invalid MIDI {field}: {value}")
            }
            Self::InvalidParameterValue(value) => {
                write!(formatter, "invalid normalized parameter value: {value}")
            }
            Self::InvalidPayloadLength { expected, actual } => {
                write!(
                    formatter,
                    "invalid payload length: expected {expected}, got {actual}"
                )
            }
            Self::PayloadTooLarge => formatter.write_str("protocol payload is too large"),
            Self::InvalidCoreValue(message) => formatter.write_str(message),
        }
    }
}

impl Error for ProtocolError {}

impl From<wvst_core::CoreError> for ProtocolError {
    fn from(value: wvst_core::CoreError) -> Self {
        Self::InvalidCoreValue(value.to_string())
    }
}
