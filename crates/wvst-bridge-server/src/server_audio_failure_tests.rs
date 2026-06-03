#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use super::*;
use std::path::PathBuf;
use std::time::Duration;
use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};

use crate::instance_registry::{StreamLifecycleParams, WorkerState};

#[cfg(unix)]
#[tokio::test]
async fn publishes_worker_failed_event_for_audio_process_error() {
    let worker_path = process_error_worker_script();
    let state = BridgeState {
        config: Arc::new(BridgeConfig::development(
            "127.0.0.1:0".parse().expect("valid bind addr"),
        )),
        host_worker: Arc::new(HostWorkerClient::new_for_test(
            PathBuf::from("missing-wvst-host-worker"),
            Duration::from_secs(5),
        )),
        instances: Arc::new(InstanceRegistry::new()),
        component_handler_events: Arc::new(
            crate::component_handler_events::ComponentHandlerEventPublisher::new(),
        ),
        events: BridgeEventBus::new(),
        metrics: Arc::new(BridgeMetrics::new()),
        plugins: Arc::new(PluginRegistry::new()),
        stream_tracker: Arc::new(AudioStreamTracker::new()),
        audio_in_flight: Arc::new(AudioInFlightLimiter::new()),
        shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
        shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
        workers: Arc::new(WorkerSupervisor::new_for_test_with_audio(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };
    let plugin = plugin_descriptor();
    let record = state
        .instances
        .create(instance_params(), &plugin)
        .expect("instance created");
    state
        .workers
        .start_instance(&record)
        .await
        .expect("worker started");
    state
        .instances
        .mark_processing(record.instance_id)
        .expect("processing");

    let response = process_binary_payload(audio_frame(record.stream_id), &state).await;
    let header = AudioFrameHeader::decode(&response).expect("diagnostic header");
    let failed = state.instances.get(record.instance_id).expect("record");
    let events = state.events.recent_since(None);
    let failed_event = events
        .into_iter()
        .find_map(|event| match event.kind {
            BridgeEventKind::WorkerFailed { error_data, .. } => error_data,
            _ => None,
        })
        .expect("worker failed event");

    assert!(header.flags.contains(AudioFrameFlags::SILENCE));
    assert!(header.flags.contains(AudioFrameFlags::PROCESS_ERROR));
    assert_eq!(failed.worker_state, WorkerState::Failed);
    assert_eq!(failed_event["schemaVersion"], 1);
    assert_eq!(failed_event["kind"], "worker-rejected");
    assert_eq!(
        failed_event["classification"]["category"],
        "worker-rejection"
    );
    assert_eq!(failed_event["workerData"]["kind"], "vst3-runtime-process");
    assert_eq!(failed_event["workerData"]["stage"], "component.process");

    let _ = state.instances.close_stream(StreamLifecycleParams {
        instance_id: record.instance_id,
    });
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

fn instance_params() -> crate::instance_registry::InstanceCreateParams {
    crate::instance_registry::InstanceCreateParams {
        plugin_id: "vst3:test".to_string(),
        class_id: Some("class-a".to_string()),
        sample_rate: 48_000,
        max_block_frames: 128,
        input_channels: 2,
        output_channels: 2,
    }
}

fn plugin_descriptor() -> wvst_scanner::PluginDescriptor {
    wvst_scanner::PluginDescriptor {
        plugin_id: "vst3:test".to_string(),
        name: "Test".to_string(),
        vendor: None,
        version: None,
        path: "/tmp/Test.vst3".to_string(),
        format: wvst_scanner::PluginFormat::Vst3,
        classes: vec![wvst_scanner::PluginClass {
            class_id: Some("class-a".to_string()),
            name: "Test Class".to_string(),
            category: Some("Fx".to_string()),
            subcategories: Vec::new(),
        }],
        metadata_source: wvst_scanner::MetadataSource::ModuleInfo,
    }
}

fn audio_frame(stream_id: u64) -> Vec<u8> {
    let samples = [0.25_f32, 0.5, -0.25, -0.5];
    let header = AudioFrameHeader::new_f32(
        StreamId::new(stream_id),
        10,
        512,
        SampleRate::new(48_000).expect("sample rate"),
        FrameCount::new(2).expect("frames"),
        ChannelCount::new(2).expect("channels"),
        AudioFrameFlags::empty(),
    )
    .expect("header");
    let mut frame = vec![0; AUDIO_FRAME_HEADER_LEN + header.payload_len as usize];
    header
        .encode(&mut frame[..AUDIO_FRAME_HEADER_LEN])
        .expect("encode header");
    let mut offset = AUDIO_FRAME_HEADER_LEN;
    for sample in samples {
        frame[offset..offset + 4].copy_from_slice(&sample.to_le_bytes());
        offset += 4;
    }
    frame
}

#[cfg(unix)]
fn process_error_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("process-error-worker.py");
    std::fs::write(
        &worker,
        r#"#!/usr/bin/env python3
import json
import socket
import struct
import sys
import threading

MAGIC = int.from_bytes(b"WVCI", "little")
VERSION = 1
HEADER_LEN = 24
KIND_RESPONSE = 2
KIND_ERROR = 3
MAX_BODY = 16 * 1024 * 1024

def read_exact(stream, size):
    data = stream.read(size) if hasattr(stream, "read") else stream.recv(size)
    if not data:
        return None
    while len(data) < size:
        chunk = stream.read(size - len(data)) if hasattr(stream, "read") else stream.recv(size - len(data))
        if not chunk:
            return None
        data += chunk
    return data

def audio_loop(address):
    host, port = address.rsplit(":", 1)
    sock = socket.create_connection((host, int(port)))
    while True:
        header = read_exact(sock, HEADER_LEN)
        if header is None:
            return
        magic, version, header_len, kind, status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
        body = read_exact(sock, body_len)
        if body is None:
            return
        error = {
            "message": "VST3 process failed",
            "data": {
                "kind": "vst3-runtime-process",
                "stage": "component.process",
                "hostError": "audio-processor-call-failed",
                "message": "VST3 process failed",
            },
        }
        payload = json.dumps(error).encode("utf-8")
        response = struct.pack("<IHHHHIQ", magic, version, header_len, 3, 4220, len(payload), sequence) + payload
        sock.sendall(response)

def pack_frame(kind, status, sequence, body):
    return struct.pack("<IHHHHIQ", MAGIC, VERSION, HEADER_LEN, kind, status, len(body), sequence) + body

def ok(request, result):
    body = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}, separators=(",", ":")).encode()
    return pack_frame(KIND_RESPONSE, 0, request["_sequence"], body)

def hello(request):
    return ok(request, {"workerName": "test-worker", "ipcVersion": 1, "capabilities": {"instanceLifecycle": True, "binaryAudioProcess": True, "framedControlIpc": True, "framedControlIpcVersion": 1, "framedControlMaxBodyBytes": MAX_BODY, "framedControlSequenceIds": True, "framedControlStatusCodes": True, "framedControlErrorResponses": True, "framedControlBatching": True}})

def ready(state="ready"):
    return {"instanceId": 1, "streamId": 1, "workerState": state, "backend": "vst3-runtime", "latencySamples": 0, "tailSamples": 0, "tailInfo": {"samples": 0, "kind": "none", "finiteSamples": 0}}

args = sys.argv[1:]
if args and args[0] == "serve-framed":
    args = args[1:]
while args:
    if args[0] == "--audio-connect":
        threading.Thread(target=audio_loop, args=(args[1],), daemon=True).start()
        args = args[2:]
    else:
        args = args[1:]

while True:
    header = read_exact(sys.stdin.buffer, HEADER_LEN)
    if header is None:
        break
    _magic, _version, _header_len, _kind, _status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
    body = read_exact(sys.stdin.buffer, body_len)
    if body is None:
        break
    request = json.loads(body.decode())
    request["_sequence"] = sequence
    method = request.get("method", "")
    if method == "worker.hello":
        frame = hello(request)
    elif method == "instance.create":
        frame = ok(request, ready())
    else:
        frame = ok(request, {"instanceId": 1, "streamId": 1, "workerState": "processing"})
    sys.stdout.buffer.write(frame)
    sys.stdout.buffer.flush()
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
    std::env::temp_dir().join(format!("wvst-server-audio-failure-test-{suffix}"))
}
