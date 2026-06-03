use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::audio_in_flight::AudioInFlightLimiter;
use crate::audio_stream_tracker::AudioStreamTracker;
use crate::component_handler_events::ComponentHandlerEventPublisher;
use crate::config::BridgeConfig;
use crate::events::BridgeEventBus;
use crate::host_worker::HostWorkerClient;
use crate::instance_registry::InstanceRegistry;
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::stream_shared_memory_pump::SharedMemoryPumpRegistry;
use crate::worker_supervisor::WorkerSupervisor;

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
        "bridge.hello" => control_handlers::handle_hello(request.id, request.params, context),
        "bridge.metrics" => ControlResponse::new(
            response_result(request.id, json!(context.metrics.snapshot())),
            session_authorized,
        ),
        "bridge.events" => ControlResponse::new(
            control_handlers::handle_bridge_events(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.scan" => ControlResponse::new(
            control_handlers::handle_plugin_scan(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.list" => ControlResponse::new(
            control_handlers::handle_plugin_list(request.id, request.params, context),
            session_authorized,
        ),
        "plugin.factoryInfo" => ControlResponse::new(
            control_handlers::handle_plugin_factory_info(request.id, request.params, context).await,
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

#[path = "control_handlers.rs"]
mod control_handlers;

#[path = "control_response.rs"]
mod control_response;

pub(crate) use control_response::{
    response_error, response_error_data, response_host_worker_error, response_instance_error,
    response_result, response_worker_supervisor_error,
};

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
