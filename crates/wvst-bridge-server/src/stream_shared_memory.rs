use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_shm_mmap::{SharedAudioMmap, SharedAudioMmapError};
use wvst_shm_transport::{
    SharedAudioLayoutError, SharedAudioRingCursorState, SharedAudioRingLayout,
    SharedAudioTransportConfig,
};

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
    mmap: SharedAudioMmap,
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryStreamStatus {
    pub descriptor: SharedMemoryStreamDescriptor,
    pub path_exists: bool,
    pub input: SharedMemoryRingStatus,
    pub output: SharedMemoryRingStatus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMemoryRingStatus {
    pub cursor: SharedAudioRingCursorState,
    pub readable_frames: u64,
    pub writable_frames: u64,
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

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSharedMemoryStatusParams {
    pub instance_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSharedMemoryProcessParams {
    pub instance_id: u64,
    #[serde(default)]
    pub frames: Option<u16>,
    #[serde(default)]
    pub midi_events: Vec<Value>,
    #[serde(default)]
    pub parameter_events: Vec<Value>,
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
                mmap,
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

    pub fn status_by_instance(
        &self,
        instance_id: u64,
    ) -> Result<Option<SharedMemoryStreamStatus>, SharedMemoryStreamError> {
        let records = self
            .records
            .lock()
            .map_err(|_| SharedMemoryStreamError::RegistryUnavailable)?;
        records
            .values()
            .find(|record| record.descriptor.instance_id == instance_id)
            .map(status_from_record)
            .transpose()
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
    drop(record.mmap);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(descriptor),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(descriptor),
        Err(source) => Err(SharedMemoryStreamError::Io { path, source }),
    }
}

fn status_from_record(
    record: &SharedMemoryStreamRecord,
) -> Result<SharedMemoryStreamStatus, SharedMemoryStreamError> {
    Ok(SharedMemoryStreamStatus {
        descriptor: record.descriptor.clone(),
        path_exists: record.path.exists(),
        input: ring_status(record.descriptor.layout.input, record.mmap.memory())?,
        output: ring_status(record.descriptor.layout.output, record.mmap.memory())?,
    })
}

fn ring_status(
    ring: SharedAudioRingLayout,
    memory: &[u8],
) -> Result<SharedMemoryRingStatus, SharedMemoryStreamError> {
    let cursor = ring.cursor_state(memory)?;
    Ok(SharedMemoryRingStatus {
        cursor,
        readable_frames: ring.readable_frames(cursor.cursor())?,
        writable_frames: ring.writable_frames(cursor.cursor())?,
    })
}

#[cfg(test)]
#[path = "stream_shared_memory_tests.rs"]
mod tests;
