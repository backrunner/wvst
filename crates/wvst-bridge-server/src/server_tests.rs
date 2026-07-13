#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use super::*;
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;
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
async fn responds_to_hello_and_rejects_invalid_binary_frames() {
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

    let response = websocket
        .next()
        .await
        .expect("binary response")
        .expect("valid websocket message");
    assert!(response.into_data().is_empty());

    let _ = shutdown_sender.send(());
}

#[tokio::test]
async fn ignores_binary_audio_before_session_authorization() {
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
        .send(Message::Binary(vec![1, 2, 3].into()))
        .await
        .expect("binary sends");
    assert!(
        timeout(Duration::from_millis(50), websocket.next())
            .await
            .is_err()
    );

    websocket
        .send(Message::Text(
            r#"{"id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#
                .into(),
        ))
        .await
        .expect("hello sends");
    let response = websocket
        .next()
        .await
        .expect("hello response")
        .expect("valid websocket message");
    assert!(response.to_text().expect("text").contains("wvst-bridge"));

    let _ = shutdown_sender.send(());
}

#[tokio::test]
async fn returns_diagnostic_silence_for_unmatched_wvst_audio_stream() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
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
        plugins: Arc::new(PluginRegistry::new()),
        stream_tracker: Arc::new(AudioStreamTracker::new()),
        audio_in_flight: Arc::new(AudioInFlightLimiter::new()),
        shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
        shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
        workers: Arc::new(WorkerSupervisor::new_for_test(
            PathBuf::from("missing-worker"),
            Duration::from_secs(5),
        )),
    };

    let response = process_binary_payload(audio_frame(4242), &state).await;
    let header = AudioFrameHeader::decode(&response).expect("diagnostic header");

    assert_eq!(header.stream_id.get(), 4242);
    assert_eq!(header.sequence, 10);
    assert!(
        header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::SILENCE)
    );
    assert!(
        header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::PROCESS_ERROR)
    );
    assert_eq!(
        read_f32_payload(&response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.0, 0.0, 0.0, 0.0]
    );

    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.binary_frames, 1);
    assert_eq!(metrics.audio_frame_route_failures, 1);
    assert_eq!(metrics.audio_frame_invalid_headers, 0);
    assert_eq!(metrics.audio_frame_invalid_lengths, 0);
    assert_eq!(metrics.audio_frame_unmatched_streams, 1);
    assert_eq!(metrics.audio_frames_routed, 0);
    assert_eq!(metrics.audio_route_latency.count, 1);

    let mut truncated = audio_frame(4343);
    truncated.pop();
    let response = process_binary_payload(truncated, &state).await;
    let header = AudioFrameHeader::decode(&response).expect("diagnostic header");

    assert_eq!(header.stream_id.get(), 4343);
    assert!(
        header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::SILENCE)
    );
    assert!(
        header
            .flags
            .contains(wvst_protocol::AudioFrameFlags::PROCESS_ERROR)
    );
    assert_eq!(
        read_f32_payload(&response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.0, 0.0, 0.0, 0.0]
    );

    let metrics = state.metrics.snapshot();
    assert_eq!(metrics.binary_frames, 2);
    assert_eq!(metrics.audio_frame_route_failures, 2);
    assert_eq!(metrics.audio_frame_invalid_headers, 0);
    assert_eq!(metrics.audio_frame_invalid_lengths, 1);
    assert_eq!(metrics.audio_frame_unmatched_streams, 1);
    assert_eq!(metrics.audio_frames_routed, 0);
    assert_eq!(metrics.audio_route_latency.count, 2);
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
async fn routes_binary_audio_frame_to_worker_fixture() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = audio_echo_worker_script();
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
        shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
        shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
        workers: Arc::new(WorkerSupervisor::new_for_test_with_audio(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };

    let rejected = process_binary_payload(vec![1, 2, 3], &state).await;
    assert!(rejected.is_empty());

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
            audio_in_flight: &state.audio_in_flight,
            shared_memory: &state.shared_memory,
            shared_memory_pumps: &state.shared_memory_pumps,
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
            audio_in_flight: &state.audio_in_flight,
            shared_memory: &state.shared_memory,
            shared_memory_pumps: &state.shared_memory_pumps,
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
    assert_eq!(metrics.audio_frames_routed, 1);
    assert_eq!(metrics.audio_frame_route_failures, 1);
    assert_eq!(metrics.audio_frame_invalid_headers, 1);
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
    assert_eq!(metrics.audio_frames_routed, 3);
    assert_eq!(metrics.audio_frame_route_failures, 2);
    assert_eq!(metrics.audio_frame_invalid_headers, 1);
    assert_eq!(metrics.audio_route_latency.count, 5);
    assert!(metrics.audio_route_latency.p95_us.is_some());

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
}

#[cfg(unix)]
#[tokio::test]
async fn drops_overlapping_audio_frame_for_same_stream() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = audio_echo_worker_script();
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
        shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
        shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
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

#[cfg(unix)]
#[tokio::test]
async fn keeps_worker_running_after_invalid_audio_request() {
    let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
    let worker_path = audio_echo_worker_script();
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
        shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
        shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
        workers: Arc::new(WorkerSupervisor::new_for_test_with_audio(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };
    let (instance_id, stream_id) = create_started_instance(&state, &plugin_id).await;

    let rejected = process_binary_payload(audio_frame_at_rate(stream_id, 44_100), &state).await;
    let rejected_header = AudioFrameHeader::decode(&rejected).expect("diagnostic header");
    assert!(rejected_header.flags.contains(AudioFrameFlags::SILENCE));
    assert!(
        rejected_header
            .flags
            .contains(AudioFrameFlags::PROCESS_ERROR)
    );
    assert_eq!(
        state.instances.get(instance_id).expect("instance").state,
        crate::instance_registry::InstanceState::Processing
    );
    assert!(
        state
            .events
            .recent_since(None)
            .into_iter()
            .all(|event| { !matches!(event.kind, BridgeEventKind::WorkerFailed { .. }) })
    );

    let processed = process_binary_payload(audio_frame(stream_id), &state).await;
    assert_eq!(
        read_f32_payload(&processed[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.25, 0.5, -0.25, -0.5]
    );

    let mismatched = process_binary_payload(audio_frame_with_sequence(stream_id, 99), &state).await;
    let mismatched_header = AudioFrameHeader::decode(&mismatched).expect("diagnostic header");
    assert_eq!(mismatched_header.sequence, 99);
    assert!(mismatched_header.flags.contains(AudioFrameFlags::SILENCE));
    assert!(
        mismatched_header
            .flags
            .contains(AudioFrameFlags::PROCESS_ERROR)
    );
    assert_eq!(
        state.instances.get(instance_id).expect("instance").state,
        crate::instance_registry::InstanceState::Failed
    );
    assert!(
        state
            .events
            .recent_since(None)
            .into_iter()
            .any(|event| { matches!(event.kind, BridgeEventKind::WorkerFailed { .. }) })
    );

    let _ = state
        .instances
        .close_stream(StreamLifecycleParams { instance_id });
    let _ = state.workers.destroy_instance(instance_id).await;
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
            audio_in_flight: &state.audio_in_flight,
            shared_memory: &state.shared_memory,
            shared_memory_pumps: &state.shared_memory_pumps,
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
            audio_in_flight: &state.audio_in_flight,
            shared_memory: &state.shared_memory,
            shared_memory_pumps: &state.shared_memory_pumps,
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

fn audio_frame_at_rate(stream_id: u64, sample_rate: u32) -> Vec<u8> {
    let samples = [0.25_f32, 0.5, -0.25, -0.5];
    let header = AudioFrameHeader::new_f32(
        StreamId::new(stream_id),
        10,
        512,
        SampleRate::new(sample_rate).expect("sample rate"),
        FrameCount::new(2).expect("frames"),
        ChannelCount::new(2).expect("channels"),
        AudioFrameFlags::empty(),
    )
    .expect("header");
    let mut frame = vec![0; AUDIO_FRAME_HEADER_LEN + header.payload_len as usize];
    header
        .encode(&mut frame[..AUDIO_FRAME_HEADER_LEN])
        .expect("encode header");
    for (destination, sample) in frame[AUDIO_FRAME_HEADER_LEN..]
        .chunks_exact_mut(4)
        .zip(samples)
    {
        destination.copy_from_slice(&sample.to_le_bytes());
    }
    frame
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
fn audio_echo_worker_script() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique_temp_dir();
    std::fs::create_dir_all(&directory).expect("temp dir");
    let worker = directory.join("audio-echo-worker.py");
    std::fs::write(
        &worker,
        r#"#!/usr/bin/env python3
import json
import struct
import sys
import socket
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

def pack_frame(kind, status, sequence, body):
    return struct.pack("<IHHHHIQ", MAGIC, VERSION, HEADER_LEN, kind, status, len(body), sequence) + body

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
        sample_rate = struct.unpack_from("<I", body, 32)[0]
        if sample_rate != 48000:
            error = json.dumps({"message": "sample rate mismatch", "data": {"schemaVersion": 1, "kind": "audio-request-invalid"}}, separators=(",", ":")).encode()
            sock.sendall(struct.pack("<IHHHHIQ", magic, version, header_len, 3, 4220, len(error), sequence) + error)
            continue
        frame_sequence = struct.unpack_from("<Q", body, 16)[0]
        if frame_sequence == 99:
            body = bytearray(body)
            struct.pack_into("<Q", body, 16, 100)
        sock.sendall(struct.pack("<IHHHHIQ", magic, version, header_len, 2, 0, len(body), sequence) + body)

def ok(request, result):
    body = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}, separators=(",", ":")).encode()
    return pack_frame(KIND_RESPONSE, 0, request["_sequence"], body)

def err(request, code, message):
    body = json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "error": {"code": code, "message": message}}, separators=(",", ":")).encode()
    return pack_frame(KIND_ERROR, code & 0xFFFF, request["_sequence"], body)

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
    elif method == "instance.startProcessing":
        frame = ok(request, {"instanceId": 1, "streamId": 1, "workerState": "processing"})
    else:
        frame = err(request, -32601, "unknown")
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

fn unique_temp_dir() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("wvst-server-test-{suffix}"))
}
