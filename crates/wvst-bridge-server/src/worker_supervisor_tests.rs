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

    for _ in 0..DEFAULT_QUARANTINE_FAILURE_THRESHOLD {
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
        &error,
        WorkerSupervisorError::Quarantined {
            failures: DEFAULT_QUARANTINE_FAILURE_THRESHOLD,
            ..
        }
    ));
    assert!(
        error.rpc_data()["releaseAfterMs"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );
    let status = supervisor
        .quarantine_status(&record.plugin_id)
        .await
        .expect("quarantine status");
    assert_eq!(status.failures, DEFAULT_QUARANTINE_FAILURE_THRESHOLD);
    assert!(status.release_after_ms > 0);
}

#[tokio::test]
async fn quarantines_after_configured_failure_threshold() {
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(PathBuf::from("missing-wvst-worker"))
            .with_timeout(Duration::from_millis(50))
            .with_quarantine_failure_threshold(2)
            .with_audio_ipc(false),
    );
    let record = record();

    for _ in 0..2 {
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
        &error,
        WorkerSupervisorError::Quarantined { failures: 2, .. }
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

    for _ in 0..DEFAULT_QUARANTINE_FAILURE_THRESHOLD {
        let _ = supervisor.start_instance(&record).await;
    }
    assert_eq!(
        supervisor.quarantine_failures(&record.plugin_id).await,
        Some(DEFAULT_QUARANTINE_FAILURE_THRESHOLD)
    );

    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        supervisor
            .release_expired_quarantine(&record.plugin_id)
            .await,
        Some(DEFAULT_QUARANTINE_FAILURE_THRESHOLD)
    );
    assert_eq!(
        supervisor.quarantine_failures(&record.plugin_id).await,
        None
    );
}

#[tokio::test]
async fn real_worker_rejects_missing_vst3_bundle() {
    let Some(worker) = option_env!("CARGO_BIN_EXE_wvst-host-worker") else {
        return;
    };
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(PathBuf::from(worker))
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false),
    );

    let error = supervisor
        .start_instance(&record())
        .await
        .expect_err("worker rejects non-bundle plugin path");

    assert!(matches!(
        &error,
        WorkerSupervisorError::WorkerRejected { code: 4220, .. }
    ));
    assert!(
        error
            .rpc_message()
            .contains("pluginPath must point to a VST3 bundle directory")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn refreshes_metadata_with_framed_control_batch() {
    let worker = ready_worker_script();
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(worker.clone())
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false),
    );

    supervisor
        .start_instance(&record())
        .await
        .expect("framed worker starts");
    let metadata = supervisor
        .metadata_refresh(1, false, true)
        .await
        .expect("metadata refresh");

    assert_eq!(metadata["instanceId"], 1);
    assert_eq!(metadata["parameters"], serde_json::json!([]));
    assert_eq!(metadata["unitInfo"], Value::Null);
    assert_eq!(metadata["state"], Value::Null);
    assert_eq!(metadata["worker"]["instances"], 1);

    let _ = supervisor.destroy_instance(1).await;
    let _ = std::fs::remove_dir_all(worker.parent().expect("worker parent"));
}

#[tokio::test]
async fn framed_control_ipc_preserves_worker_rejections() {
    let Some(worker) = option_env!("CARGO_BIN_EXE_wvst-host-worker") else {
        return;
    };
    let supervisor = WorkerSupervisor::with_options(
        WorkerSupervisorOptions::new(PathBuf::from(worker))
            .with_timeout(Duration::from_secs(5))
            .with_audio_ipc(false),
    );
    let record = InstanceRecord {
        sample_rate: 0,
        ..record()
    };

    let error = supervisor
        .start_instance(&record)
        .await
        .expect_err("worker rejects invalid sample rate");

    assert!(matches!(
        error,
        WorkerSupervisorError::WorkerRejected { code: 4220, .. }
    ));
    assert_eq!(error.rpc_data()["kind"], "worker-rejected");
}

#[test]
fn validates_framed_control_response_headers() {
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::Response, 0, 7, 0),
            7,
            "worker.metrics",
        )
        .is_ok()
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::ErrorResponse, 4220, 7, 0),
            7,
            "worker.metrics",
        )
        .is_ok()
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::Response, 4220, 7, 0),
            7,
            "worker.metrics",
        )
        .expect_err("nonzero response status")
        .contains("nonzero status")
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::ErrorResponse, 0, 7, 0),
            7,
            "worker.metrics",
        )
        .expect_err("zero error status")
        .contains("zero status")
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::Request, 0, 7, 0),
            7,
            "worker.metrics",
        )
        .expect_err("request while responding")
        .contains("request frame")
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::BatchResponse, 0, 7, 0),
            7,
            "worker.metrics",
        )
        .expect_err("batch response to non-batch request")
        .contains("batch response")
    );
    assert!(
        validate_framed_response_header(
            &WorkerControlIpcHeader::new(WorkerControlMessageKind::Response, 0, 8, 0),
            7,
            "worker.metrics",
        )
        .expect_err("sequence mismatch")
        .contains("sequence mismatch")
    );
    assert!(
        validate_framed_response_body_len(WORKER_CONTROL_IPC_MAX_BODY_LEN + 1)
            .expect_err("oversized body")
            .contains("body too large")
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
        tail_info: crate::instance_registry::RuntimeTailInfo::from_samples(0),
    }
}

#[cfg(unix)]
fn worker_with_child_script(
    directory: &std::path::Path,
    child_pid_file: &std::path::Path,
) -> PathBuf {
    let worker = directory.join("worker-with-child.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{CHILD_WORKER_FIXTURE}")
        .replace("__CHILD_PID_FILE__", &child_pid_file.to_string_lossy());
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn reject_create_worker_script() -> PathBuf {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("reject-create-worker.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{REJECT_CREATE_WORKER_FIXTURE}");
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn incompatible_worker_script() -> PathBuf {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("incompatible-worker.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{INCOMPATIBLE_WORKER_FIXTURE}");
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn ready_worker_script() -> PathBuf {
    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("ready-worker.py");
    let source = format!("{FRAMED_WORKER_PREAMBLE}{READY_WORKER_FIXTURE}");
    write_framed_worker_script(&worker, &source);
    worker
}

#[cfg(unix)]
fn write_framed_worker_script(path: &std::path::Path, source: &str) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, source).expect("script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}

#[cfg(unix)]
const FRAMED_WORKER_PREAMBLE: &str = r#"#!/usr/bin/env python3
import json
import os
import struct
import subprocess
import sys
import time

MAGIC = int.from_bytes(b"WVCI", "little")
VERSION = 1
HEADER_LEN = 24
KIND_RESPONSE = 2
KIND_ERROR = 3
MAX_BODY = 16 * 1024 * 1024

def read_exact(size):
    data = sys.stdin.buffer.read(size)
    if not data:
        return None
    while len(data) < size:
        chunk = sys.stdin.buffer.read(size - len(data))
        if not chunk:
            return None
        data += chunk
    return data

def pack_frame(kind, status, sequence, body):
    return struct.pack("<IHHHHIQ", MAGIC, VERSION, HEADER_LEN, kind, status, len(body), sequence) + body

def ok(request, result):
    body = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}, separators=(",", ":")).encode()
    return KIND_RESPONSE, 0, body, False

def err(request, code, message, data=None):
    payload = {"jsonrpc": "2.0", "id": request.get("id"), "error": {"code": code, "message": message}}
    if data is not None:
        payload["error"]["data"] = data
    body = json.dumps(payload, separators=(",", ":")).encode()
    return KIND_ERROR, code & 0xFFFF, body, False

def hello_result(name="test-worker", ipc_version=1, framed=True):
    caps = {"instanceLifecycle": True, "binaryAudioProcess": True}
    if framed:
        caps.update({
            "framedControlIpc": True,
            "framedControlIpcVersion": 1,
            "framedControlMaxBodyBytes": MAX_BODY,
            "framedControlSequenceIds": True,
            "framedControlStatusCodes": True,
            "framedControlErrorResponses": True,
            "framedControlBatching": True,
        })
    return {"workerName": name, "ipcVersion": ipc_version, "capabilities": caps}

def ready_result(state="ready"):
    return {"instanceId": 1, "streamId": 1, "workerState": state, "backend": "vst3-runtime", "latencySamples": 0, "tailSamples": 0, "tailInfo": {"samples": 0, "kind": "none", "finiteSamples": 0}}

def runtime_metrics():
    return {
        "ipcVersion": 1,
        "instances": 1,
        "processingInstances": 0,
        "vst3RuntimeInstances": 1,
        "runtime": [{
            "streamId": 1,
            "backend": "vst3-runtime",
            "runtimeCapabilities": {"schemaVersion": 2, "binaryAudioProcess": True},
            "latencySamples": 0,
            "tailSamples": 0,
            "tailInfo": {"samples": 0, "kind": "none", "finiteSamples": 0},
            "sharedMemory": None,
            "diagnostics": {},
        }],
    }

def run():
    while True:
        header = read_exact(HEADER_LEN)
        if header is None:
            return
        _magic, _version, _header_len, kind, _status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
        body = read_exact(body_len)
        if body is None:
            return
        rkind, rstatus, rbody, should_exit = handle_frame(kind, body)
        sys.stdout.buffer.write(pack_frame(rkind, rstatus, sequence, rbody))
        sys.stdout.buffer.flush()
        if should_exit:
            return

def handle_frame(kind, body):
    if kind == 4:
        responses = []
        should_exit = False
        offset = 0
        while offset < len(body):
            header = body[offset:offset + HEADER_LEN]
            _magic, _version, _header_len, child_kind, _status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
            offset += HEADER_LEN
            child_body = body[offset:offset + body_len]
            offset += body_len
            request = json.loads(child_body.decode())
            rkind, rstatus, rbody, child_exit = handle(request)
            responses.append(pack_frame(rkind, rstatus, sequence, rbody))
            should_exit = should_exit or child_exit
        return 5, 0, b"".join(responses), should_exit

    request = json.loads(body.decode())
    return handle(request)

"#;

#[cfg(unix)]
const READY_WORKER_FIXTURE: &str = r#"
def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return ok(request, hello_result("ready-worker"))
    if method == "instance.create":
        return ok(request, ready_result())
    if method == "instance.parameters":
        return ok(request, {"instanceId": 1, "parameters": []})
    if method == "instance.units":
        return ok(request, {"instanceId": 1, "unitInfo": None})
    if method == "worker.metrics":
        return ok(request, runtime_metrics())
    if method == "instance.destroy":
        kind, status, body, _ = ok(request, {"workerState": "destroyed"})
        return kind, status, body, True
    return ok(request, {})

run()
"#;

#[cfg(unix)]
const REJECT_CREATE_WORKER_FIXTURE: &str = r#"
def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return ok(request, hello_result("reject-worker"))
    if method == "instance.create":
        return err(request, 4220, "controller init failed", {"kind": "vst3-runtime-init", "stage": "controller.initialize", "hostError": "edit-controller-call-failed", "message": "controller init failed"})
    return ok(request, {})

run()
"#;

#[cfg(unix)]
const INCOMPATIBLE_WORKER_FIXTURE: &str = r#"
def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return ok(request, hello_result("bad-worker", 99, False))
    return err(request, -32601, "unknown")

run()
"#;

#[cfg(unix)]
const CHILD_WORKER_FIXTURE: &str = r#"
CHILD_PID_FILE = "__CHILD_PID_FILE__"
child = subprocess.Popen(["sleep", "60"])
with open(CHILD_PID_FILE, "w") as file:
    file.write(str(child.pid))

def handle(request):
    method = request.get("method", "")
    if method == "worker.hello":
        return ok(request, hello_result("child-worker"))
    if method == "instance.create":
        return ok(request, ready_result())
    if method == "instance.destroy":
        kind, status, body, _ = ok(request, {"workerState": "destroyed"})
        return kind, status, body, False
    return ok(request, {})

run()
while True:
    time.sleep(1)
"#;

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
fn unique_temp_dir() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("wvst-worker-supervisor-test-{suffix}"))
}
