use std::path::{Path, PathBuf};

use super::*;
use crate::instance_registry::{InstanceState, RuntimeTailInfo, WorkerRuntimeInfo, WorkerState};
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

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let root_mode = std::fs::metadata(&root)
            .expect("root metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = std::fs::metadata(&descriptor.path)
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(root_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }

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

#[tokio::test]
async fn serializes_shared_memory_lifecycle_per_instance() {
    let registry = std::sync::Arc::new(SharedMemoryStreamRegistry::new_in(unique_root()));
    let first = registry.lock_instance(7).await.expect("first lock");
    let second_registry = std::sync::Arc::clone(&registry);
    let second =
        tokio::spawn(async move { second_registry.lock_instance(7).await.expect("second lock") });

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), async {
            while !second.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_err()
    );

    drop(first);
    tokio::time::timeout(std::time::Duration::from_secs(1), second)
        .await
        .expect("second lock completes")
        .expect("second task");
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

#[cfg(unix)]
#[test]
fn rejects_preexisting_stream_symlink_without_truncating_target() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = unique_root();
    std::fs::create_dir(&root).expect("root");
    let mut permissions = std::fs::metadata(&root).expect("metadata").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&root, permissions).expect("permissions");
    let target = root.with_extension("target");
    std::fs::write(&target, b"keep-me").expect("target");
    symlink(&target, root.join("stream-9-instance-7.wvst-shm")).expect("symlink");
    let registry = SharedMemoryStreamRegistry::new_in(&root);

    assert!(matches!(
        registry.create_for_instance(&test_record(StreamState::Open), Some(2)),
        Err(SharedMemoryStreamError::Mmap(
            SharedAudioMmapError::Io { .. }
        ))
    ));
    assert_eq!(std::fs::read(&target).expect("target bytes"), b"keep-me");

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(target);
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
        input_bus_index: None,
        output_bus_index: None,
        state: InstanceState::Ready,
        worker_state: WorkerState::Ready,
        stream_state,
        backend: Some("vst3-runtime".to_string()),
        controller_class_id: None,
        runtime_capabilities: RuntimeCapabilities::default(),
        latency_samples: 0,
        tail_samples: 0,
        tail_info: RuntimeTailInfo::from_samples(0),
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
