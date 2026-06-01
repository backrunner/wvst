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

#[cfg(unix)]
#[tokio::test]
async fn rejects_incompatible_worker_ipc_version() {
    let worker = incompatible_worker_script();
    let supervisor = WorkerSupervisor::new_for_test(worker.clone(), Duration::from_secs(5));

    let error = supervisor
        .start_instance(&record())
        .await
        .expect_err("incompatible worker");

    assert!(matches!(
        error,
        WorkerSupervisorError::IncompatibleWorker {
            expected_ipc_version: EXPECTED_WORKER_IPC_VERSION,
            actual_ipc_version: Some(99),
            ..
        }
    ));

    let _ = std::fs::remove_dir_all(worker.parent().expect("worker parent"));
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

#[cfg(unix)]
fn incompatible_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("incompatible-worker.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"bad-worker","ipcVersion":99,"capabilities":{"instanceLifecycle":true}}}\n' "$id" ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":-32601,"message":"unknown"}}\n' "$id" ;;
  esac
done
"#,
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&worker, permissions).expect("permissions");
    worker
}

#[cfg(unix)]
fn unique_temp_dir() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("wvst-worker-supervisor-test-{suffix}"))
}
