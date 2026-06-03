use super::ipc_framed::{control_response_status_code, serve_framed};
use super::*;
use wvst_protocol::{
    WORKER_CONTROL_IPC_HEADER_LEN, WorkerControlIpcBatch, WorkerControlIpcBatchRole,
    WorkerControlIpcHeader, WorkerControlIpcMessage, WorkerControlMessageKind,
};

#[test]
fn responds_to_worker_hello() {
    let mut state = WorkerIpcState::default();
    let response = handle_ipc_line(r#"{"id":1,"method":"worker.hello"}"#, &mut state);
    let value: Value = serde_json::from_str(&response).expect("json");

    assert_eq!(value["result"]["workerName"], "wvst-host-worker");
    assert_eq!(value["result"]["ipcVersion"], WORKER_IPC_VERSION);
    assert_eq!(value["result"]["capabilities"]["instanceLifecycle"], true);
    assert_eq!(value["result"]["capabilities"]["vst3Parameters"], true);
    assert_eq!(value["result"]["capabilities"]["vst3UnitProgramData"], true);
    assert_eq!(value["result"]["capabilities"]["vst3ProgramListData"], true);
    assert_eq!(value["result"]["capabilities"]["vst3UnitData"], true);
    assert_eq!(value["result"]["capabilities"]["vst3ControllerState"], true);
    assert_eq!(value["result"]["capabilities"]["vst3ControlErrors"], true);
    assert_eq!(
        value["result"]["capabilities"]["framedControlMaxBodyBytes"],
        WORKER_CONTROL_IPC_MAX_BODY_LEN
    );
    assert_eq!(
        value["result"]["capabilities"]["framedControlSequenceIds"],
        true
    );
    assert_eq!(
        value["result"]["capabilities"]["framedControlStatusCodes"],
        true
    );
    assert_eq!(
        value["result"]["capabilities"]["framedControlErrorResponses"],
        true
    );
    assert_eq!(
        value["result"]["capabilities"]["framedControlBatching"],
        true
    );
    assert_eq!(
        value["result"]["capabilities"]["maxVst3StateBytes"],
        DEFAULT_MAX_VST3_STATE_BYTES
    );
}

#[test]
fn rejects_instance_create_when_plugin_path_is_not_bundle_directory() {
    let mut state = WorkerIpcState::default();
    let create = handle_ipc_line(
        r#"{"id":1,"method":"instance.create","params":{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","classId":"class-a","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":2,"outputChannels":2}}"#,
        &mut state,
    );
    let create_value: Value = serde_json::from_str(&create).expect("create json");
    assert_eq!(create_value["error"]["code"], 4220);
    assert!(
        create_value["error"]["message"]
            .as_str()
            .expect("message")
            .contains("pluginPath must point to a VST3 bundle directory")
    );
    assert_eq!(state.instances.len(), 0);

    let metrics = handle_ipc_line(r#"{"id":8,"method":"worker.metrics"}"#, &mut state);
    let metrics_value: Value = serde_json::from_str(&metrics).expect("metrics json");
    assert_eq!(metrics_value["result"]["instances"], 0);
    assert_eq!(metrics_value["result"]["vst3RuntimeInstances"], 0);
}

#[test]
fn rejects_instance_create_when_class_id_is_missing() {
    let mut state = WorkerIpcState::default();
    let create = handle_ipc_line(
        r#"{"id":1,"method":"instance.create","params":{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":2,"outputChannels":2}}"#,
        &mut state,
    );
    let create_value: Value = serde_json::from_str(&create).expect("create json");
    assert_eq!(create_value["error"]["code"], 4220);
    assert_eq!(
        create_value["error"]["message"],
        "classId is required for VST3 runtime instance create"
    );
    assert_eq!(state.instances.len(), 0);
}

#[test]
fn serializes_structured_error_data() {
    let response = response_error_data(
        json!(1),
        4220,
        "controller init failed",
        json!({
            "kind": "vst3-runtime-init",
            "stage": "controller.initialize",
        }),
    );
    let value: Value = serde_json::from_str(&response).expect("error json");

    assert_eq!(value["error"]["code"], 4220);
    assert_eq!(value["error"]["data"]["kind"], "vst3-runtime-init");
    assert_eq!(value["error"]["data"]["stage"], "controller.initialize");
}

#[test]
fn classifies_runtime_component_create_failure() {
    let bundle_path = unique_temp_dir();
    std::fs::create_dir_all(&bundle_path).expect("bundle dir");

    let request = json!({
        "id": 19,
        "method": "instance.create",
        "params": {
            "instanceId": 19,
            "streamId": 20,
            "pluginId": "vst3:broken",
            "pluginPath": bundle_path,
            "classId": "{e831ff31-f2d5-4301-928e-bbee25697802}",
            "className": "Broken",
            "sampleRate": 48_000,
            "maxBlockFrames": 128,
            "inputChannels": 2,
            "outputChannels": 2
        }
    });
    let mut state = WorkerIpcState::default();
    let response = handle_ipc_line(&request.to_string(), &mut state);
    let value: Value = serde_json::from_str(&response).expect("create error json");

    assert_eq!(value["error"]["code"], 4220);
    assert_eq!(value["error"]["data"]["kind"], "vst3-runtime-init");
    assert_eq!(value["error"]["data"]["stage"], "component.create");
    assert!(value["error"]["data"]["hostError"].is_string());
    assert_eq!(value["error"]["data"]["compatibility"]["schemaVersion"], 1);
    assert!(value["error"]["data"]["compatibility"]["category"].is_string());
    assert_eq!(state.instances.len(), 0);

    let _ = std::fs::remove_dir_all(bundle_path);
}

#[test]
fn serve_framed_round_trips_control_frames() {
    let request =
        WorkerControlIpcMessage::request(42, br#"{"id":1,"method":"worker.hello"}"#.to_vec())
            .expect("request")
            .encode()
            .expect("encoded request");
    let mut output = Vec::new();

    serve_framed(&request[..], &mut output).expect("served");

    let header = WorkerControlIpcHeader::decode(&output).expect("header");
    assert_eq!(header.sequence, 42);
    assert_eq!(
        header.body_len as usize,
        output.len() - WORKER_CONTROL_IPC_HEADER_LEN
    );
    let response: Value =
        serde_json::from_slice(&output[WORKER_CONTROL_IPC_HEADER_LEN..]).expect("response json");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["capabilities"]["framedControlIpc"], true);
    assert_eq!(
        response["result"]["capabilities"]["framedControlIpcVersion"],
        WORKER_CONTROL_IPC_SCHEMA_VERSION
    );
    assert_eq!(
        response["result"]["capabilities"]["framedControlMaxBodyBytes"],
        WORKER_CONTROL_IPC_MAX_BODY_LEN
    );
    assert_eq!(
        response["result"]["capabilities"]["framedControlSequenceIds"],
        true
    );
    assert_eq!(
        response["result"]["capabilities"]["framedControlStatusCodes"],
        true
    );
    assert_eq!(
        response["result"]["capabilities"]["framedControlErrorResponses"],
        true
    );
    assert_eq!(
        response["result"]["capabilities"]["framedControlBatching"],
        true
    );
}

#[test]
fn serve_framed_round_trips_control_batch_frames() {
    let first =
        WorkerControlIpcMessage::request(50, br#"{"id":1,"method":"worker.hello"}"#.to_vec())
            .expect("first request");
    let second =
        WorkerControlIpcMessage::request(51, br#"{"id":2,"method":"missing.method"}"#.to_vec())
            .expect("second request");
    let request = WorkerControlIpcBatch::request_frame(500, vec![first, second])
        .expect("batch request")
        .encode()
        .expect("encoded batch request");
    let mut output = Vec::new();

    serve_framed(&request[..], &mut output).expect("served");

    let header = WorkerControlIpcHeader::decode(&output).expect("batch header");
    assert_eq!(header.kind, WorkerControlMessageKind::BatchResponse);
    assert_eq!(header.sequence, 500);
    let batch = WorkerControlIpcBatch::decode_body(
        WorkerControlIpcBatchRole::Response,
        &output[WORKER_CONTROL_IPC_HEADER_LEN..],
    )
    .expect("response batch");
    let messages = batch.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].header.kind, WorkerControlMessageKind::Response);
    assert_eq!(messages[0].header.sequence, 50);
    assert_eq!(
        messages[1].header.kind,
        WorkerControlMessageKind::ErrorResponse
    );
    assert_eq!(messages[1].header.sequence, 51);
    assert_eq!(messages[1].header.status_code, 1);

    let first_response: Value = serde_json::from_slice(&messages[0].body).expect("first response");
    let second_response: Value =
        serde_json::from_slice(&messages[1].body).expect("second response");
    assert_eq!(first_response["id"], 1);
    assert_eq!(
        first_response["result"]["capabilities"]["framedControlBatching"],
        true
    );
    assert_eq!(second_response["id"], 2);
    assert_eq!(second_response["error"]["code"], -32601);
}

#[test]
fn serve_framed_classifies_json_rpc_errors() {
    let request =
        WorkerControlIpcMessage::request(43, br#"{"id":2,"method":"missing.method"}"#.to_vec())
            .expect("request")
            .encode()
            .expect("encoded request");
    let mut output = Vec::new();

    serve_framed(&request[..], &mut output).expect("served");

    let header = WorkerControlIpcHeader::decode(&output).expect("header");
    assert_eq!(header.kind, WorkerControlMessageKind::ErrorResponse);
    assert_eq!(header.sequence, 43);
    assert_eq!(header.status_code, 1);
    let response: Value =
        serde_json::from_slice(&output[WORKER_CONTROL_IPC_HEADER_LEN..]).expect("response json");
    assert_eq!(response["id"], 2);
    assert_eq!(response["error"]["code"], -32601);
}

#[test]
fn serve_framed_rejects_non_request_frames() {
    let request =
        WorkerControlIpcMessage::response(44, br#"{"id":3,"method":"worker.hello"}"#.to_vec())
            .expect("response frame")
            .encode()
            .expect("encoded response frame");
    let mut output = Vec::new();

    let error = serve_framed(&request[..], &mut output).expect_err("invalid request frame");

    assert!(error.contains("invalid control request frame kind"));
    assert!(output.is_empty());
}

#[test]
fn serve_framed_rejects_oversized_request_bodies() {
    let header = WorkerControlIpcHeader::new(
        WorkerControlMessageKind::Request,
        0,
        45,
        WORKER_CONTROL_IPC_MAX_BODY_LEN + 1,
    );
    let mut request = [0; WORKER_CONTROL_IPC_HEADER_LEN];
    header.encode(&mut request).expect("header");
    let mut output = Vec::new();

    let error = serve_framed(&request[..], &mut output).expect_err("oversized request");

    assert!(error.contains("control request body too large"));
    assert!(output.is_empty());
}

#[test]
fn positive_json_rpc_errors_keep_status_code() {
    assert_eq!(
        control_response_status_code(
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":4220,"message":"bad"}}"#
        ),
        Some(4220)
    );
}

fn unique_temp_dir() -> std::path::PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("wvst-host-worker-ipc-test-{suffix}"))
}
