use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_core::audio::MAX_CHANNEL_COUNT;
use wvst_core::{ChannelCount, FrameCount, InstanceId, SampleRate, StreamId};
use wvst_scanner::{PluginClass, PluginDescriptor};

#[derive(Debug)]
pub struct InstanceRegistry {
    next_instance_id: AtomicU64,
    next_stream_id: AtomicU64,
    records: Mutex<BTreeMap<u64, InstanceRecord>>,
}

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
    pub state: InstanceState,
    pub worker_state: WorkerState,
    pub stream_state: StreamState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_class_id: Option<String>,
    pub latency_samples: u32,
    pub tail_samples: u32,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceState {
    Allocated,
    Ready,
    Processing,
    Stopped,
    Failed,
    Destroyed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerState {
    NotStarted,
    Ready,
    Processing,
    Stopped,
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
    pub latency_samples: u32,
    pub tail_samples: u32,
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
            latency_samples: json_u32(value, "latencySamples"),
            tail_samples: json_u32(value, "tailSamples"),
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
    InstanceNotFound(u64),
    RegistryUnavailable,
}

impl InstanceRegistry {
    pub fn new() -> Self {
        Self {
            next_instance_id: AtomicU64::new(1),
            next_stream_id: AtomicU64::new(1),
            records: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn create(
        &self,
        params: InstanceCreateParams,
        plugin: &PluginDescriptor,
    ) -> Result<InstanceRecord, InstanceError> {
        validate_create_params(&params)?;

        if params.plugin_id != plugin.plugin_id {
            return Err(InstanceError::PluginNotFound(params.plugin_id));
        }

        let class = select_class(plugin, params.class_id.as_deref())?;
        let instance_id = InstanceId::new(self.next_instance_id.fetch_add(1, Ordering::Relaxed));
        let stream_id = StreamId::new(self.next_stream_id.fetch_add(1, Ordering::Relaxed));
        let record = InstanceRecord {
            instance_id: instance_id.get(),
            stream_id: stream_id.get(),
            plugin_id: plugin.plugin_id.clone(),
            plugin_path: plugin.path.clone(),
            class_id: params
                .class_id
                .or_else(|| class.and_then(|item| item.class_id.clone())),
            class_name: class.map(|item| item.name.clone()),
            sample_rate: params.sample_rate,
            max_block_frames: params.max_block_frames,
            input_channels: params.input_channels,
            output_channels: params.output_channels,
            state: InstanceState::Allocated,
            worker_state: WorkerState::NotStarted,
            stream_state: StreamState::Open,
            backend: None,
            controller_class_id: None,
            latency_samples: 0,
            tail_samples: 0,
        };

        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        records.insert(record.instance_id, record.clone());

        Ok(record)
    }

    pub fn list(&self) -> Vec<InstanceRecord> {
        self.records
            .lock()
            .map(|records| records.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        self.records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?
            .get(&instance_id)
            .cloned()
            .ok_or(InstanceError::InstanceNotFound(instance_id))
    }

    pub fn find_by_stream_id(&self, stream_id: u64) -> Option<InstanceRecord> {
        self.records
            .lock()
            .ok()?
            .values()
            .find(|record| record.stream_id == stream_id)
            .cloned()
    }

    pub fn find_open_by_stream_id(&self, stream_id: u64) -> Option<InstanceRecord> {
        self.records
            .lock()
            .ok()?
            .values()
            .find(|record| {
                record.stream_id == stream_id && record.stream_state == StreamState::Open
            })
            .cloned()
    }

    pub fn open_stream(
        &self,
        params: StreamLifecycleParams,
    ) -> Result<InstanceRecord, InstanceError> {
        self.set_stream_state(params.instance_id, StreamState::Open)
    }

    pub fn close_stream(
        &self,
        params: StreamLifecycleParams,
    ) -> Result<InstanceRecord, InstanceError> {
        self.set_stream_state(params.instance_id, StreamState::Closed)
    }

    pub fn mark_worker_ready(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        self.mark_worker_ready_inner(instance_id, None)
    }

    pub fn mark_worker_ready_with_runtime(
        &self,
        instance_id: u64,
        runtime: WorkerRuntimeInfo,
    ) -> Result<InstanceRecord, InstanceError> {
        self.mark_worker_ready_inner(instance_id, Some(runtime))
    }

    fn mark_worker_ready_inner(
        &self,
        instance_id: u64,
        runtime: Option<WorkerRuntimeInfo>,
    ) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        match record.state {
            InstanceState::Processing => record.worker_state = WorkerState::Processing,
            InstanceState::Stopped => record.worker_state = WorkerState::Stopped,
            _ => {
                record.state = InstanceState::Ready;
                record.worker_state = WorkerState::Ready;
            }
        }
        if let Some(runtime) = runtime {
            record.backend = runtime.backend;
            record.controller_class_id = runtime.controller_class_id;
            record.latency_samples = runtime.latency_samples;
            record.tail_samples = runtime.tail_samples;
        }

        Ok(record.clone())
    }

    pub fn mark_worker_failed(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Failed;
        record.worker_state = WorkerState::Failed;

        Ok(record.clone())
    }

    pub fn mark_processing(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Processing;
        record.worker_state = WorkerState::Processing;

        Ok(record.clone())
    }

    pub fn mark_stopped(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Stopped;
        record.worker_state = WorkerState::Stopped;

        Ok(record.clone())
    }

    pub fn destroy(
        &self,
        params: InstanceDestroyParams,
    ) -> Result<InstanceDestroyResult, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .remove(&params.instance_id)
            .ok_or(InstanceError::InstanceNotFound(params.instance_id))?;

        Ok(InstanceDestroyResult {
            instance_id: record.instance_id,
            stream_id: record.stream_id,
            state: InstanceState::Destroyed,
        })
    }

    fn set_stream_state(
        &self,
        instance_id: u64,
        stream_state: StreamState,
    ) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.stream_state = stream_state;

        Ok(record.clone())
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::PluginNotFound(_) | Self::ClassNotFound(_) | Self::InstanceNotFound(_) => 4040,
            Self::RegistryUnavailable => 5034,
            _ => 4220,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::InvalidPluginId => "pluginId is required".to_string(),
            Self::PluginNotFound(plugin_id) => {
                format!("plugin not found in registry: {plugin_id}")
            }
            Self::ClassNotFound(class_id) => format!("plugin class not found: {class_id}"),
            Self::InvalidSampleRate(value) => format!("invalid sampleRate: {value}"),
            Self::InvalidMaxBlockFrames(value) => {
                format!("invalid maxBlockFrames: {value}")
            }
            Self::InvalidInputChannels(value) => format!("invalid inputChannels: {value}"),
            Self::InvalidOutputChannels(value) => format!("invalid outputChannels: {value}"),
            Self::InstanceNotFound(instance_id) => {
                format!("instance not found: {instance_id}")
            }
            Self::RegistryUnavailable => "instance registry unavailable".to_string(),
        }
    }

    pub fn rpc_data(&self) -> Value {
        match self {
            Self::InvalidPluginId => json!({ "kind": "invalid-plugin-id" }),
            Self::PluginNotFound(plugin_id) => {
                json!({ "kind": "plugin-not-found", "pluginId": plugin_id })
            }
            Self::ClassNotFound(class_id) => {
                json!({ "kind": "class-not-found", "classId": class_id })
            }
            Self::InvalidSampleRate(value) => {
                json!({ "kind": "invalid-sample-rate", "sampleRate": value })
            }
            Self::InvalidMaxBlockFrames(value) => {
                json!({ "kind": "invalid-max-block-frames", "maxBlockFrames": value })
            }
            Self::InvalidInputChannels(value) => {
                json!({ "kind": "invalid-input-channels", "inputChannels": value })
            }
            Self::InvalidOutputChannels(value) => {
                json!({ "kind": "invalid-output-channels", "outputChannels": value })
            }
            Self::InstanceNotFound(instance_id) => {
                json!({ "kind": "instance-not-found", "instanceId": instance_id })
            }
            Self::RegistryUnavailable => json!({ "kind": "registry-unavailable" }),
        }
    }
}

fn validate_create_params(params: &InstanceCreateParams) -> Result<(), InstanceError> {
    if params.plugin_id.is_empty() {
        return Err(InstanceError::InvalidPluginId);
    }

    SampleRate::new(params.sample_rate)
        .map_err(|_| InstanceError::InvalidSampleRate(params.sample_rate))?;
    FrameCount::new(params.max_block_frames)
        .map_err(|_| InstanceError::InvalidMaxBlockFrames(params.max_block_frames))?;

    if params.input_channels > MAX_CHANNEL_COUNT {
        return Err(InstanceError::InvalidInputChannels(params.input_channels));
    }

    ChannelCount::new(params.output_channels)
        .map_err(|_| InstanceError::InvalidOutputChannels(params.output_channels))?;

    Ok(())
}

fn json_u32(value: &Value, key: &'static str) -> u32 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn select_class<'a>(
    plugin: &'a PluginDescriptor,
    requested_class_id: Option<&str>,
) -> Result<Option<&'a PluginClass>, InstanceError> {
    let Some(requested_class_id) = requested_class_id else {
        return Ok(plugin.classes.first());
    };

    plugin
        .classes
        .iter()
        .find(|class| class.class_id.as_deref() == Some(requested_class_id))
        .ok_or_else(|| InstanceError::ClassNotFound(requested_class_id.to_string()))
        .map(Some)
}

#[cfg(test)]
#[path = "instance_registry_tests.rs"]
mod tests;
