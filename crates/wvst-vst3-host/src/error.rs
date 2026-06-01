use std::error::Error;
use std::fmt::{Display, Formatter};

pub type HostResult<T> = Result<T, HostError>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HostError {
    BundleExecutableNotFound(String),
    FactoryCallFailed { method: &'static str, result: i32 },
    FactoryReturnedNull,
    MissingSymbol(&'static str),
    ModuleLoadFailed(String),
    UnsupportedPlatform(&'static str),
    InvalidChannelCount { input: usize, output: usize },
    InvalidBufferLength { expected: usize, actual: usize },
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
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
