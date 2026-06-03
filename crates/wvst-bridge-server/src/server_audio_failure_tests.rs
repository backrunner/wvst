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
    let worker = directory.join("process-error-worker.sh");
    std::fs::write(
        &worker,
        r#"#!/bin/sh
audio_addr=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --audio-connect) audio_addr="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -n "$audio_addr" ]; then
  python3 - "$audio_addr" <<'PY' &
import json
import socket
import struct
import sys

addr = sys.argv[1]
host, port = addr.rsplit(":", 1)
sock = socket.create_connection((host, int(port)))
while True:
    header = sock.recv(24)
    if not header:
        break
    while len(header) < 24:
        chunk = sock.recv(24 - len(header))
        if not chunk:
            raise SystemExit(0)
        header += chunk
    magic, version, header_len, kind, status, body_len, sequence = struct.unpack("<IHHHHIQ", header)
    body = b""
    while len(body) < body_len:
        chunk = sock.recv(body_len - len(body))
        if not chunk:
            raise SystemExit(0)
        body += chunk
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
PY
fi
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"vst3-runtime","latencySamples":0,"tailSamples":0}}\n' "$id" ;;
    *) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"processing"}}\n' "$id" ;;
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
    std::env::temp_dir().join(format!("wvst-server-audio-failure-test-{suffix}"))
}
