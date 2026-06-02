use super::*;
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::connect_async;
use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};

use crate::instance_registry::StreamLifecycleParams;

#[test]
fn control_message_too_large_response_is_structured() {
    let response = control_message_too_large_response(4, 5);
    let value: serde_json::Value = serde_json::from_str(&response).expect("json");

    assert_eq!(value["error"]["code"], 4130);
    assert_eq!(value["error"]["data"]["kind"], "control-message-too-large");
    assert_eq!(value["error"]["data"]["maxBytes"], 4);
    assert_eq!(value["error"]["data"]["actualBytes"], 5);
}

#[tokio::test]
async fn responds_to_hello_and_echoes_binary_frames() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let server = BridgeServer::bind(config).await.expect("server binds");
    let addr = server.local_addr().expect("local addr");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    tokio::spawn(async move {
        let _ = server
            .serve_until(async {
                let _ = shutdown_receiver.await;
            })
            .await;
    });

    let (mut websocket, _) = connect_async(format!("ws://{addr}"))
        .await
        .expect("client connects");

    websocket
        .send(Message::Text(r#"{"id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#.into()))
        .await
        .expect("hello sends");

    let hello = websocket
        .next()
        .await
        .expect("hello response")
        .expect("valid websocket message");
    assert!(hello.to_text().expect("text").contains("wvst-bridge"));

    websocket
        .send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .expect("binary sends");

    let echoed = websocket
        .next()
        .await
        .expect("echoed response")
        .expect("valid websocket message");
    assert_eq!(echoed.into_data(), vec![1, 2, 3]);

    let _ = shutdown_sender.send(());
}

#[tokio::test]
async fn rejects_oversized_control_messages() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
        .with_max_control_message_bytes(8);
    let server = BridgeServer::bind(config).await.expect("server binds");
    let addr = server.local_addr().expect("local addr");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    tokio::spawn(async move {
        let _ = server
            .serve_until(async {
                let _ = shutdown_receiver.await;
            })
            .await;
    });

    let (mut websocket, _) = connect_async(format!("ws://{addr}"))
        .await
        .expect("client connects");

    websocket
        .send(Message::Text(r#"{"id":1,"method":"bridge.hello"}"#.into()))
        .await
        .expect("oversized control sends");

    let response = websocket
        .next()
        .await
        .expect("oversized response")
        .expect("valid websocket message");
    let value: serde_json::Value =
        serde_json::from_str(response.to_text().expect("response text")).expect("response json");

    assert_eq!(value["id"], serde_json::Value::Null);
    assert_eq!(value["error"]["code"], 4130);
    assert_eq!(value["error"]["data"]["kind"], "control-message-too-large");

    let _ = shutdown_sender.send(());
}

#[tokio::test]
async fn pushes_bridge_events_after_hello() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let events = BridgeEventBus::new();
    let server = BridgeServer::bind_with_host_worker_and_events(
        config,
        HostWorkerClient::from_env(),
        events.clone(),
    )
    .await
    .expect("server binds");
    let addr = server.local_addr().expect("local addr");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    tokio::spawn(async move {
        let _ = server
            .serve_until(async {
                let _ = shutdown_receiver.await;
            })
            .await;
    });

    let (mut websocket, _) = connect_async(format!("ws://{addr}"))
        .await
        .expect("client connects");

    websocket
        .send(Message::Text(hello_request(1).into()))
        .await
        .expect("hello sends");
    let hello = websocket
        .next()
        .await
        .expect("hello response")
        .expect("valid websocket message");
    let hello_value: serde_json::Value =
        serde_json::from_str(hello.to_text().expect("hello text")).expect("hello json");
    assert_eq!(hello_value["id"], 1);

    events.emit(BridgeEventKind::WorkerProcessing { instance_id: 7 });

    let pushed = websocket
        .next()
        .await
        .expect("event notification")
        .expect("valid websocket message");
    let pushed_value: serde_json::Value =
        serde_json::from_str(pushed.to_text().expect("event text")).expect("event json");
    assert_eq!(pushed_value["jsonrpc"], "2.0");
    assert_eq!(pushed_value["method"], "bridge.event");
    assert_eq!(
        pushed_value["params"]["event"]["kind"]["type"],
        "worker-processing"
    );
    assert_eq!(pushed_value["params"]["event"]["kind"]["instanceId"], 7);

    let _ = shutdown_sender.send(());
}

#[tokio::test]
async fn does_not_push_events_emitted_before_hello() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let events = BridgeEventBus::new();
    let server = BridgeServer::bind_with_host_worker_and_events(
        config,
        HostWorkerClient::from_env(),
        events.clone(),
    )
    .await
    .expect("server binds");
    let addr = server.local_addr().expect("local addr");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    tokio::spawn(async move {
        let _ = server
            .serve_until(async {
                let _ = shutdown_receiver.await;
            })
            .await;
    });

    let (mut websocket, _) = connect_async(format!("ws://{addr}"))
        .await
        .expect("client connects");
    events.emit(BridgeEventKind::WorkerProcessing { instance_id: 7 });

    websocket
        .send(Message::Text(hello_request(1).into()))
        .await
        .expect("hello sends");
    let hello = websocket
        .next()
        .await
        .expect("hello response")
        .expect("valid websocket message");
    let hello_value: serde_json::Value =
        serde_json::from_str(hello.to_text().expect("hello text")).expect("hello json");
    assert_eq!(hello_value["id"], 1);

    let maybe_event = tokio::time::timeout(Duration::from_millis(50), websocket.next()).await;
    assert!(maybe_event.is_err());

    let _ = shutdown_sender.send(());
}

#[cfg(unix)]
#[tokio::test]
async fn routes_binary_audio_frame_to_worker_passthrough() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = passthrough_worker_script();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let state = BridgeState {
        config: Arc::new(config),
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
        plugins: Arc::new(plugins),
        stream_tracker: Arc::new(AudioStreamTracker::new()),
        audio_in_flight: Arc::new(AudioInFlightLimiter::new()),
        workers: Arc::new(WorkerSupervisor::new_for_test_with_audio(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };

    let echoed = process_binary_payload(vec![1, 2, 3], &state).await;
    assert_eq!(echoed, vec![1, 2, 3]);

    let create_request = serde_json::json!({
        "id": 1,
        "method": "instance.create",
        "params": {
            "pluginId": plugin_id,
            "classId": "class-a",
            "sampleRate": 48000,
            "maxBlockFrames": 128,
            "inputChannels": 2,
            "outputChannels": 2
        }
    })
    .to_string();
    let create = handle_control_text(
        &create_request,
        ControlContext {
            config: &state.config,
            host_worker: &state.host_worker,
            instances: &state.instances,
            component_handler_events: &state.component_handler_events,
            events: &state.events,
            metrics: &state.metrics,
            plugins: &state.plugins,
            stream_tracker: &state.stream_tracker,
            origin: None,
            session_authorized: true,
            workers: &state.workers,
        },
    )
    .await;
    let create_value: serde_json::Value =
        serde_json::from_str(&create.text).expect("valid create json");
    let stream_id = create_value["result"]["streamId"]
        .as_u64()
        .expect("stream id");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    let start_request = serde_json::json!({
        "id": 2,
        "method": "instance.start",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let start = handle_control_text(
        &start_request,
        ControlContext {
            config: &state.config,
            host_worker: &state.host_worker,
            instances: &state.instances,
            component_handler_events: &state.component_handler_events,
            events: &state.events,
            metrics: &state.metrics,
            plugins: &state.plugins,
            stream_tracker: &state.stream_tracker,
            origin: None,
            session_authorized: true,
            workers: &state.workers,
        },
    )
    .await;
    let start_value: serde_json::Value =
        serde_json::from_str(&start.text).expect("valid start json");
    assert_eq!(start_value["result"]["instance"]["state"], "processing");

    let processed = process_binary_payload(audio_frame(stream_id), &state).await;
    let header = AudioFrameHeader::decode(&processed).expect("processed header");
    assert_eq!(header.stream_id.get(), stream_id);
    assert_eq!(
        read_f32_payload(&processed[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.25, 0.5, -0.25, -0.5]
    );
    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.binary_frames, 2);
    assert_eq!(metrics.audio_frame_fallbacks, 1);
    assert_eq!(metrics.audio_frames_routed, 1);
    assert_eq!(metrics.audio_frame_route_failures, 0);
    assert_eq!(metrics.audio_route_latency.count, 2);
    assert!(metrics.audio_route_latency.p50_us.is_some());

    let _ = process_binary_payload(audio_frame_with_sequence(stream_id, 12), &state).await;
    let _ = process_binary_payload(
        audio_frame_with_sequence_and_flags(stream_id, 12, wvst_protocol::AudioFrameFlags::LATE),
        &state,
    )
    .await;
    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.binary_frames, 4);
    assert_eq!(metrics.audio_frames_routed, 3);
    assert_eq!(metrics.audio_sequence_gap_events, 1);
    assert_eq!(metrics.audio_sequence_gap_frames, 1);
    assert_eq!(metrics.audio_frames_duplicate, 1);
    assert_eq!(metrics.audio_frames_late, 1);
    assert_eq!(metrics.audio_backpressure_drops, 0);
    assert_eq!(metrics.audio_interarrival_jitter.count, 2);

    state
        .instances
        .close_stream(StreamLifecycleParams { instance_id })
        .expect("stream closed");
    let closed_stream_response = process_binary_payload(audio_frame(stream_id), &state).await;
    let closed_stream_header =
        AudioFrameHeader::decode(&closed_stream_response).expect("closed stream header");
    assert!(
        closed_stream_header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::SILENCE)
    );
    assert!(
        closed_stream_header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::END_OF_STREAM)
    );
    assert_eq!(
        read_f32_payload(&closed_stream_response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.0, 0.0, 0.0, 0.0]
    );
    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.binary_frames, 5);
    assert_eq!(metrics.audio_frame_fallbacks, 1);
    assert_eq!(metrics.audio_frames_routed, 3);
    assert_eq!(metrics.audio_frame_route_failures, 1);
    assert_eq!(metrics.audio_route_latency.count, 5);
    assert!(metrics.audio_route_latency.p95_us.is_some());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn drops_overlapping_audio_frame_for_same_stream() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = passthrough_worker_script();
    let (plugins, root, plugin_id) = scanned_plugin_registry();
    let state = BridgeState {
        config: Arc::new(config),
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
        plugins: Arc::new(plugins),
        stream_tracker: Arc::new(AudioStreamTracker::new()),
        audio_in_flight: Arc::new(AudioInFlightLimiter::new()),
        workers: Arc::new(WorkerSupervisor::new_for_test_with_audio(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };
    let (instance_id, stream_id) = create_started_instance(&state, &plugin_id).await;
    let guard = state
        .audio_in_flight
        .try_acquire(stream_id)
        .expect("manual in-flight guard");

    let response = process_binary_payload(audio_frame(stream_id), &state).await;
    let header = AudioFrameHeader::decode(&response).expect("backpressure header");

    assert!(
        header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::SILENCE)
    );
    assert!(header.flags.contains(wvst_protocol::AudioFrameFlags::LATE));
    assert_eq!(
        read_f32_payload(&response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.0, 0.0, 0.0, 0.0]
    );
    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.audio_backpressure_drops, 1);
    assert_eq!(metrics.audio_frame_route_failures, 1);
    drop(guard);

    let processed = process_binary_payload(audio_frame_with_sequence(stream_id, 11), &state).await;
    assert_eq!(
        read_f32_payload(&processed[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.25, 0.5, -0.25, -0.5]
    );

    let _ = state
        .instances
        .close_stream(StreamLifecycleParams { instance_id });
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

async fn create_started_instance(state: &BridgeState, plugin_id: &str) -> (u64, u64) {
    let create_request = serde_json::json!({
        "id": 1,
        "method": "instance.create",
        "params": {
            "pluginId": plugin_id,
            "classId": "class-a",
            "sampleRate": 48000,
            "maxBlockFrames": 128,
            "inputChannels": 2,
            "outputChannels": 2
        }
    })
    .to_string();
    let create = handle_control_text(
        &create_request,
        ControlContext {
            config: &state.config,
            host_worker: &state.host_worker,
            instances: &state.instances,
            component_handler_events: &state.component_handler_events,
            events: &state.events,
            metrics: &state.metrics,
            plugins: &state.plugins,
            stream_tracker: &state.stream_tracker,
            origin: None,
            session_authorized: true,
            workers: &state.workers,
        },
    )
    .await;
    let create_value: serde_json::Value =
        serde_json::from_str(&create.text).expect("valid create json");
    let stream_id = create_value["result"]["streamId"]
        .as_u64()
        .expect("stream id");
    let instance_id = create_value["result"]["instanceId"]
        .as_u64()
        .expect("instance id");

    let start_request = serde_json::json!({
        "id": 2,
        "method": "instance.start",
        "params": { "instanceId": instance_id }
    })
    .to_string();
    let start = handle_control_text(
        &start_request,
        ControlContext {
            config: &state.config,
            host_worker: &state.host_worker,
            instances: &state.instances,
            component_handler_events: &state.component_handler_events,
            events: &state.events,
            metrics: &state.metrics,
            plugins: &state.plugins,
            stream_tracker: &state.stream_tracker,
            origin: None,
            session_authorized: true,
            workers: &state.workers,
        },
    )
    .await;
    let start_value: serde_json::Value =
        serde_json::from_str(&start.text).expect("valid start json");
    assert_eq!(start_value["result"]["instance"]["state"], "processing");

    (instance_id, stream_id)
}

fn read_f32_payload(payload: &[u8]) -> Option<Vec<f32>> {
    if payload.len() % 4 != 0 {
        return None;
    }

    Some(
        payload
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
    )
}

fn audio_frame(stream_id: u64) -> Vec<u8> {
    audio_frame_with_sequence(stream_id, 10)
}

fn hello_request(id: u64) -> String {
    format!(
        r#"{{"id":{id},"method":"bridge.hello","params":{{"clientName":"test","clientVersion":"0.1.0","protocolMin":{{"major":1,"minor":0}},"protocolMax":{{"major":1,"minor":0}},"audioFrameVersion":1}}}}"#
    )
}

fn audio_frame_with_sequence(stream_id: u64, sequence: u64) -> Vec<u8> {
    audio_frame_with_sequence_and_flags(
        stream_id,
        sequence,
        wvst_protocol::AudioFrameFlags::empty(),
    )
}

fn audio_frame_with_sequence_and_flags(
    stream_id: u64,
    sequence: u64,
    flags: wvst_protocol::AudioFrameFlags,
) -> Vec<u8> {
    let samples = [0.25_f32, 0.5, -0.25, -0.5];
    let header = AudioFrameHeader::new_f32(
        StreamId::new(stream_id),
        sequence,
        512,
        SampleRate::new(48_000).expect("sample rate"),
        FrameCount::new(2).expect("frames"),
        ChannelCount::new(2).expect("channels"),
        flags,
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

fn scanned_plugin_registry() -> (PluginRegistry, PathBuf, String) {
    let root = unique_temp_dir();
    let bundle = root.join("Test.vst3").join("Contents");
    std::fs::create_dir_all(&bundle).expect("bundle directory");
    std::fs::write(
        bundle.join("moduleinfo.json"),
        r#"{"Name":"Test","Classes":[{"CID":"class-a","Name":"Test Class","Category":"Fx"}]}"#,
    )
    .expect("moduleinfo");

    let plugins = PluginRegistry::new();
    let report = plugins.scan_paths(vec![root.clone()]);
    let plugin_id = report.plugins[0].plugin_id.clone();

    (plugins, root, plugin_id)
}

#[cfg(unix)]
fn passthrough_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("passthrough-worker.sh");
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
    response = struct.pack("<IHHHHIQ", magic, version, header_len, 2, 0, len(body), sequence) + body
    sock.sendall(response)
PY
fi
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true,"binaryAudioProcess":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready","backend":"passthrough","latencySamples":0,"tailSamples":0}}\n' "$id" ;;
    *instance.startProcessing*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"processing"}}\n' "$id" ;;
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

fn unique_temp_dir() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("wvst-server-test-{suffix}"))
}
