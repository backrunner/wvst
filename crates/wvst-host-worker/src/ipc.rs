use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_protocol::{WORKER_CONTROL_IPC_MAX_BODY_LEN, WORKER_CONTROL_IPC_SCHEMA_VERSION};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    DEFAULT_MAX_VST3_STATE_BYTES, Vst3InputEvent, Vst3ParameterChange, Vst3ProcessOutput,
};

#[path = "ipc_audio.rs"]
mod ipc_audio;
#[path = "ipc_backend.rs"]
mod ipc_backend;
#[path = "ipc_backend_audio_bus.rs"]
mod ipc_backend_audio_bus;
#[path = "ipc_backend_error.rs"]
mod ipc_backend_error;
#[path = "ipc_buffers.rs"]
mod ipc_buffers;
#[path = "ipc_capabilities.rs"]
mod ipc_capabilities;
#[path = "ipc_connection.rs"]
mod ipc_connection;
#[path = "ipc_event_ordering.rs"]
mod ipc_event_ordering;
#[path = "ipc_framed.rs"]
mod ipc_framed;
#[path = "ipc_midi.rs"]
mod ipc_midi;
#[path = "ipc_parameter_events.rs"]
mod ipc_parameter_events;
#[path = "ipc_parameters.rs"]
mod ipc_parameters;
#[path = "ipc_payload.rs"]
mod ipc_payload;
#[path = "ipc_shared_memory.rs"]
mod ipc_shared_memory;
#[path = "ipc_state.rs"]
mod ipc_state;
#[path = "ipc_unit_data.rs"]
mod ipc_unit_data;
#[path = "ipc_units.rs"]
mod ipc_units;
#[path = "ipc_vst3_events.rs"]
mod ipc_vst3_events;

use ipc_backend::{WorkerBackend, WorkerBackendKind};
use ipc_backend_error::WorkerBackendError;
use ipc_buffers::AudioScratchBuffers;
use ipc_capabilities::WorkerRuntimeCapabilities;
pub use ipc_framed::serve_framed_stdio;
use ipc_shared_memory::WorkerSharedMemoryStream;

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
    shared_memory: Option<WorkerSharedMemoryStream>,
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
    plugin_path: String,
    #[serde(default)]
    class_id: Option<String>,
    sample_rate: u32,
    max_block_frames: u16,
    input_channels: usize,
    output_channels: usize,
    #[serde(default)]
    input_bus_index: Option<i32>,
    #[serde(default)]
    output_bus_index: Option<i32>,
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
    tail_info: wvst_vst3_host::Vst3TailSamples,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum WorkerState {
    Ready,
    Processing,
    Stopped,
    Destroyed,
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
        "stream.sharedMemory.attach" => {
            ipc_shared_memory::handle_stream_shared_memory_attach(request.id, request.params, state)
        }
        "stream.sharedMemory.detach" => {
            ipc_shared_memory::handle_stream_shared_memory_detach(request.id, request.params, state)
        }
        "stream.sharedMemory.process" => ipc_shared_memory::handle_stream_shared_memory_process(
            request.id,
            request.params,
            state,
        ),
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
            ipc_state::handle_instance_get_state(request.id, request.params, state)
        }
        "instance.setState" => {
            ipc_state::handle_instance_set_state(request.id, request.params, state)
        }
        "instance.connection.notifyComponent" => {
            ipc_connection::handle_instance_notify_component(request.id, request.params, state)
        }
        "instance.connection.notifyController" => {
            ipc_connection::handle_instance_notify_controller(request.id, request.params, state)
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

    let backend = match WorkerBackend::from_create_params(&params) {
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
        tail_info: backend.tail_info(),
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
            shared_memory: None,
        },
    );

    response_result(id, json!(ready))
}

#[cfg(test)]
fn insert_test_audio_instance(
    state: &mut WorkerIpcState,
    input_channels: usize,
    output_channels: usize,
) {
    let backend = WorkerBackend::test_audio_runtime(input_channels, output_channels);
    let capabilities = backend.capabilities();
    let buffers =
        AudioScratchBuffers::new(128, input_channels, output_channels).expect("test audio buffers");

    state.instances.insert(
        7,
        WorkerInstance {
            stream_id: 9,
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels,
            output_channels,
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
            shared_memory: None,
        },
    );
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
        return response_backend_error(id, 4220, &error);
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
        return response_backend_error(id, 4220, &error);
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
            "sampleRateValidation": true,
            "framedControlIpc": true,
            "framedControlIpcVersion": WORKER_CONTROL_IPC_SCHEMA_VERSION,
            "framedControlMaxBodyBytes": WORKER_CONTROL_IPC_MAX_BODY_LEN,
            "framedControlSequenceIds": true,
            "framedControlStatusCodes": true,
            "framedControlErrorResponses": true,
            "framedControlBatching": true,
            "vst3ControlErrors": true,
            "vst3ControlPayloadLimits": true,
            "sharedMemoryAudio": true,
            "maxVst3StateBytes": DEFAULT_MAX_VST3_STATE_BYTES
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
                    "runtimeCapabilities": &instance.capabilities,
                    "latencySamples": instance.backend.latency_samples(),
                    "tailSamples": instance.backend.tail_samples(),
                    "tailInfo": instance.backend.tail_info(),
                    "sharedMemory": instance
                        .shared_memory
                        .as_ref()
                        .map(|shared_memory| shared_memory.diagnostics())
                        .unwrap_or(Value::Null),
                    "diagnostics": instance
                        .backend
                        .diagnostics_with_process_output(Some(&instance.process_output)),
                })
            })
            .collect::<Vec<_>>(),
    })
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

fn response_backend_error(id: Value, code: i64, error: &WorkerBackendError) -> String {
    match error.data() {
        Some(data) => response_error_data(id, code, error.message(), data.clone()),
        None => response_error(id, code, error.message()),
    }
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
#[path = "ipc_tests.rs"]
mod tests;
