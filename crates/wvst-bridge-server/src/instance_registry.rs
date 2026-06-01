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
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceState {
    Allocated,
    Destroyed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerState {
    NotStarted,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDestroyResult {
    pub instance_id: u64,
    pub stream_id: u64,
    pub state: InstanceState,
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

    plugin
        .classes
        .iter()
        .find(|class| class.class_id.as_deref() == Some(requested_class_id))
        .ok_or_else(|| InstanceError::ClassNotFound(requested_class_id.to_string()))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wvst_scanner::{MetadataSource, PluginFormat};

    #[test]
    fn creates_multiple_records_for_same_plugin() {
        let registry = InstanceRegistry::new();
        let plugin = plugin();
        let first = registry
            .create(create_params(), &plugin)
            .expect("first instance");
        let second = registry
            .create(create_params(), &plugin)
            .expect("second instance");

        assert_eq!(first.instance_id, 1);
        assert_eq!(first.stream_id, 1);
        assert_eq!(second.instance_id, 2);
        assert_eq!(second.stream_id, 2);
        assert_eq!(registry.list().len(), 2);
    }

    #[test]
    fn destroys_record_by_instance_id() {
        let registry = InstanceRegistry::new();
        let plugin = plugin();
        let record = registry.create(create_params(), &plugin).expect("instance");

        let destroyed = registry
            .destroy(InstanceDestroyParams {
                instance_id: record.instance_id,
            })
            .expect("destroyed");

        assert_eq!(destroyed.state, InstanceState::Destroyed);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn rejects_invalid_sample_rate() {
        let registry = InstanceRegistry::new();
        let plugin = plugin();
        let mut params = create_params();
        params.sample_rate = 1;

        assert_eq!(
            registry.create(params, &plugin),
            Err(InstanceError::InvalidSampleRate(1))
        );
    }

    fn create_params() -> InstanceCreateParams {
        InstanceCreateParams {
            plugin_id: "vst3:test".to_string(),
            class_id: Some("class-a".to_string()),
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
        }
    }

    fn plugin() -> PluginDescriptor {
        PluginDescriptor {
            plugin_id: "vst3:test".to_string(),
            format: PluginFormat::Vst3,
            name: "Test".to_string(),
            vendor: Some("WVST".to_string()),
            version: Some("0.1.0".to_string()),
            path: "/tmp/Test.vst3".to_string(),
            classes: vec![PluginClass {
                class_id: Some("class-a".to_string()),
                name: "Test Class".to_string(),
                category: Some("Fx".to_string()),
                subcategories: Vec::new(),
            }],
            metadata_source: MetadataSource::ModuleInfo,
        }
    }
}
