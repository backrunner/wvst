use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_shm_mmap::{SharedAudioMmap, SharedAudioMmapError};
use wvst_shm_transport::{SharedAudioLayoutError, SharedAudioTransportConfig};

use crate::instance_registry::{InstanceRecord, StreamState};

const DEFAULT_CAPACITY_BLOCKS: u32 = 4;
static REGISTRY_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct SharedMemoryStreamRegistry {
    root_dir: PathBuf,
    records: Mutex<BTreeMap<u64, SharedMemoryStreamRecord>>,
}

#[derive(Debug)]
struct SharedMemoryStreamRecord {
    descriptor: SharedMemoryStreamDescriptor,
    path: PathBuf,
    _mmap: SharedAudioMmap,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryStreamDescriptor {
    pub schema_version: u16,
    pub transport: String,
    pub instance_id: u64,
    pub stream_id: u64,
    pub path: String,
    pub total_bytes: u64,
    pub descriptor_bytes: Vec<u8>,
    pub layout: wvst_shm_transport::SharedAudioTransportLayout,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSharedMemoryCreateParams {
    pub instance_id: u64,
    #[serde(default)]
    pub capacity_blocks: Option<u32>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSharedMemoryDestroyParams {
    pub instance_id: u64,
}

#[derive(Debug)]
pub enum SharedMemoryStreamError {
    InvalidCapacityBlocks,
    StreamClosed {
        instance_id: u64,
    },
    RegistryUnavailable,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Layout(SharedAudioLayoutError),
    Mmap(SharedAudioMmapError),
}

impl SharedMemoryStreamRegistry {
    pub fn new() -> Self {
        let counter = REGISTRY_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self::new_in(
            std::env::temp_dir().join(format!("wvst-shm-{}-{counter}", std::process::id())),
        )
    }

    pub fn new_in(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            records: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn create_for_instance(
        &self,
        record: &InstanceRecord,
        capacity_blocks: Option<u32>,
    ) -> Result<SharedMemoryStreamDescriptor, SharedMemoryStreamError> {
        if record.stream_state != StreamState::Open {
            return Err(SharedMemoryStreamError::StreamClosed {
                instance_id: record.instance_id,
            });
        }
        let capacity_blocks = capacity_blocks.unwrap_or(DEFAULT_CAPACITY_BLOCKS);
        if capacity_blocks == 0 {
            return Err(SharedMemoryStreamError::InvalidCapacityBlocks);
        }

        self.destroy_by_stream_id(record.stream_id)?;
        std::fs::create_dir_all(&self.root_dir).map_err(|source| SharedMemoryStreamError::Io {
            path: self.root_dir.clone(),
            source,
        })?;

        let layout = SharedAudioTransportConfig::new(
            record.sample_rate,
            record.max_block_frames,
            capacity_blocks,
            record.input_channels,
            record.output_channels,
        )
        .layout()?;
        let path = self.path_for(record.instance_id, record.stream_id);
        let mmap = SharedAudioMmap::create(&path, &layout)?;
        let descriptor = SharedMemoryStreamDescriptor {
            schema_version: 1,
            transport: "file-backed-mmap".to_string(),
            instance_id: record.instance_id,
            stream_id: record.stream_id,
            path: path.display().to_string(),
            total_bytes: layout.total_bytes,
            descriptor_bytes: layout.descriptor_bytes().to_vec(),
            layout,
        };

        let mut records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryStreamError::RegistryUnavailable)?;
        records.insert(
            record.stream_id,
            SharedMemoryStreamRecord {
                descriptor: descriptor.clone(),
                path,
                _mmap: mmap,
            },
        );
        Ok(descriptor)
    }

    pub fn destroy_by_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<SharedMemoryStreamDescriptor>, SharedMemoryStreamError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryStreamError::RegistryUnavailable)?;
        let stream_id = records.iter().find_map(|(stream_id, record)| {
            (record.descriptor.instance_id == instance_id).then_some(*stream_id)
        });
        let Some(stream_id) = stream_id else {
            return Ok(None);
        };
        records.remove(&stream_id).map(cleanup_record).transpose()
    }

    pub fn destroy_by_stream_id(
        &self,
        stream_id: u64,
    ) -> Result<Option<SharedMemoryStreamDescriptor>, SharedMemoryStreamError> {
        let record = self
            .records
            .lock()
            .map_err(|_| SharedMemoryStreamError::RegistryUnavailable)?
            .remove(&stream_id);
        record.map(cleanup_record).transpose()
    }

    pub fn get_by_stream_id(&self, stream_id: u64) -> Option<SharedMemoryStreamDescriptor> {
        self.records
            .lock()
            .ok()?
            .get(&stream_id)
            .map(|record| record.descriptor.clone())
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn path_for(&self, instance_id: u64, stream_id: u64) -> PathBuf {
        self.root_dir.join(format!(
            "stream-{stream_id}-instance-{instance_id}.wvst-shm"
        ))
    }
}

impl Default for SharedMemoryStreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SharedMemoryStreamRegistry {
    fn drop(&mut self) {
        let records = match self.records.get_mut() {
            Ok(records) => records,
            Err(poisoned) => poisoned.into_inner(),
        };
        let records = std::mem::take(records);
        for record in records.into_values() {
            let _ = cleanup_record(record);
        }
    }
}

impl SharedMemoryStreamError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::StreamClosed { .. } => 4094,
            Self::RegistryUnavailable => 5035,
            Self::Io { .. } | Self::Mmap(_) => 5036,
            Self::InvalidCapacityBlocks | Self::Layout(_) => 4221,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::InvalidCapacityBlocks => "capacityBlocks must be non-zero".to_string(),
            Self::StreamClosed { instance_id } => {
                format!("stream is closed for instance {instance_id}")
            }
            Self::RegistryUnavailable => "shared memory stream registry unavailable".to_string(),
            Self::Io { path, source } => format!("{}: {source}", path.display()),
            Self::Layout(source) => source.to_string(),
            Self::Mmap(source) => source.to_string(),
        }
    }

    pub fn rpc_data(&self) -> Value {
        match self {
            Self::InvalidCapacityBlocks => {
                json!({ "kind": "invalid-capacity-blocks" })
            }
            Self::StreamClosed { instance_id } => {
                json!({ "kind": "stream-closed", "instanceId": instance_id })
            }
            Self::RegistryUnavailable => {
                json!({ "kind": "shared-memory-registry-unavailable" })
            }
            Self::Io { path, source } => {
                json!({ "kind": "shared-memory-io", "path": path, "message": source.to_string() })
            }
            Self::Layout(source) => {
                json!({ "kind": "shared-memory-layout", "message": source.to_string() })
            }
            Self::Mmap(source) => {
                json!({ "kind": "shared-memory-mmap", "message": source.to_string() })
            }
        }
    }
}

impl From<SharedAudioLayoutError> for SharedMemoryStreamError {
    fn from(source: SharedAudioLayoutError) -> Self {
        Self::Layout(source)
    }
}

impl From<SharedAudioMmapError> for SharedMemoryStreamError {
    fn from(source: SharedAudioMmapError) -> Self {
        Self::Mmap(source)
    }
}

fn cleanup_record(
    record: SharedMemoryStreamRecord,
) -> Result<SharedMemoryStreamDescriptor, SharedMemoryStreamError> {
    let descriptor = record.descriptor;
    let path = record.path;
    drop(record._mmap);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(descriptor),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(descriptor),
        Err(source) => Err(SharedMemoryStreamError::Io { path, source }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_registry::{InstanceState, WorkerRuntimeInfo, WorkerState};
    use crate::runtime_capabilities::RuntimeCapabilities;

    #[test]
    fn creates_and_destroys_shared_memory_for_stream() {
        let root = unique_root();
        let registry = SharedMemoryStreamRegistry::new_in(&root);
        let record = test_record(StreamState::Open);

        let descriptor = registry
            .create_for_instance(&record, Some(2))
            .expect("descriptor");

        assert_eq!(descriptor.instance_id, record.instance_id);
        assert_eq!(descriptor.stream_id, record.stream_id);
        assert_eq!(descriptor.layout.config.capacity_blocks, 2);
        assert_eq!(descriptor.layout.config.input_channels, 2);
        assert!(Path::new(&descriptor.path).exists());
        assert!(registry.get_by_stream_id(record.stream_id).is_some());

        let destroyed = registry
            .destroy_by_instance(record.instance_id)
            .expect("destroy")
            .expect("record");
        assert_eq!(destroyed.stream_id, record.stream_id);
        assert!(!Path::new(&descriptor.path).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_closed_streams_and_zero_capacity() {
        let registry = SharedMemoryStreamRegistry::new_in(unique_root());
        assert!(matches!(
            registry.create_for_instance(&test_record(StreamState::Closed), Some(2)),
            Err(SharedMemoryStreamError::StreamClosed { instance_id: 7 })
        ));
        assert!(matches!(
            registry.create_for_instance(&test_record(StreamState::Open), Some(0)),
            Err(SharedMemoryStreamError::InvalidCapacityBlocks)
        ));
    }

    #[test]
    fn drop_removes_remaining_shared_memory_files() {
        let root = unique_root();
        let path;
        {
            let registry = SharedMemoryStreamRegistry::new_in(&root);
            let descriptor = registry
                .create_for_instance(&test_record(StreamState::Open), Some(2))
                .expect("descriptor");
            path = PathBuf::from(&descriptor.path);
            assert!(path.exists());
        }

        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_record(stream_state: StreamState) -> InstanceRecord {
        let _ = WorkerRuntimeInfo::default();
        InstanceRecord {
            instance_id: 7,
            stream_id: 9,
            plugin_id: "plugin".to_string(),
            plugin_path: "/tmp/plugin.vst3".to_string(),
            class_id: Some("class".to_string()),
            class_name: Some("Class".to_string()),
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            state: InstanceState::Ready,
            worker_state: WorkerState::Ready,
            stream_state,
            backend: Some("passthrough".to_string()),
            controller_class_id: None,
            runtime_capabilities: RuntimeCapabilities::default(),
            latency_samples: 0,
            tail_samples: 0,
        }
    }

    fn unique_root() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "wvst-bridge-shm-test-{}-{nanos}",
            std::process::id()
        ))
    }
}
