use super::*;
use crate::instance_registry::{InstanceState, WorkerState};

#[tokio::test]
async fn quarantines_plugin_after_repeated_start_failures() {
    let supervisor = WorkerSupervisor::new_for_test(
        PathBuf::from("missing-wvst-worker"),
        Duration::from_millis(50),
    );
    let record = record();

    for _ in 0..QUARANTINE_FAILURES {
        assert!(matches!(
            supervisor.start_instance(&record).await,
            Err(WorkerSupervisorError::Spawn { .. })
        ));
    }

    let error = supervisor
        .start_instance(&record)
        .await
        .expect_err("quarantined");

    assert!(matches!(
        error,
        WorkerSupervisorError::Quarantined {
            failures: QUARANTINE_FAILURES,
            ..
        }
    ));
}

fn record() -> InstanceRecord {
    InstanceRecord {
        instance_id: 1,
        stream_id: 1,
        plugin_id: "vst3:test".to_string(),
        plugin_path: "/tmp/Test.vst3".to_string(),
        class_id: Some("class-a".to_string()),
        class_name: Some("Test".to_string()),
        sample_rate: 48_000,
        max_block_frames: 128,
        input_channels: 2,
        output_channels: 2,
        state: InstanceState::Allocated,
        worker_state: WorkerState::NotStarted,
    }
}
