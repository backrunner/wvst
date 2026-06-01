use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_scanner::{MetadataSource, PluginClass, PluginDescriptor, PluginFormat};
use wvst_vst3_host::HeadlessPluginInstance;

#[path = "ipc_audio.rs"]
mod ipc_audio;

const WORKER_IPC_VERSION: u16 = 1;

#[derive(Default)]
pub struct WorkerIpcState {
    instances: BTreeMap<u64, WorkerInstance>,
}

struct WorkerInstance {
    stream_id: u64,
    input_channels: usize,
    output_channels: usize,
    processing: bool,
    plugin: HeadlessPluginInstance,
}

#[derive(Debug, Deserialize)]
struct IpcRequest {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceCreateParams {
    instance_id: u64,
    stream_id: u64,
    plugin_id: String,
    plugin_path: String,
    #[serde(default)]
    class_id: Option<String>,
    #[serde(default)]
    class_name: Option<String>,
    input_channels: usize,
    output_channels: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDestroyParams {
    instance_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceProcessingParams {
    instance_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceReady {
    instance_id: u64,
    stream_id: u64,
    worker_state: WorkerState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerState {
    Ready,
    Processing,
    Stopped,
    Destroyed,
}

pub fn serve_stdio(audio_connect: Option<String>) -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    serve_with_audio(stdin.lock(), stdout.lock(), audio_connect)
}

#[cfg(test)]
pub fn serve(reader: impl BufRead, mut writer: impl Write) -> Result<(), String> {
    serve_with_audio(reader, &mut writer, None)
}

fn serve_with_audio(
    reader: impl BufRead,
    mut writer: impl Write,
    audio_connect: Option<String>,
) -> Result<(), String> {
    let state = Arc::new(Mutex::new(WorkerIpcState::default()));
    if let Some(address) = audio_connect {
        ipc_audio::spawn_audio_thread(address, Arc::clone(&state));
    }

    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let response = {
            let mut state = state
                .lock()
                .map_err(|_| "worker state mutex poisoned".to_string())?;
            handle_ipc_line(&line, &mut state)
        };
        writeln!(writer, "{response}").map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn handle_ipc_line(line: &str, state: &mut WorkerIpcState) -> String {
    let request = match serde_json::from_str::<IpcRequest>(line) {
        Ok(request) => request,
        Err(error) => {
            return response_error(Value::Null, -32700, format!("parse error: {error}"));
        }
    };

    match request.method.as_str() {
        "worker.hello" => response_result(request.id, worker_hello()),
        "worker.metrics" => response_result(request.id, worker_metrics(state)),
        "instance.create" => handle_instance_create(request.id, request.params, state),
        "instance.startProcessing" => {
            handle_instance_processing(request.id, request.params, state, true)
        }
        "instance.stopProcessing" => {
            handle_instance_processing(request.id, request.params, state, false)
        }
        "instance.destroy" => handle_instance_destroy(request.id, request.params, state),
        _ => response_error(
            request.id,
            -32601,
            format!("method not found: {}", request.method),
        ),
    }
}

fn handle_instance_create(id: Value, params: Value, state: &mut WorkerIpcState) -> String {
    let params = match serde_json::from_value::<InstanceCreateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance create params: {error}"),
            );
        }
    };

    if state.instances.contains_key(&params.instance_id) {
        return response_error(id, 4092, "instance already exists");
    }

    let descriptor = descriptor_from_params(&params);
    let plugin = match HeadlessPluginInstance::new(
        &descriptor,
        params.input_channels,
        params.output_channels,
    ) {
        Ok(plugin) => plugin,
        Err(error) => return response_error(id, 4220, error.to_string()),
    };

    let ready = InstanceReady {
        instance_id: params.instance_id,
        stream_id: params.stream_id,
        worker_state: WorkerState::Ready,
    };
    state.instances.insert(
        params.instance_id,
        WorkerInstance {
            stream_id: params.stream_id,
            input_channels: params.input_channels,
            output_channels: params.output_channels,
            processing: false,
            plugin,
        },
    );

    response_result(id, json!(ready))
}

fn handle_instance_destroy(id: Value, params: Value, state: &mut WorkerIpcState) -> String {
    let params = match serde_json::from_value::<InstanceDestroyParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance destroy params: {error}"),
            );
        }
    };

    let Some(instance) = state.instances.remove(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "streamId": instance.stream_id,
            "workerState": WorkerState::Destroyed,
        }),
    )
}

fn handle_instance_processing(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
    processing: bool,
) -> String {
    let params = match serde_json::from_value::<InstanceProcessingParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid instance processing params: {error}"),
            );
        }
    };

    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    instance.processing = processing;
    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "streamId": instance.stream_id,
            "workerState": if processing {
                WorkerState::Processing
            } else {
                WorkerState::Stopped
            },
        }),
    )
}

fn worker_hello() -> Value {
    json!({
        "workerName": "wvst-host-worker",
        "workerVersion": env!("CARGO_PKG_VERSION"),
        "ipcVersion": WORKER_IPC_VERSION,
        "capabilities": {
            "factoryInfo": true,
            "instanceLifecycle": true,
            "fakePassthrough": true,
            "binaryAudioProcess": true
        }
    })
}

fn worker_metrics(state: &WorkerIpcState) -> Value {
    json!({
        "ipcVersion": WORKER_IPC_VERSION,
        "instances": state.instances.len(),
        "processingInstances": state
            .instances
            .values()
            .filter(|instance| instance.processing)
            .count(),
    })
}

fn descriptor_from_params(params: &InstanceCreateParams) -> PluginDescriptor {
    let class_name = params
        .class_name
        .clone()
        .unwrap_or_else(|| "WVST Worker Passthrough".to_string());

    PluginDescriptor {
        plugin_id: params.plugin_id.clone(),
        format: PluginFormat::Vst3,
        name: class_name.clone(),
        vendor: Some("WVST".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        path: params.plugin_path.clone(),
        classes: vec![PluginClass {
            class_id: params.class_id.clone(),
            name: class_name,
            category: Some("Fx".to_string()),
            subcategories: vec!["Stereo".to_string()],
        }],
        metadata_source: MetadataSource::BundleName,
    }
}

fn response_result(id: Value, result: Value) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

fn response_error(id: Value, code: i64, message: impl Into<String>) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    }))
}

fn serialize_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"internal error\"}}"
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responds_to_worker_hello() {
        let mut state = WorkerIpcState::default();
        let response = handle_ipc_line(r#"{"id":1,"method":"worker.hello"}"#, &mut state);
        let value: Value = serde_json::from_str(&response).expect("json");

        assert_eq!(value["result"]["workerName"], "wvst-host-worker");
        assert_eq!(value["result"]["ipcVersion"], WORKER_IPC_VERSION);
        assert_eq!(value["result"]["capabilities"]["instanceLifecycle"], true);
    }

    #[test]
    fn creates_processes_and_destroys_passthrough_instance() {
        let mut state = WorkerIpcState::default();
        let create = handle_ipc_line(
            r#"{"id":1,"method":"instance.create","params":{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","classId":"class-a","className":"Test","inputChannels":2,"outputChannels":2}}"#,
            &mut state,
        );
        let create_value: Value = serde_json::from_str(&create).expect("create json");
        assert_eq!(create_value["result"]["workerState"], "ready");

        let start = handle_ipc_line(
            r#"{"id":2,"method":"instance.startProcessing","params":{"instanceId":7}}"#,
            &mut state,
        );
        let start_value: Value = serde_json::from_str(&start).expect("start json");
        assert_eq!(start_value["result"]["workerState"], "processing");

        let stop = handle_ipc_line(
            r#"{"id":3,"method":"instance.stopProcessing","params":{"instanceId":7}}"#,
            &mut state,
        );
        let stop_value: Value = serde_json::from_str(&stop).expect("stop json");
        assert_eq!(stop_value["result"]["workerState"], "stopped");

        let destroy = handle_ipc_line(
            r#"{"id":4,"method":"instance.destroy","params":{"instanceId":7}}"#,
            &mut state,
        );
        let destroy_value: Value = serde_json::from_str(&destroy).expect("destroy json");
        assert_eq!(destroy_value["result"]["workerState"], "destroyed");
        assert_eq!(state.instances.len(), 0);
    }

    #[test]
    fn serve_writes_one_response_per_line() {
        let input = br#"{"id":1,"method":"worker.hello"}
{"id":2,"method":"worker.metrics"}
"#;
        let mut output = Vec::new();

        serve(&input[..], &mut output).expect("served");

        let text = String::from_utf8(output).expect("utf8");
        assert_eq!(text.lines().count(), 2);
    }
}
