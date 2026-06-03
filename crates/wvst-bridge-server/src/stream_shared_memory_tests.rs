use std::path::{Path, PathBuf};

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

    let status = registry
        .status_by_instance(record.instance_id)
        .expect("status")
        .expect("status record");
    assert_eq!(status.descriptor, descriptor);
    assert!(status.path_exists);
    assert_eq!(status.input.readable_frames, 0);
    assert_eq!(status.input.writable_frames, 256);
    assert_eq!(status.output.readable_frames, 0);
    assert_eq!(status.output.writable_frames, 256);
    assert_eq!(status.input.cursor.read_frame, 0);
    assert_eq!(status.input.cursor.write_frame, 0);

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
