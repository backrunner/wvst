use std::path::PathBuf;

use serde_json::{Value, json};
use wvst_shm_mmap::SharedAudioMmap;
use wvst_shm_transport::SharedAudioTransportConfig;

use super::super::{WorkerIpcState, handle_ipc_line};
use super::SHARED_MEMORY_ERROR_INVALID;

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
    assert_eq!(process_value["result"]["frames"], 2);
    assert_eq!(process_value["result"]["input"]["frames"], 2);
    assert_eq!(process_value["result"]["output"]["frames"], 2);

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

fn create_instance(state: &mut WorkerIpcState, input_channels: usize, output_channels: usize) {
    let request = format!(
        r#"{{"id":1,"method":"instance.create","params":{{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","classId":"class-a","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":{input_channels},"outputChannels":{output_channels}}}}}"#,
    );
    let response = handle_ipc_line(&request, state);

    assert!(response.contains(r#""result""#), "{response}");
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
