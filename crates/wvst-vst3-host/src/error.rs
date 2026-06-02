use std::error::Error;
use std::fmt::{Display, Formatter};

pub type HostResult<T> = Result<T, HostError>;

#[derive(Debug, Clone, PartialEq)]
pub enum HostError {
    AudioProcessorCallFailed {
        method: &'static str,
        result: i32,
    },
    AudioProcessorReturnedNull,
    AudioProcessorVTableMissing,
    BundleExecutableNotFound(String),
    ComponentCallFailed {
        method: &'static str,
        result: i32,
    },
    ComponentReturnedNull,
    ComponentVTableMissing,
    ConnectionPointCallFailed {
        method: &'static str,
        result: i32,
    },
    ConnectionPointMessageNull,
    ConnectionPointReturnedNull,
    ConnectionPointVTableMissing,
    EditControllerCallFailed {
        method: &'static str,
        result: i32,
    },
    EditControllerReturnedNull,
    EditControllerVTableMissing,
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
    InterfaceQueryFailed {
        interface_id: String,
        result: i32,
    },
    InterfaceReturnedNull {
        interface_id: String,
    },
    InvalidInterfaceId(String),
    InvalidMaxBlockFrames(u16),
    InvalidSampleRate(u32),
    UnsupportedSpeakerArrangement(u16),
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
    InvalidStateStreamSeek {
        position: i64,
    },
    InvalidEventCount {
        max: usize,
        actual: usize,
    },
    InvalidEventSampleOffset {
        frames: usize,
        actual: usize,
    },
    InvalidParameterChangeCount {
        max: usize,
        actual: usize,
    },
    InvalidParameterChangeValue {
        parameter_id: u32,
        value: f64,
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
            Self::ComponentCallFailed { method, result } => {
                write!(
                    formatter,
                    "VST3 component call failed: {method} returned {result}"
                )
            }
            Self::ComponentReturnedNull => formatter.write_str("VST3 component pointer is null"),
            Self::ComponentVTableMissing => formatter.write_str("VST3 component vtable is null"),
            Self::ConnectionPointCallFailed { method, result } => {
                write!(
                    formatter,
                    "VST3 connection point call failed: {method} returned {result}"
                )
            }
            Self::ConnectionPointMessageNull => {
                formatter.write_str("VST3 connection point message pointer is null")
            }
            Self::ConnectionPointReturnedNull => {
                formatter.write_str("VST3 connection point pointer is null")
            }
            Self::ConnectionPointVTableMissing => {
                formatter.write_str("VST3 connection point vtable is null")
            }
            Self::EditControllerCallFailed { method, result } => {
                write!(
                    formatter,
                    "VST3 edit controller call failed: {method} returned {result}"
                )
            }
            Self::EditControllerReturnedNull => {
                formatter.write_str("VST3 edit controller pointer is null")
            }
            Self::EditControllerVTableMissing => {
                formatter.write_str("VST3 edit controller vtable is null")
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
            Self::InterfaceQueryFailed {
                interface_id,
                result,
            } => {
                write!(
                    formatter,
                    "VST3 queryInterface failed for interface {interface_id}: {result}"
                )
            }
            Self::InterfaceReturnedNull { interface_id } => {
                write!(
                    formatter,
                    "VST3 queryInterface returned null for interface {interface_id}"
                )
            }
            Self::InvalidInterfaceId(interface_id) => {
                write!(formatter, "invalid VST3 interface id: {interface_id}")
            }
            Self::InvalidMaxBlockFrames(value) => {
                write!(formatter, "invalid VST3 max block frame count: {value}")
            }
            Self::InvalidSampleRate(value) => {
                write!(formatter, "invalid VST3 sample rate: {value}")
            }
            Self::UnsupportedSpeakerArrangement(channels) => {
                write!(
                    formatter,
                    "unsupported VST3 speaker arrangement for channel count: {channels}"
                )
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
            Self::InvalidStateStreamSeek { position } => {
                write!(
                    formatter,
                    "invalid VST3 state stream seek position: {position}"
                )
            }
            Self::InvalidEventCount { max, actual } => {
                write!(
                    formatter,
                    "invalid VST3 event count: max {max}, got {actual}"
                )
            }
            Self::InvalidEventSampleOffset { frames, actual } => {
                write!(
                    formatter,
                    "invalid VST3 event sample offset: frames={frames}, offset={actual}"
                )
            }
            Self::InvalidParameterChangeCount { max, actual } => {
                write!(
                    formatter,
                    "invalid VST3 parameter change count: max {max}, got {actual}"
                )
            }
            Self::InvalidParameterChangeValue {
                parameter_id,
                value,
            } => {
                write!(
                    formatter,
                    "invalid normalized VST3 parameter value: parameter={parameter_id}, value={value}"
                )
            }
        }
    }
}

impl Error for HostError {}
