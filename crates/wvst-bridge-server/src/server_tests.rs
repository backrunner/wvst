use super::*;
use futures_util::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_tungstenite::connect_async;
use wvst_core::{FrameCount, SampleRate};

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
        metrics: Arc::new(BridgeMetrics::new()),
        plugins: Arc::new(plugins),
        workers: Arc::new(WorkerSupervisor::new_for_test(
            worker_path.clone(),
            Duration::from_secs(5),
        )),
    };

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
            metrics: &state.metrics,
            plugins: &state.plugins,
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

    let processed = process_binary_payload(audio_frame(stream_id), &state).await;
    let header = AudioFrameHeader::decode(&processed).expect("processed header");
    assert_eq!(header.stream_id.get(), stream_id);
    assert_eq!(
        read_f32_payload(&processed[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.25, 0.5, -0.25, -0.5]
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(worker_path.parent().expect("worker parent"));
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
        wvst_protocol::AudioFrameFlags::empty(),
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
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *worker.hello*) printf '{"jsonrpc":"2.0","id":%s,"result":{"workerName":"test-worker","ipcVersion":1,"capabilities":{"instanceLifecycle":true}}}\n' "$id" ;;
    *instance.create*) printf '{"jsonrpc":"2.0","id":%s,"result":{"instanceId":1,"streamId":1,"workerState":"ready"}}\n' "$id" ;;
    *debug.processInterleavedF32*) printf '{"jsonrpc":"2.0","id":%s,"result":{"streamId":1,"inputChannels":2,"outputChannels":2,"stats":{"frames":2,"inputChannels":2,"outputChannels":2},"output":[0.25,0.5,-0.25,-0.5]}}\n' "$id" ;;
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
