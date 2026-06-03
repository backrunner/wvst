use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::runtime_capabilities::RuntimeCapabilities;

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceCreateParams {
    pub plugin_id: String,
    #[serde(default)]
    pub class_id: Option<String>,
    pub sample_rate: u32,
    pub max_block_frames: u16,
    pub input_channels: u16,
    pub output_channels: u16,
    #[serde(default)]
    pub input_bus_index: Option<i32>,
    #[serde(default)]
    pub output_bus_index: Option<i32>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDestroyParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatusParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRestartParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceProcessingParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceParameterParams {
    pub instance_id: u64,
    pub parameter_id: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceParameterInfoParams {
    pub instance_id: u64,
    pub parameter_id: u32,
    #[serde(default)]
    pub value_normalized: Option<f64>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceParameterValueByStringParams {
    pub instance_id: u64,
    pub parameter_id: u32,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceParameterNormalizedByPlainParams {
    pub instance_id: u64,
    pub parameter_id: u32,
    pub value_plain: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceParameterSetParams {
    pub instance_id: u64,
    pub parameter_id: u32,
    pub value_normalized: f64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceSetStateParams {
    pub instance_id: u64,
    #[serde(default)]
    pub component_state_base64: Option<String>,
    #[serde(default)]
    pub controller_state_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConnectionNotifyParams {
    pub instance_id: u64,
    pub message_id: String,
    #[serde(default = "empty_json_object")]
    pub attributes: Value,
}

fn empty_json_object() -> Value {
    json!({})
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSelectUnitParams {
    pub instance_id: u64,
    pub unit_id: i32,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUnitByBusParams {
    pub instance_id: u64,
    pub direction: String,
    pub bus_index: i32,
    pub channel: i32,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUnitProgramDataParams {
    pub instance_id: u64,
    pub list_or_unit_id: i32,
    pub program_index: i32,
    pub data_base64: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceProgramDataParams {
    pub instance_id: u64,
    pub list_id: i32,
    pub program_index: i32,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSetProgramDataParams {
    pub instance_id: u64,
    pub list_id: i32,
    pub program_index: i32,
    pub data_base64: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUnitDataParams {
    pub instance_id: u64,
    pub unit_id: i32,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSetUnitDataParams {
    pub instance_id: u64,
    pub unit_id: i32,
    pub data_base64: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamLifecycleParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRecord {
    pub instance_id: u64,
    pub stream_id: u64,
    pub plugin_id: String,
    pub plugin_path: String,
    pub class_id: Option<String>,
    pub class_name: Option<String>,
    pub sample_rate: u32,
    pub max_block_frames: u16,
    pub input_channels: u16,
    pub output_channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_bus_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_bus_index: Option<i32>,
    pub state: InstanceState,
    pub worker_state: WorkerState,
    pub stream_state: StreamState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_class_id: Option<String>,
    pub runtime_capabilities: RuntimeCapabilities,
    pub latency_samples: u32,
    pub tail_samples: u32,
    pub tail_info: RuntimeTailInfo,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeTailKind {
    None,
    Finite,
    Infinite,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTailInfo {
    pub samples: u32,
    pub kind: RuntimeTailKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finite_samples: Option<u32>,
}

impl RuntimeTailInfo {
    pub const fn from_samples(samples: u32) -> Self {
        match samples {
            0 => Self {
                samples,
                kind: RuntimeTailKind::None,
                finite_samples: Some(0),
            },
            u32::MAX => Self {
                samples,
                kind: RuntimeTailKind::Infinite,
                finite_samples: None,
            },
            samples => Self {
                samples,
                kind: RuntimeTailKind::Finite,
                finite_samples: Some(samples),
            },
        }
    }

    fn from_worker_result(value: &Value) -> Self {
        let samples = value
            .get("tailInfo")
            .and_then(|tail| tail.get("samples"))
            .and_then(Value::as_u64)
            .and_then(|samples| u32::try_from(samples).ok())
            .unwrap_or(0);
        Self::from_samples(samples)
    }
}

impl Default for RuntimeTailInfo {
    fn default() -> Self {
        Self::from_samples(0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceState {
    Allocated,
    Starting,
    Ready,
    Processing,
    Stopping,
    Stopped,
    Recovering,
    Failed,
    Destroyed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerState {
    NotStarted,
    Starting,
    Ready,
    Processing,
    Stopping,
    Stopped,
    Recovering,
    Failed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDestroyResult {
    pub instance_id: u64,
    pub stream_id: u64,
    pub state: InstanceState,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct WorkerRuntimeInfo {
    pub backend: Option<String>,
    pub controller_class_id: Option<String>,
    pub runtime_capabilities: RuntimeCapabilities,
    pub latency_samples: u32,
    pub tail_samples: u32,
    pub tail_info: RuntimeTailInfo,
}

impl WorkerRuntimeInfo {
    pub fn from_worker_result(value: &Value) -> Self {
        Self {
            backend: value
                .get("backend")
                .and_then(Value::as_str)
                .map(str::to_string),
            controller_class_id: value
                .get("controllerClassId")
                .and_then(Value::as_str)
                .map(str::to_string),
            runtime_capabilities: RuntimeCapabilities::from_worker_result(
                value.get("runtimeCapabilities"),
            ),
            latency_samples: json_u32(value, "latencySamples"),
            tail_samples: json_u32(value, "tailSamples"),
            tail_info: RuntimeTailInfo::from_worker_result(value),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InstanceError {
    InvalidPluginId,
    PluginNotFound(String),
    ClassNotFound(String),
    InvalidSampleRate(u32),
    InvalidMaxBlockFrames(u16),
    InvalidInputChannels(u16),
    InvalidOutputChannels(u16),
    InvalidInputBusIndex(i32),
    InvalidOutputBusIndex(i32),
    InstanceNotFound(u64),
    RegistryUnavailable,
}

fn json_u32(value: &Value, key: &'static str) -> u32 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}
