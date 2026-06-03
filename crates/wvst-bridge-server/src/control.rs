use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_core::ProtocolVersion;
use wvst_protocol::{AUDIO_FRAME_VERSION, negotiate_protocol};

use crate::audio_in_flight::AudioInFlightLimiter;
use crate::audio_stream_tracker::AudioStreamTracker;
use crate::component_handler_events::ComponentHandlerEventPublisher;
use crate::config::BridgeConfig;
use crate::events::BridgeEventBus;
use crate::host_worker::{HostWorkerClient, HostWorkerError};
use crate::instance_registry::{InstanceError, InstanceRegistry};
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::stream_shared_memory_pump::SharedMemoryPumpRegistry;
use crate::worker_supervisor::{WorkerSupervisor, WorkerSupervisorError};

pub(crate) struct ControlContext<'a> {
    pub(crate) config: &'a BridgeConfig,
    pub(crate) host_worker: &'a HostWorkerClient,
    pub(crate) instances: &'a InstanceRegistry,
    pub(crate) component_handler_events: &'a ComponentHandlerEventPublisher,
    pub(crate) events: &'a BridgeEventBus,
    pub(crate) metrics: &'a Arc<BridgeMetrics>,
    pub(crate) plugins: &'a PluginRegistry,
    pub(crate) stream_tracker: &'a AudioStreamTracker,
    pub(crate) audio_in_flight: &'a AudioInFlightLimiter,
    pub(crate) shared_memory: &'a Arc<SharedMemoryStreamRegistry>,
    pub(crate) shared_memory_pumps: &'a SharedMemoryPumpRegistry,
    pub(crate) origin: Option<&'a str>,
    pub(crate) session_authorized: bool,
    pub(crate) workers: &'a Arc<WorkerSupervisor>,
}

#[derive(Debug, Clone)]
pub(crate) struct ControlResponse {
    pub(crate) text: String,
    pub(crate) session_authorized: bool,
}

impl ControlResponse {
    fn new(text: String, session_authorized: bool) -> Self {
        Self {
            text,
            session_authorized,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelloParams {
    client_name: String,
    client_version: String,
    protocol_min: WireProtocolVersion,
    protocol_max: WireProtocolVersion,
    audio_frame_version: u16,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    origin: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginScanParams {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginListParams {
    #[serde(default)]
    rescan: bool,
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginFactoryInfoParams {
    path: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeEventsParams {
    #[serde(default)]
    after_sequence: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
struct WireProtocolVersion {
    major: u16,
    minor: u16,
}

impl From<WireProtocolVersion> for ProtocolVersion {
    fn from(value: WireProtocolVersion) -> Self {
        Self::new(value.major, value.minor)
    }
}

impl From<ProtocolVersion> for WireProtocolVersion {
    fn from(value: ProtocolVersion) -> Self {
        Self {
            major: value.major,
            minor: value.minor,
        }
    }
}

pub(crate) async fn handle_control_text(
    text: &str,
    context: ControlContext<'_>,
) -> ControlResponse {
    context.metrics.increment_control_messages();

    let request = match serde_json::from_str::<RpcRequest>(text) {
        Ok(request) => request,
        Err(error) => {
            return ControlResponse::new(
                response_error(Value::Null, -32700, format!("parse error: {error}")),
                context.session_authorized,
            );
        }
    };

    if request.method != "bridge.hello" && !context.session_authorized {
        return ControlResponse::new(
            response_error(request.id, 4012, "bridge session not authorized"),
            false,
        );
    }

    let session_authorized = context.session_authorized;
    match request.method.as_str() {
        "bridge.hello" => handle_hello(request.id, request.params, context),
        "bridge.metrics" => ControlResponse::new(
            response_result(request.id, json!(context.metrics.snapshot())),
            session_authorized,
        ),
        "bridge.events" => ControlResponse::new(
            handle_bridge_events(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.scan" => ControlResponse::new(
            handle_plugin_scan(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.list" => ControlResponse::new(
            handle_plugin_list(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.factoryInfo" => ControlResponse::new(
            handle_plugin_factory_info(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.create" => ControlResponse::new(
            control_instances::handle_instance_create(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.list" => ControlResponse::new(
            response_result(request.id, json!(context.instances.list())),
            session_authorized,
        ),
        "instance.status" => ControlResponse::new(
            control_instances::handle_instance_status(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.restart" => ControlResponse::new(
            control_instances::handle_instance_restart(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.start" => ControlResponse::new(
            control_instances::handle_instance_start(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.stop" => ControlResponse::new(
            control_instances::handle_instance_stop(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.parameters" => ControlResponse::new(
            control_instances::handle_instance_parameters(request.id, request.params, context)
                .await,
            session_authorized,
        ),
        "instance.units" => ControlResponse::new(
            control_instances::handle_instance_units(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.metadata.refresh" => ControlResponse::new(
            control_instances::handle_instance_metadata_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.runtime.snapshot" => ControlResponse::new(
            control_instances::handle_instance_runtime_snapshot(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.selectUnit" => ControlResponse::new(
            control_instance_units::handle_instance_select_unit(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.unitByBus" => ControlResponse::new(
            control_instance_units::handle_instance_unit_by_bus(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.setUnitProgramData" => ControlResponse::new(
            control_instance_units::handle_instance_set_unit_program_data(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.setUnitProgramDataAndRefresh" => ControlResponse::new(
            control_instance_units::handle_instance_set_unit_program_data_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.programData.supported" => ControlResponse::new(
            control_instance_units::handle_instance_program_data_supported(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.programData.get" => ControlResponse::new(
            control_instance_units::handle_instance_get_program_data(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.programData.set" => ControlResponse::new(
            control_instance_units::handle_instance_set_program_data(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.programData.setAndRefresh" => ControlResponse::new(
            control_instance_units::handle_instance_set_program_data_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.unitData.supported" => ControlResponse::new(
            control_instance_units::handle_instance_unit_data_supported(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.unitData.get" => ControlResponse::new(
            control_instance_units::handle_instance_get_unit_data(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.unitData.set" => ControlResponse::new(
            control_instance_units::handle_instance_set_unit_data(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.unitData.setAndRefresh" => ControlResponse::new(
            control_instance_units::handle_instance_set_unit_data_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.get" => ControlResponse::new(
            control_instances::handle_instance_parameter_get(request.id, request.params, context)
                .await,
            session_authorized,
        ),
        "instance.parameter.info" => ControlResponse::new(
            control_instances::handle_instance_parameter_info(request.id, request.params, context)
                .await,
            session_authorized,
        ),
        "instance.parameter.valueByString" => ControlResponse::new(
            control_instances::handle_instance_parameter_value_by_string(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.normalizedByPlain" => ControlResponse::new(
            control_instances::handle_instance_parameter_normalized_by_plain(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.set" => ControlResponse::new(
            control_instances::handle_instance_parameter_set(request.id, request.params, context)
                .await,
            session_authorized,
        ),
        "instance.parameter.beginEdit" => ControlResponse::new(
            control_instances::handle_instance_parameter_begin_edit(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.performEdit" => ControlResponse::new(
            control_instances::handle_instance_parameter_perform_edit(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.endEdit" => ControlResponse::new(
            control_instances::handle_instance_parameter_end_edit(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.parameter.edit" => ControlResponse::new(
            control_instances::handle_instance_parameter_edit(request.id, request.params, context)
                .await,
            session_authorized,
        ),
        "instance.getState" => ControlResponse::new(
            control_instances::handle_instance_get_state(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.setState" => ControlResponse::new(
            control_instances::handle_instance_set_state(request.id, request.params, context).await,
            session_authorized,
        ),
        "instance.state.setAndRefresh" => ControlResponse::new(
            control_instances::handle_instance_state_set_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.connection.notifyComponent" => ControlResponse::new(
            control_instances::handle_instance_notify_component(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.connection.notifyComponentAndRefresh" => ControlResponse::new(
            control_instances::handle_instance_notify_component_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.connection.notifyController" => ControlResponse::new(
            control_instances::handle_instance_notify_controller(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.connection.notifyControllerAndRefresh" => ControlResponse::new(
            control_instances::handle_instance_notify_controller_and_refresh(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "instance.destroy" => ControlResponse::new(
            control_instances::handle_instance_destroy(request.id, request.params, context).await,
            session_authorized,
        ),
        "stream.open" => ControlResponse::new(
            control_instances::handle_stream_open(request.id, request.params, context),
            session_authorized,
        ),
        "stream.close" => ControlResponse::new(
            control_instances::handle_stream_close(request.id, request.params, context).await,
            session_authorized,
        ),
        "stream.sharedMemory.create" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_create(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.destroy" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_destroy(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.status" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_status(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.pump.start" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_pump_start(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.pump.stop" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_pump_stop(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.pump.status" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_pump_status(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.pump.enqueueEvents" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_pump_enqueue_events(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.pump.clearEvents" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_pump_clear_events(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        "stream.sharedMemory.process" => ControlResponse::new(
            control_stream_shared_memory::handle_stream_shared_memory_process(
                request.id,
                request.params,
                context,
            )
            .await,
            session_authorized,
        ),
        _ => ControlResponse::new(
            response_error(
                request.id,
                -32601,
                format!("method not found: {}", request.method),
            ),
            session_authorized,
        ),
    }
}

fn handle_plugin_scan(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = parse_params::<PluginScanParams>(params).unwrap_or_default();
    let report = if params.paths.is_empty() {
        context.plugins.scan_default_paths()
    } else {
        context.plugins.scan_paths(paths_from_strings(params.paths))
    };

    response_result(id, json!(report))
}

fn handle_bridge_events(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = parse_params::<BridgeEventsParams>(params).unwrap_or_default();
    let events = context.events.recent_since(params.after_sequence);

    response_result(
        id,
        json!({
            "events": events,
            "lastSequence": events.last().map(|event| event.sequence),
        }),
    )
}

fn handle_plugin_list(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = parse_params::<PluginListParams>(params).unwrap_or_default();
    let report = if params.rescan {
        if params.paths.is_empty() {
            context.plugins.scan_default_paths()
        } else {
            context.plugins.scan_paths(paths_from_strings(params.paths))
        }
    } else {
        context.plugins.list()
    };

    response_result(id, json!(report))
}

async fn handle_plugin_factory_info(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<PluginFactoryInfoParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid factory info params: {error}"));
        }
    };

    if params.path.is_empty() {
        return response_error(id, -32602, "factory info path is required");
    }

    match context.host_worker.factory_info(params.path).await {
        Ok(info) => response_result(id, info),
        Err(error) => response_host_worker_error(id, error),
    }
}

fn handle_hello(id: Value, params: Value, context: ControlContext<'_>) -> ControlResponse {
    let params = match serde_json::from_value::<HelloParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::new(
                response_error(id, -32602, format!("invalid hello params: {error}")),
                context.session_authorized,
            );
        }
    };

    let effective_origin = context.origin.or(params.origin.as_deref());
    if !context.config.origin_is_allowed(effective_origin) {
        return ControlResponse::new(
            response_error(id, 4010, "origin denied"),
            context.session_authorized,
        );
    }

    if !context.config.token_is_valid(params.token.as_deref()) {
        return ControlResponse::new(
            response_error(id, 4011, "invalid pairing token"),
            context.session_authorized,
        );
    }

    if params.audio_frame_version != AUDIO_FRAME_VERSION {
        return ControlResponse::new(
            response_error(id, 4091, "unsupported audio frame version"),
            context.session_authorized,
        );
    }

    let Some(protocol) = negotiate_protocol(
        params.protocol_min.into(),
        params.protocol_max.into(),
        wvst_core::CURRENT_PROTOCOL_VERSION,
    ) else {
        return ControlResponse::new(
            response_error(id, 4090, "protocol version mismatch"),
            context.session_authorized,
        );
    };

    context.metrics.increment_hello_requests();

    ControlResponse::new(
        response_result(
            id,
            json!({
                "bridgeName": "wvst-bridge",
                "bridgeVersion": env!("CARGO_PKG_VERSION"),
                "protocol": WireProtocolVersion::from(protocol),
                "audioFrameVersion": AUDIO_FRAME_VERSION,
                "pairingRequired": context.config.token_required(),
                "origin": effective_origin,
                "client": {
                    "name": params.client_name,
                    "version": params.client_version
                },
                "lowLatency": {
                    "sharedArrayBufferRequired": true,
                    "crossOriginIsolationRequired": true
                },
                "allowedOrigins": context.config.allowed_origins(),
                "metrics": context.metrics.snapshot()
            }),
        ),
        true,
    )
}

fn parse_params<T>(params: Value) -> Result<T, serde_json::Error>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if params.is_null() {
        Ok(T::default())
    } else {
        serde_json::from_value(params)
    }
}

fn paths_from_strings(paths: Vec<String>) -> Vec<std::path::PathBuf> {
    paths.into_iter().map(std::path::PathBuf::from).collect()
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

fn response_host_worker_error(id: Value, error: HostWorkerError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

fn response_instance_error(id: Value, error: InstanceError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

fn response_worker_supervisor_error(id: Value, error: WorkerSupervisorError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
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
#[path = "control_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "control_instance_tests.rs"]
mod instance_tests;

#[path = "control_instances.rs"]
mod control_instances;

#[path = "control_instance_units.rs"]
mod control_instance_units;

#[path = "control_stream_shared_memory.rs"]
mod control_stream_shared_memory;
