use std::path::PathBuf;

use serde_json::{Value, json};
use wvst_shm_mmap::SharedAudioMmap;
use wvst_shm_transport::SharedAudioTransportConfig;

use super::super::{WorkerIpcState, handle_ipc_line, insert_test_audio_instance};
use super::{SHARED_MEMORY_ERROR_INVALID, SHARED_MEMORY_ERROR_UNAVAILABLE};

#[test]
fn attaches_processes_and_detaches_shared_memory_audio() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    let (path, mut mmap) = create_mmap("roundtrip", 48_000, 128, 2, 2);
    let input = [0.1_f32, 0.2, 0.3, 0.4];
    let input_ring = mmap.layout().input;
    input_ring
        .write_interleaved_f32(mmap.memory_mut(), &input, 2)
        .expect("input write");

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    let attach_value: Value = serde_json::from_str(&attach).expect("attach json");
    assert_eq!(attach_value["result"]["attached"], true);
    assert_eq!(
        attach_value["result"]["sharedMemory"]["transport"],
        "file-backed-mmap"
    );

    start_processing(&mut state);
    let process = handle_ipc_line(
        r#"{"id":4,"method":"stream.sharedMemory.process","params":{"instanceId":7,"frames":2}}"#,
        &mut state,
    );
    let process_value: Value = serde_json::from_str(&process).expect("process json");
    assert_eq!(process_value["result"]["instanceId"], 7);
    assert_eq!(process_value["result"]["streamId"], 9);
    assert_eq!(process_value["result"]["transport"], "file-backed-mmap");
    assert_eq!(process_value["result"]["frames"], 2);
    assert_eq!(process_value["result"]["input"]["frames"], 2);
    assert_eq!(process_value["result"]["output"]["frames"], 2);
    assert_eq!(process_value["result"]["inputEvents"]["midiEvents"], 0);
    assert_eq!(process_value["result"]["inputEvents"]["parameterEvents"], 0);

    let output_ring = mmap.layout().output;
    let mut output = [0.0; 4];
    output_ring
        .read_interleaved_f32(mmap.memory_mut(), &mut output, 2)
        .expect("output read");
    assert_eq!(output, input);

    let detach = handle_ipc_line(
        r#"{"id":5,"method":"stream.sharedMemory.detach","params":{"instanceId":7}}"#,
        &mut state,
    );
    let detach_value: Value = serde_json::from_str(&detach).expect("detach json");
    assert_eq!(detach_value["result"]["detached"], true);

    let _ = std::fs::remove_file(path);
}

#[test]
fn processes_shared_memory_audio_with_midi_and_parameter_events() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    let (path, mut mmap) = create_mmap("events", 48_000, 128, 2, 2);
    mmap.layout()
        .input
        .write_interleaved_f32(mmap.memory_mut(), &[0.1, 0.2, 0.3, 0.4], 2)
        .expect("input write");

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    assert!(attach.contains(r#""result""#), "{attach}");

    start_processing(&mut state);
    let process = handle_ipc_line(
        &json!({
            "id": 4,
            "method": "stream.sharedMemory.process",
            "params": {
                "instanceId": 7,
                "frames": 2,
                "midiEvents": [{
                    "sampleOffset": 1,
                    "kind": 1,
                    "channel": 0,
                    "data1": 60,
                    "data2": 100,
                    "noteId": 123
                }],
                "parameterEvents": [{
                    "sampleOffset": 1,
                    "parameterId": 42,
                    "valueNormalized": 0.5
                }]
            }
        })
        .to_string(),
        &mut state,
    );
    let process_value: Value = serde_json::from_str(&process).expect("process json");

    assert_eq!(process_value["result"]["inputEvents"]["midiEvents"], 1);
    assert_eq!(process_value["result"]["inputEvents"]["parameterEvents"], 1);
    assert_eq!(process_value["result"]["inputEvents"]["vst3InputEvents"], 1);
    assert_eq!(
        process_value["result"]["inputEvents"]["vst3ParameterChanges"],
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_shared_memory_events_outside_processed_block() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    let (path, mut mmap) = create_mmap("event-offset", 48_000, 128, 2, 2);
    mmap.layout()
        .input
        .write_interleaved_f32(mmap.memory_mut(), &[0.1, 0.2, 0.3, 0.4], 2)
        .expect("input write");

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    assert!(attach.contains(r#""result""#), "{attach}");

    start_processing(&mut state);
    let process = handle_ipc_line(
        &json!({
            "id": 4,
            "method": "stream.sharedMemory.process",
            "params": {
                "instanceId": 7,
                "frames": 2,
                "midiEvents": [{
                    "sampleOffset": 2,
                    "kind": 1,
                    "channel": 0,
                    "data1": 60,
                    "data2": 100
                }]
            }
        })
        .to_string(),
        &mut state,
    );
    let process_value: Value = serde_json::from_str(&process).expect("process json");

    assert_eq!(process_value["error"]["code"], SHARED_MEMORY_ERROR_INVALID);
    assert_eq!(process_value["error"]["data"]["reason"], "midi-events");
    assert_eq!(
        process_value["error"]["data"]["workerData"]["reason"],
        "midi-events"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_shared_memory_layout_mismatch() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    let (path, _mmap) = create_mmap("mismatch", 44_100, 128, 2, 2);

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    let attach_value: Value = serde_json::from_str(&attach).expect("attach json");

    assert_eq!(attach_value["error"]["code"], SHARED_MEMORY_ERROR_INVALID);
    assert!(
        attach_value["error"]["message"]
            .as_str()
            .expect("message")
            .contains("sample rate mismatch")
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn reports_input_underrun_reason_when_input_ring_is_empty() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    let (path, _mmap) = create_mmap("input-underrun", 48_000, 128, 2, 2);

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    assert!(attach.contains(r#""result""#), "{attach}");

    start_processing(&mut state);
    let process = handle_ipc_line(
        r#"{"id":4,"method":"stream.sharedMemory.process","params":{"instanceId":7,"frames":2}}"#,
        &mut state,
    );
    let process_value: Value = serde_json::from_str(&process).expect("process json");

    assert_eq!(
        process_value["error"]["code"],
        SHARED_MEMORY_ERROR_UNAVAILABLE
    );
    assert_eq!(process_value["error"]["data"]["reason"], "input-underrun");
    assert_eq!(
        process_value["error"]["data"]["workerData"]["reason"],
        "input-underrun"
    );
    assert_eq!(
        process_value["error"]["data"]["workerData"]["requestedFrames"],
        2
    );
    assert_eq!(
        process_value["error"]["data"]["workerData"]["availableFrames"],
        0
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn processes_zero_input_shared_memory_without_input_ring_frames() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 0, 2);
    let (path, mut mmap) = create_mmap("zero-input", 48_000, 128, 0, 2);

    let attach = handle_ipc_line(
        &json!({
            "id": 2,
            "method": "stream.sharedMemory.attach",
            "params": { "instanceId": 7, "path": path.display().to_string() }
        })
        .to_string(),
        &mut state,
    );
    assert!(attach.contains(r#""result""#), "{attach}");

    start_processing(&mut state);
    let process = handle_ipc_line(
        r#"{"id":4,"method":"stream.sharedMemory.process","params":{"instanceId":7,"frames":2}}"#,
        &mut state,
    );
    let process_value: Value = serde_json::from_str(&process).expect("process json");

    assert_eq!(process_value["result"]["instanceId"], 7);
    assert_eq!(process_value["result"]["frames"], 2);
    assert_eq!(process_value["result"]["input"]["frames"], 2);
    assert_eq!(process_value["result"]["input"]["samples"], 0);
    assert_eq!(process_value["result"]["output"]["frames"], 2);

    let output_ring = mmap.layout().output;
    let mut output = [1.0_f32; 4];
    output_ring
        .read_interleaved_f32(mmap.memory_mut(), &mut output, 2)
        .expect("output read");
    assert_eq!(output, [0.0, 0.0, 0.0, 0.0]);

    let _ = std::fs::remove_file(path);
}

fn create_instance(state: &mut WorkerIpcState, input_channels: usize, output_channels: usize) {
    insert_test_audio_instance(state, input_channels, output_channels);
}

fn start_processing(state: &mut WorkerIpcState) {
    let response = handle_ipc_line(
        r#"{"id":3,"method":"instance.startProcessing","params":{"instanceId":7}}"#,
        state,
    );

    assert!(response.contains(r#""result""#), "{response}");
}

fn create_mmap(
    label: &str,
    sample_rate: u32,
    max_block_frames: u16,
    input_channels: u16,
    output_channels: u16,
) -> (PathBuf, SharedAudioMmap) {
    let path = unique_path(label);
    let layout = SharedAudioTransportConfig::new(
        sample_rate,
        max_block_frames,
        2,
        input_channels,
        output_channels,
    )
    .layout()
    .expect("layout");
    let mmap = SharedAudioMmap::create(&path, &layout).expect("mmap");
    (path, mmap)
}

fn unique_path(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "wvst-host-worker-shm-{label}-{}-{nanos}",
        std::process::id()
    ))
}
