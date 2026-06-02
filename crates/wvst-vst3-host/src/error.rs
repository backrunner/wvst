use std::error::Error;
use std::fmt::{Display, Formatter};

pub type HostResult<T> = Result<T, HostError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HostError {
    AudioProcessorCallFailed {
        method: &'static str,
        result: i32,
    },
    AudioProcessorReturnedNull,
    AudioProcessorVTableMissing,
    BundleExecutableNotFound(String),
    FactoryCallFailed {
        method: &'static str,
        result: i32,
    },
    FactoryReturnedNull,
    InstanceCreationFailed {
        class_id: String,
        interface_id: String,
        result: i32,
    },
    InstanceReturnedNull {
        class_id: String,
        interface_id: String,
    },
    InvalidClassId(String),
    InvalidLifecycleTransition {
        from: &'static str,
        action: &'static str,
    },
    InvalidMaxBlockFrames(u16),
    InvalidSampleRate(u32),
    MissingSymbol(&'static str),
    ModuleLoadFailed(String),
    UnsupportedPlatform(&'static str),
    InvalidChannelCount {
        input: usize,
        output: usize,
    },
    InvalidBufferLength {
        expected: usize,
        actual: usize,
    },
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AudioProcessorCallFailed { method, result } => {
                write!(
                    formatter,
                    "VST3 audio processor call failed: {method} returned {result}"
                )
            }
            Self::AudioProcessorReturnedNull => {
                formatter.write_str("VST3 audio processor pointer is null")
            }
            Self::AudioProcessorVTableMissing => {
                formatter.write_str("VST3 audio processor vtable is null")
            }
            Self::BundleExecutableNotFound(path) => {
                write!(formatter, "VST3 executable not found in bundle: {path}")
            }
            Self::FactoryCallFailed { method, result } => {
                write!(
                    formatter,
                    "VST3 factory call failed: {method} returned {result}"
                )
            }
            Self::FactoryReturnedNull => formatter.write_str("GetPluginFactory returned null"),
            Self::InstanceCreationFailed {
                class_id,
                interface_id,
                result,
            } => {
                write!(
                    formatter,
                    "VST3 createInstance failed for class {class_id} and interface {interface_id}: {result}"
                )
            }
            Self::InstanceReturnedNull {
                class_id,
                interface_id,
            } => {
                write!(
                    formatter,
                    "VST3 createInstance returned null for class {class_id} and interface {interface_id}"
                )
            }
            Self::InvalidClassId(class_id) => {
                write!(formatter, "invalid VST3 class id: {class_id}")
            }
            Self::InvalidLifecycleTransition { from, action } => {
                write!(
                    formatter,
                    "invalid VST3 lifecycle transition: cannot {action} from {from}"
                )
            }
            Self::InvalidMaxBlockFrames(value) => {
                write!(formatter, "invalid VST3 max block frame count: {value}")
            }
            Self::InvalidSampleRate(value) => {
                write!(formatter, "invalid VST3 sample rate: {value}")
            }
            Self::MissingSymbol(symbol) => write!(formatter, "missing VST3 symbol: {symbol}"),
            Self::ModuleLoadFailed(message) => write!(formatter, "module load failed: {message}"),
            Self::UnsupportedPlatform(message) => formatter.write_str(message),
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
