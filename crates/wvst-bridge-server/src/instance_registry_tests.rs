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
    assert_eq!(first.backend, None);
    assert_eq!(first.controller_class_id, None);
    assert_eq!(first.latency_samples, 0);
    assert_eq!(first.tail_samples, 0);
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
fn marks_worker_ready() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let starting = registry
        .mark_worker_starting(record.instance_id)
        .expect("starting");
    assert_eq!(starting.state, InstanceState::Starting);
    assert_eq!(starting.worker_state, WorkerState::Starting);

    let ready = registry
        .mark_worker_ready(record.instance_id)
        .expect("ready");

    assert_eq!(ready.state, InstanceState::Ready);
    assert_eq!(ready.worker_state, WorkerState::Ready);
}

#[test]
fn records_worker_runtime_info() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let ready = registry
        .mark_worker_ready_with_runtime(
            record.instance_id,
            WorkerRuntimeInfo {
                backend: Some("vst3-runtime".to_string()),
                controller_class_id: Some("2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a".to_string()),
                latency_samples: 64,
                tail_samples: 128,
            },
        )
        .expect("ready");

    assert_eq!(ready.backend.as_deref(), Some("vst3-runtime"));
    assert_eq!(
        ready.controller_class_id.as_deref(),
        Some("2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")
    );
    assert_eq!(ready.latency_samples, 64);
    assert_eq!(ready.tail_samples, 128);
    assert_eq!(registry.list()[0].latency_samples, 64);
}

#[test]
fn opens_and_closes_stream() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let closed = registry
        .close_stream(StreamLifecycleParams {
            instance_id: record.instance_id,
        })
        .expect("closed");
    assert_eq!(closed.stream_state, StreamState::Closed);
    assert!(registry.find_open_by_stream_id(record.stream_id).is_none());

    let open = registry
        .open_stream(StreamLifecycleParams {
            instance_id: record.instance_id,
        })
        .expect("open");
    assert_eq!(open.stream_state, StreamState::Open);
    assert_eq!(
        registry
            .find_open_by_stream_id(record.stream_id)
            .expect("stream")
            .instance_id,
        record.instance_id
    );
}

#[test]
fn marks_worker_failed() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let failed = registry
        .mark_worker_failed(record.instance_id)
        .expect("failed");

    assert_eq!(failed.state, InstanceState::Failed);
    assert_eq!(failed.worker_state, WorkerState::Failed);
}

#[test]
fn marks_worker_recovering() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let recovering = registry
        .mark_worker_recovering(record.instance_id)
        .expect("recovering");

    assert_eq!(recovering.state, InstanceState::Recovering);
    assert_eq!(recovering.worker_state, WorkerState::Recovering);
}

#[test]
fn marks_processing_and_stopped() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    let processing = registry
        .mark_processing(record.instance_id)
        .expect("processing");
    assert_eq!(processing.state, InstanceState::Processing);
    assert_eq!(processing.worker_state, WorkerState::Processing);

    let stopping = registry
        .mark_stopping(record.instance_id)
        .expect("stopping");
    assert_eq!(stopping.state, InstanceState::Stopping);
    assert_eq!(stopping.worker_state, WorkerState::Stopping);

    let stopped = registry.mark_stopped(record.instance_id).expect("stopped");
    assert_eq!(stopped.state, InstanceState::Stopped);
    assert_eq!(stopped.worker_state, WorkerState::Stopped);
}

#[test]
fn heartbeat_ready_preserves_processing_state() {
    let registry = InstanceRegistry::new();
    let plugin = plugin();
    let record = registry.create(create_params(), &plugin).expect("instance");

    registry
        .mark_processing(record.instance_id)
        .expect("processing");
    let heartbeat = registry
        .mark_worker_ready(record.instance_id)
        .expect("heartbeat");

    assert_eq!(heartbeat.state, InstanceState::Processing);
    assert_eq!(heartbeat.worker_state, WorkerState::Processing);
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
