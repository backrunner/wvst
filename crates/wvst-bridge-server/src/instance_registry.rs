use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use wvst_core::audio::MAX_CHANNEL_COUNT;
use wvst_core::{ChannelCount, FrameCount, InstanceId, SampleRate, StreamId};
use wvst_scanner::{PluginClass, PluginDescriptor};

use crate::runtime_capabilities::RuntimeCapabilities;

mod types;

pub use types::*;

#[derive(Debug)]
pub struct InstanceRegistry {
    next_instance_id: AtomicU64,
    next_stream_id: AtomicU64,
    records: Mutex<BTreeMap<u64, InstanceRecord>>,
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
            runtime_capabilities: RuntimeCapabilities::default(),
            latency_samples: 0,
            tail_samples: 0,
            tail_info: RuntimeTailInfo::default(),
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

    pub fn idle_worker_reclaim_candidate(
        &self,
        plugin_id: &str,
        exclude_instance_id: u64,
    ) -> Option<InstanceRecord> {
        self.records
            .lock()
            .ok()?
            .values()
            .find(|record| {
                record.plugin_id == plugin_id
                    && record.instance_id != exclude_instance_id
                    && is_reclaimable_idle_worker(record)
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

    pub fn mark_worker_starting(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Starting;
        record.worker_state = WorkerState::Starting;

        Ok(record.clone())
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
            record.runtime_capabilities = runtime.runtime_capabilities;
            record.latency_samples = runtime.latency_samples;
            record.tail_samples = runtime.tail_samples;
            record.tail_info = runtime.tail_info;
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

    pub fn mark_worker_recovering(
        &self,
        instance_id: u64,
    ) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Recovering;
        record.worker_state = WorkerState::Recovering;

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

    pub fn mark_stopping(&self, instance_id: u64) -> Result<InstanceRecord, InstanceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| InstanceError::RegistryUnavailable)?;
        let record = records
            .get_mut(&instance_id)
            .ok_or(InstanceError::InstanceNotFound(instance_id))?;

        record.state = InstanceState::Stopping;
        record.worker_state = WorkerState::Stopping;

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

fn select_class<'a>(
    plugin: &'a PluginDescriptor,
    requested_class_id: Option<&str>,
) -> Result<Option<&'a PluginClass>, InstanceError> {
    let Some(requested_class_id) = requested_class_id else {
        return Ok(plugin.classes.first());
    };
    if requested_class_id.is_empty() {
        return Err(InstanceError::ClassNotFound(requested_class_id.to_string()));
    }

    Ok(plugin
        .classes
        .iter()
        .find(|class| class.class_id.as_deref() == Some(requested_class_id)))
}

fn is_reclaimable_idle_worker(record: &InstanceRecord) -> bool {
    matches!(record.stream_state, StreamState::Closed)
        && matches!(record.state, InstanceState::Ready | InstanceState::Stopped)
        && matches!(
            record.worker_state,
            WorkerState::Ready | WorkerState::Stopped
        )
}

#[cfg(test)]
#[path = "instance_registry_tests.rs"]
mod tests;
