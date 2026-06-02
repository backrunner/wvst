use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_scanner::{MetadataSource, PluginClass, PluginDescriptor, PluginFormat};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    Vst3InputEvent, Vst3ParameterChange, Vst3ProcessOutput,
};

#[path = "ipc_audio.rs"]
mod ipc_audio;
#[path = "ipc_backend.rs"]
mod ipc_backend;
#[path = "ipc_buffers.rs"]
mod ipc_buffers;
#[path = "ipc_capabilities.rs"]
mod ipc_capabilities;
#[path = "ipc_midi.rs"]
mod ipc_midi;
#[path = "ipc_parameter_events.rs"]
mod ipc_parameter_events;
#[path = "ipc_parameters.rs"]
mod ipc_parameters;
#[path = "ipc_unit_data.rs"]
mod ipc_unit_data;
#[path = "ipc_units.rs"]
mod ipc_units;

use ipc_backend::{WorkerBackend, WorkerBackendKind};
use ipc_buffers::AudioScratchBuffers;
use ipc_capabilities::WorkerRuntimeCapabilities;

const WORKER_IPC_VERSION: u16 = 1;

#[derive(Default)]
pub struct WorkerIpcState {
    instances: BTreeMap<u64, WorkerInstance>,
}

struct WorkerInstance {
    stream_id: u64,
    sample_rate: u32,
    max_block_frames: u16,
    input_channels: usize,
    output_channels: usize,
    processing: bool,
    backend: WorkerBackend,
    capabilities: WorkerRuntimeCapabilities,
    buffers: AudioScratchBuffers,
    events: Vec<Vst3InputEvent>,
    parameter_changes: Vec<Vst3ParameterChange>,
    process_output: Vst3ProcessOutput,
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
    sample_rate: u32,
    max_block_frames: u16,
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
    backend: WorkerBackendKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_class_id: Option<String>,
    runtime_capabilities: WorkerRuntimeCapabilities,
    latency_samples: u32,
    tail_samples: u32,
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
        "instance.parameters" => {
            ipc_parameters::handle_instance_parameters(request.id, request.params, state)
        }
        "instance.units" => {
            ipc_parameters::handle_instance_units(request.id, request.params, state)
        }
        "instance.selectUnit" => {
            ipc_units::handle_instance_select_unit(request.id, request.params, state)
        }
        "instance.unitByBus" => {
            ipc_units::handle_instance_unit_by_bus(request.id, request.params, state)
        }
        "instance.setUnitProgramData" => {
            ipc_units::handle_instance_set_unit_program_data(request.id, request.params, state)
        }
        "instance.programData.supported" => {
            ipc_unit_data::handle_instance_program_data_supported(request.id, request.params, state)
        }
        "instance.programData.get" => {
            ipc_unit_data::handle_instance_get_program_data(request.id, request.params, state)
        }
        "instance.programData.set" => {
            ipc_unit_data::handle_instance_set_program_data(request.id, request.params, state)
        }
        "instance.unitData.supported" => {
            ipc_unit_data::handle_instance_unit_data_supported(request.id, request.params, state)
        }
        "instance.unitData.get" => {
            ipc_unit_data::handle_instance_get_unit_data(request.id, request.params, state)
        }
        "instance.unitData.set" => {
            ipc_unit_data::handle_instance_set_unit_data(request.id, request.params, state)
        }
        "instance.parameter.get" => {
            ipc_parameters::handle_instance_parameter_get(request.id, request.params, state)
        }
        "instance.parameter.info" => {
            ipc_parameters::handle_instance_parameter_info(request.id, request.params, state)
        }
        "instance.parameter.valueByString" => {
            ipc_parameters::handle_instance_parameter_value_by_string(
                request.id,
                request.params,
                state,
            )
        }
        "instance.parameter.normalizedByPlain" => {
            ipc_parameters::handle_instance_parameter_normalized_by_plain(
                request.id,
                request.params,
                state,
            )
        }
        "instance.parameter.set" => {
            ipc_parameters::handle_instance_parameter_set(request.id, request.params, state)
        }
        "instance.parameter.beginEdit" => {
            ipc_parameters::handle_instance_parameter_begin_edit(request.id, request.params, state)
        }
        "instance.parameter.performEdit" => ipc_parameters::handle_instance_parameter_perform_edit(
            request.id,
            request.params,
            state,
        ),
        "instance.parameter.endEdit" => {
            ipc_parameters::handle_instance_parameter_end_edit(request.id, request.params, state)
        }
        "instance.getState" => {
            ipc_parameters::handle_instance_get_state(request.id, request.params, state)
        }
        "instance.setState" => {
            ipc_parameters::handle_instance_set_state(request.id, request.params, state)
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
    if params.sample_rate == 0 {
        return response_error(id, 4220, "invalid sampleRate: 0");
    }
    if params.max_block_frames == 0 {
        return response_error(id, 4220, "invalid maxBlockFrames: 0");
    }

    let descriptor = descriptor_from_params(&params);
    let backend = match WorkerBackend::from_create_params(&params, &descriptor) {
        Ok(backend) => backend,
        Err(error) => {
            return match error.data() {
                Some(data) => response_error_data(id, 4220, error.message(), data.clone()),
                None => response_error(id, 4220, error.message()),
            };
        }
    };
    let buffers = match AudioScratchBuffers::new(
        params.max_block_frames,
        params.input_channels,
        params.output_channels,
    ) {
        Ok(buffers) => buffers,
        Err(error) => return response_error(id, 4220, error),
    };

    let ready = InstanceReady {
        instance_id: params.instance_id,
        stream_id: params.stream_id,
        worker_state: WorkerState::Ready,
        backend: backend.kind(),
        controller_class_id: backend.controller_class_id(),
        runtime_capabilities: backend.capabilities(),
        latency_samples: backend.latency_samples(),
        tail_samples: backend.tail_samples(),
    };
    let capabilities = backend.capabilities();
    state.instances.insert(
        params.instance_id,
        WorkerInstance {
            stream_id: params.stream_id,
            sample_rate: params.sample_rate,
            max_block_frames: params.max_block_frames,
            input_channels: params.input_channels,
            output_channels: params.output_channels,
            processing: false,
            backend,
            capabilities,
            buffers,
            events: Vec::with_capacity(DEFAULT_MAX_VST3_EVENTS_PER_BLOCK),
            parameter_changes: Vec::with_capacity(DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK),
            process_output: Vst3ProcessOutput::with_capacities(
                DEFAULT_MAX_VST3_EVENTS_PER_BLOCK,
                DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
            ),
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

    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };
    if let Err(error) = instance.backend.terminate(instance.processing) {
        return response_error(id, 4220, error);
    }
    let stream_id = instance.stream_id;
    state.instances.remove(&params.instance_id);

    response_result(
        id,
        json!({
            "instanceId": params.instance_id,
            "streamId": stream_id,
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

    let backend_result = if processing {
        instance.backend.start_processing()
    } else {
        instance.backend.stop_processing()
    };
    if let Err(error) = backend_result {
        return response_error(id, 4220, error);
    }

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
            "binaryAudioProcess": true,
            "vst3CreateInstance": true,
            "vst3AudioProcessorProbe": true,
            "vst3RuntimeInstance": true,
            "vst3Parameters": true,
            "vst3ParameterAutomation": true,
            "vst3UnitInfo": true,
            "vst3UnitProgramData": true,
            "vst3ProgramListData": true,
            "vst3UnitData": true,
            "vst3ControllerState": true,
            "preallocatedAudioBuffers": true,
            "sampleRateValidation": true
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
        "passthroughInstances": state
            .instances
            .values()
            .filter(|instance| instance.backend.kind() == WorkerBackendKind::Passthrough)
            .count(),
        "vst3RuntimeInstances": state
            .instances
            .values()
            .filter(|instance| instance.backend.kind() == WorkerBackendKind::Vst3Runtime)
            .count(),
        "runtime": state
            .instances
            .values()
            .map(|instance| {
                json!({
                    "streamId": instance.stream_id,
                    "backend": instance.backend.kind(),
                    "runtimeCapabilities": instance.capabilities,
                    "latencySamples": instance.backend.latency_samples(),
                    "tailSamples": instance.backend.tail_samples(),
                    "diagnostics": instance.backend.diagnostics(),
                })
            })
            .collect::<Vec<_>>(),
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

fn response_error_data(id: Value, code: i64, message: impl Into<String>, data: Value) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
            "data": data
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
        assert_eq!(value["result"]["capabilities"]["vst3Parameters"], true);
        assert_eq!(value["result"]["capabilities"]["vst3UnitProgramData"], true);
        assert_eq!(value["result"]["capabilities"]["vst3ProgramListData"], true);
        assert_eq!(value["result"]["capabilities"]["vst3UnitData"], true);
        assert_eq!(value["result"]["capabilities"]["vst3ControllerState"], true);
    }

    #[test]
    fn creates_processes_and_destroys_passthrough_instance() {
        let mut state = WorkerIpcState::default();
        let create = handle_ipc_line(
            r#"{"id":1,"method":"instance.create","params":{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","classId":"class-a","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":2,"outputChannels":2}}"#,
            &mut state,
        );
        let create_value: Value = serde_json::from_str(&create).expect("create json");
        assert_eq!(create_value["result"]["workerState"], "ready");
        assert_eq!(create_value["result"]["backend"], "passthrough");
        assert_eq!(
            create_value["result"]["runtimeCapabilities"]["schemaVersion"],
            1
        );
        assert_eq!(
            create_value["result"]["runtimeCapabilities"]["binaryAudioProcess"],
            true
        );
        assert_eq!(
            create_value["result"]["runtimeCapabilities"]["parameters"],
            false
        );
        assert_eq!(
            create_value["result"]["runtimeCapabilities"]["controllerState"],
            false
        );
        assert_eq!(create_value["result"]["latencySamples"], 0);
        assert_eq!(create_value["result"]["tailSamples"], 0);

        let metrics = handle_ipc_line(r#"{"id":8,"method":"worker.metrics"}"#, &mut state);
        let metrics_value: Value = serde_json::from_str(&metrics).expect("metrics json");
        assert_eq!(
            metrics_value["result"]["runtime"][0]["backend"],
            "passthrough"
        );
        assert_eq!(
            metrics_value["result"]["runtime"][0]["runtimeCapabilities"]["schemaVersion"],
            1
        );
        assert_eq!(
            metrics_value["result"]["runtime"][0]["runtimeCapabilities"]["binaryAudioProcess"],
            true
        );
        assert_eq!(
            metrics_value["result"]["runtime"][0]["diagnostics"]["passthroughReason"]["kind"],
            "non-bundle-path"
        );
        assert_eq!(
            metrics_value["result"]["runtime"][0]["runtimeCapabilities"]["componentState"],
            false
        );
        assert_eq!(metrics_value["result"]["runtime"][0]["latencySamples"], 0);
        assert_eq!(metrics_value["result"]["runtime"][0]["tailSamples"], 0);
        assert_eq!(
            metrics_value["result"]["runtime"][0]["diagnostics"]["componentHandler"],
            Value::Null
        );

        let parameters = handle_ipc_line(
            r#"{"id":9,"method":"instance.parameters","params":{"instanceId":7}}"#,
            &mut state,
        );
        let parameters_value: Value = serde_json::from_str(&parameters).expect("params json");
        assert_eq!(parameters_value["result"]["parameters"], json!([]));

        let units = handle_ipc_line(
            r#"{"id":11,"method":"instance.units","params":{"instanceId":7}}"#,
            &mut state,
        );
        let units_value: Value = serde_json::from_str(&units).expect("units json");
        assert_eq!(units_value["result"]["unitInfo"], Value::Null);

        let select_unit = handle_ipc_line(
            r#"{"id":12,"method":"instance.selectUnit","params":{"instanceId":7,"unitId":1}}"#,
            &mut state,
        );
        let select_unit_value: Value =
            serde_json::from_str(&select_unit).expect("select unit json");
        assert_eq!(select_unit_value["error"]["code"], 4220);

        let program_supported = handle_ipc_line(
            r#"{"id":13,"method":"instance.programData.supported","params":{"instanceId":7,"listId":1,"programIndex":0}}"#,
            &mut state,
        );
        let program_supported_value: Value =
            serde_json::from_str(&program_supported).expect("program supported json");
        assert_eq!(program_supported_value["result"]["supported"], false);

        let unit_supported = handle_ipc_line(
            r#"{"id":14,"method":"instance.unitData.supported","params":{"instanceId":7,"unitId":1}}"#,
            &mut state,
        );
        let unit_supported_value: Value =
            serde_json::from_str(&unit_supported).expect("unit supported json");
        assert_eq!(unit_supported_value["result"]["supported"], false);

        let set_state_missing = handle_ipc_line(
            r#"{"id":15,"method":"instance.setState","params":{"instanceId":7}}"#,
            &mut state,
        );
        let set_state_missing_value: Value =
            serde_json::from_str(&set_state_missing).expect("set state missing json");
        assert_eq!(set_state_missing_value["error"]["code"], -32602);

        let parameter_get = handle_ipc_line(
            r#"{"id":10,"method":"instance.parameter.get","params":{"instanceId":7,"parameterId":1}}"#,
            &mut state,
        );
        let parameter_get_value: Value =
            serde_json::from_str(&parameter_get).expect("parameter get json");
        assert_eq!(parameter_get_value["error"]["code"], 4040);

        let parameter_info = handle_ipc_line(
            r#"{"id":16,"method":"instance.parameter.info","params":{"instanceId":7,"parameterId":1,"valueNormalized":0.5}}"#,
            &mut state,
        );
        let parameter_info_value: Value =
            serde_json::from_str(&parameter_info).expect("parameter info json");
        assert_eq!(parameter_info_value["error"]["code"], 4220);

        let parameter_value_by_string = handle_ipc_line(
            r#"{"id":17,"method":"instance.parameter.valueByString","params":{"instanceId":7,"parameterId":1,"value":"0.5"}}"#,
            &mut state,
        );
        let parameter_value_by_string_value: Value =
            serde_json::from_str(&parameter_value_by_string).expect("value-by-string json");
        assert_eq!(parameter_value_by_string_value["error"]["code"], 4220);

        let parameter_normalized_by_plain = handle_ipc_line(
            r#"{"id":18,"method":"instance.parameter.normalizedByPlain","params":{"instanceId":7,"parameterId":1,"valuePlain":50.0}}"#,
            &mut state,
        );
        let parameter_normalized_by_plain_value: Value =
            serde_json::from_str(&parameter_normalized_by_plain).expect("normalized-by-plain json");
        assert_eq!(parameter_normalized_by_plain_value["error"]["code"], 4040);

        let parameter_begin_edit = handle_ipc_line(
            r#"{"id":19,"method":"instance.parameter.beginEdit","params":{"instanceId":7,"parameterId":1}}"#,
            &mut state,
        );
        let parameter_begin_edit_value: Value =
            serde_json::from_str(&parameter_begin_edit).expect("begin-edit json");
        assert_eq!(parameter_begin_edit_value["error"]["code"], 4220);

        let parameter_perform_edit = handle_ipc_line(
            r#"{"id":20,"method":"instance.parameter.performEdit","params":{"instanceId":7,"parameterId":1,"valueNormalized":0.5}}"#,
            &mut state,
        );
        let parameter_perform_edit_value: Value =
            serde_json::from_str(&parameter_perform_edit).expect("perform-edit json");
        assert_eq!(parameter_perform_edit_value["error"]["code"], 4220);

        let parameter_end_edit = handle_ipc_line(
            r#"{"id":21,"method":"instance.parameter.endEdit","params":{"instanceId":7,"parameterId":1}}"#,
            &mut state,
        );
        let parameter_end_edit_value: Value =
            serde_json::from_str(&parameter_end_edit).expect("end-edit json");
        assert_eq!(parameter_end_edit_value["error"]["code"], 4220);

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
    fn records_passthrough_reason_when_class_id_is_missing() {
        let mut state = WorkerIpcState::default();
        let create = handle_ipc_line(
            r#"{"id":1,"method":"instance.create","params":{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":2,"outputChannels":2}}"#,
            &mut state,
        );
        let create_value: Value = serde_json::from_str(&create).expect("create json");
        assert_eq!(create_value["result"]["backend"], "passthrough");

        let metrics = handle_ipc_line(r#"{"id":8,"method":"worker.metrics"}"#, &mut state);
        let metrics_value: Value = serde_json::from_str(&metrics).expect("metrics json");
        assert_eq!(
            metrics_value["result"]["runtime"][0]["diagnostics"]["passthroughReason"]["kind"],
            "missing-class-id"
        );
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
        assert_eq!(state.instances.len(), 0);

        let _ = std::fs::remove_dir_all(bundle_path);
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

    fn unique_temp_dir() -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("wvst-host-worker-ipc-test-{suffix}"))
    }
}
