use super::*;
use crate::instance_registry::{InstanceState, StreamState, WorkerState};
use crate::metrics::BridgeMetrics;

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

#[tokio::test]
async fn releases_quarantine_after_duration() {
    let supervisor = WorkerSupervisor::new_for_test_with_quarantine(
        PathBuf::from("missing-wvst-worker"),
        Duration::from_millis(50),
        Duration::from_millis(1),
    );
    let record = record();

    for _ in 0..QUARANTINE_FAILURES {
        let _ = supervisor.start_instance(&record).await;
    }
    assert_eq!(
        supervisor.quarantine_failures(&record.plugin_id).await,
        Some(QUARANTINE_FAILURES)
    );

    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        supervisor
            .release_expired_quarantine(&record.plugin_id)
            .await,
        Some(QUARANTINE_FAILURES)
    );
    assert_eq!(
        supervisor.quarantine_failures(&record.plugin_id).await,
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_start_when_instance_limit_is_reached() {
    let worker = ready_worker_script();
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_max_instances(1),
    );
    supervisor
        .start_instance(&record())
        .await
        .expect("first instance starts");
    let second = InstanceRecord {
        instance_id: 2,
        stream_id: 2,
        ..record()
    };

    let error = supervisor
        .start_instance(&second)
        .await
        .expect_err("limit reached");

    assert!(matches!(
        error,
        WorkerSupervisorError::ResourceLimitExceeded {
            limit: 1,
            active: 1,
        }
    ));

    let _ = supervisor.destroy_instance(1).await;
    let _ = std::fs::remove_dir_all(worker.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn records_worker_shutdown_audit_when_destroying_instance() {
    let worker = ready_worker_script();
    let metrics = Arc::new(BridgeMetrics::new());
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_metrics(Arc::clone(&metrics)),
    );

    supervisor
        .start_instance(&record())
        .await
        .expect("instance starts");
    supervisor
        .destroy_instance(1)
        .await
        .expect("instance destroyed");

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.worker_shutdowns, 1);
    assert!(snapshot.worker_kill_requests <= 1);
    assert!(snapshot.worker_tree_kill_requests <= 1);
    assert_eq!(snapshot.worker_forced_kill_requests, 0);
    assert_eq!(snapshot.worker_wait_successes, 1);
    assert_eq!(snapshot.worker_wait_timeouts, 0);

    let _ = std::fs::remove_dir_all(worker.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn kills_worker_process_group_when_destroying_instance() {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let child_pid_file = directory.join("child.pid");
    let worker = worker_with_child_script(&directory, &child_pid_file);
    let metrics = Arc::new(BridgeMetrics::new());
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false)
            .with_metrics(Arc::clone(&metrics)),
    );

    supervisor
        .start_instance(&record())
        .await
        .expect("instance starts");
    let child_pid = read_child_pid(&child_pid_file).await;
    assert!(process_is_alive(child_pid));

    supervisor
        .destroy_instance(1)
        .await
        .expect("instance destroyed");

    assert!(
        wait_until_process_exits(child_pid).await,
        "worker child process {child_pid} was not cleaned up"
    );
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.worker_shutdowns, 1);
    assert_eq!(snapshot.worker_tree_kill_requests, 1);
    assert_eq!(snapshot.worker_forced_kill_requests, 0);

    let _ = std::fs::remove_dir_all(directory);
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

#[cfg(unix)]
#[tokio::test]
async fn preserves_worker_rejection_data() {
    let worker = reject_create_worker_script();
    let supervisor = WorkerSupervisor::new_for_test(worker.clone(), Duration::from_secs(5));

    let error = supervisor
        .start_instance(&record())
        .await
        .expect_err("worker rejected create");
    let data = error.rpc_data();

    assert_eq!(data["kind"], "worker-rejected");
    assert_eq!(data["workerData"]["kind"], "vst3-runtime-init");
    assert_eq!(data["workerData"]["stage"], "controller.initialize");

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
        stream_state: StreamState::Open,
        backend: None,
        controller_class_id: None,
        runtime_capabilities: crate::runtime_capabilities::RuntimeCapabilities::default(),
        latency_samples: 0,
        tail_samples: 0,
    }
}

#[cfg(unix)]
fn worker_with_child_script(
    directory: &std::path::Path,
    child_pid_file: &std::path::Path,
) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let worker = directory.join("worker-with-child.sh");
    std::fs::write(
        &worker,
        format!(
            r#"#!/bin/sh
sleep 60 &
child_pid=$!
printf '%s\n' "$child_pid" > '{child_pid_file}'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{{"jsonrpc":"2.0","id":%s,"result":{{"workerName":"child-worker","ipcVersion":1,"capabilities":{{"instanceLifecycle":true,"binaryAudioProcess":true}}}}}}\n' "$id" ;;
    *instance.create*) printf '{{"jsonrpc":"2.0","id":%s,"result":{{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough","latencySamples":0,"tailSamples":0}}}}\n' "$id" ;;
    *instance.destroy*) printf '{{"jsonrpc":"2.0","id":%s,"result":{{"workerState":"destroyed"}}}}\n' "$id"; while :; do sleep 1; done ;;
    *) printf '{{"jsonrpc":"2.0","id":%s,"result":{{}}}}\n' "$id" ;;
  esac
done
"#,
            child_pid_file = child_pid_file.display()
        ),
    )
    .expect("script");
    let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&worker, permissions).expect("permissions");
    worker
}

#[cfg(unix)]
async fn read_child_pid(path: &std::path::Path) -> u32 {
    for _ in 0..100 {
        if let Ok(pid) = std::fs::read_to_string(path)
            .map(|value| value.trim().to_string())
            .and_then(|value| value.parse::<u32>().map_err(std::io::Error::other))
        {
            return pid;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    panic!("child pid was not written to {}", path.display());
}

#[cfg(unix)]
async fn wait_until_process_exits(pid: u32) -> bool {
    for _ in 0..100 {
        if !process_is_alive(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    false
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    rustix::process::Pid::from_raw(pid as i32)
        .is_some_and(|pid| rustix::process::test_kill_process(pid).is_ok())
}

#[cfg(unix)]
fn reject_create_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("reject-create-worker.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"reject-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"error":{"code":4220,"message":"controller init failed","data":{"kind":"vst3-runtime-init","stage":"controller.initialize","hostError":"edit-controller-call-failed","message":"controller init failed"}}}\n' "$id" ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id" ;;
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
fn ready_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("ready-worker.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"ready-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough","latencySamples":0,"tailSamples":0}}\n' "$id" ;;
    *instance.destroy*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerState":"destroyed"}}\n' "$id"; exit 0 ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"result":{}}\n' "$id" ;;
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
