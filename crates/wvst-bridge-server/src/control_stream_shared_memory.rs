use std::time::Instant;

use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_error_data, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::instance_registry::{InstanceState, StreamState};
use crate::stream_shared_memory::{
    SharedMemoryStreamError, StreamSharedMemoryCreateParams, StreamSharedMemoryDestroyParams,
    StreamSharedMemoryProcessParams, StreamSharedMemoryStatusParams,
};
use crate::stream_shared_memory_pump::{
    SharedMemoryPumpClearEventsParams, SharedMemoryPumpEnqueueEventsParams, SharedMemoryPumpError,
    SharedMemoryPumpInstanceParams, SharedMemoryPumpStartParams, pump_config_from_params,
};

pub async fn handle_stream_shared_memory_create(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<StreamSharedMemoryCreateParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory create params: {error}"),
            );
        }
    };
    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };

    let _ = context
        .workers
        .detach_shared_memory(params.instance_id)
        .await;
    let descriptor = match context
        .shared_memory
        .create_for_instance(&record, params.capacity_blocks)
    {
        Ok(descriptor) => descriptor,
        Err(error) => return response_shared_memory_error(id, error),
    };
    let worker = match context
        .workers
        .attach_shared_memory(params.instance_id, descriptor.path.clone())
        .await
    {
        Ok(worker) => worker,
        Err(error) => {
            let _ = context
                .shared_memory
                .destroy_by_instance(params.instance_id);
            return response_worker_supervisor_error(id, error);
        }
    };

    response_result(id, descriptor_with_worker(descriptor, worker))
}

pub async fn handle_stream_shared_memory_destroy(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<StreamSharedMemoryDestroyParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory destroy params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    let worker = context
        .workers
        .detach_shared_memory(params.instance_id)
        .await
        .map(|worker| json!({ "ok": true, "result": worker }))
        .unwrap_or_else(|error| {
            json!({
                "ok": false,
                "code": error.rpc_code(),
                "message": error.rpc_message(),
                "data": error.rpc_data(),
            })
        });

    match context
        .shared_memory
        .destroy_by_instance(params.instance_id)
    {
        Ok(descriptor) => response_result(
            id,
            json!({
                "destroyed": descriptor.is_some(),
                "descriptor": descriptor,
                "worker": worker,
            }),
        ),
        Err(error) => response_shared_memory_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_status(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<StreamSharedMemoryStatusParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory status params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.shared_memory.status_by_instance(params.instance_id) {
        Ok(status) => response_result(
            id,
            json!({
                "attached": status.is_some(),
                "status": status,
            }),
        ),
        Err(error) => response_shared_memory_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_pump_start(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryPumpStartParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory pump start params: {error}"),
            );
        }
    };
    let record = match context.instances.get(params.instance_id) {
        Ok(record) => record,
        Err(error) => return response_instance_error(id, error),
    };
    if record.stream_state != StreamState::Open {
        return response_pump_error(
            id,
            SharedMemoryPumpError::StreamClosed {
                instance_id: record.instance_id,
            },
        );
    }
    if record.state != InstanceState::Processing {
        return response_pump_error(
            id,
            SharedMemoryPumpError::InstanceNotProcessing {
                instance_id: record.instance_id,
            },
        );
    }
    match context.shared_memory.status_by_instance(record.instance_id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return response_pump_error(
                id,
                SharedMemoryPumpError::NotAttached {
                    instance_id: record.instance_id,
                },
            );
        }
        Err(error) => return response_shared_memory_error(id, error),
    }
    let config = match pump_config_from_params(params, record.max_block_frames, record.sample_rate)
    {
        Ok(config) => config,
        Err(error) => return response_pump_error(id, error),
    };

    match context.shared_memory_pumps.start(
        config,
        std::sync::Arc::clone(context.workers),
        std::sync::Arc::clone(context.metrics),
        std::sync::Arc::clone(context.shared_memory),
    ) {
        Ok(status) => response_result(id, json!({ "started": true, "status": status })),
        Err(error) => response_pump_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_pump_stop(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryPumpInstanceParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory pump stop params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .shared_memory_pumps
        .stop_by_instance(params.instance_id)
    {
        Ok(status) => response_result(
            id,
            json!({
                "stopped": status.is_some(),
                "status": status,
            }),
        ),
        Err(error) => response_pump_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_pump_status(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryPumpInstanceParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory pump status params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context
        .shared_memory_pumps
        .status_by_instance(params.instance_id)
    {
        Ok(status) => response_result(
            id,
            json!({
                "running": status.is_some(),
                "status": status,
            }),
        ),
        Err(error) => response_pump_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_pump_enqueue_events(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryPumpEnqueueEventsParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory pump enqueue events params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.shared_memory_pumps.enqueue_events(params) {
        Ok(result) => response_result(id, json!(result)),
        Err(error) => response_pump_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_pump_clear_events(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<SharedMemoryPumpClearEventsParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory pump clear events params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    match context.shared_memory_pumps.clear_events(params) {
        Ok(result) => response_result(id, json!(result)),
        Err(error) => response_pump_error(id, error),
    }
}

pub async fn handle_stream_shared_memory_process(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
    let params = match serde_json::from_value::<StreamSharedMemoryProcessParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid stream shared memory process params: {error}"),
            );
        }
    };
    if let Err(error) = context.instances.get(params.instance_id) {
        return response_instance_error(id, error);
    }

    let started_at = Instant::now();
    match context
        .workers
        .process_shared_memory(
            params.instance_id,
            params.frames,
            params.midi_events,
            params.parameter_events,
        )
        .await
    {
        Ok(result) => {
            context.metrics.record_shared_memory_process_success(
                result.get("frames").and_then(Value::as_u64).unwrap_or(0),
                started_at.elapsed().as_micros() as u64,
            );
            response_result(id, result)
        }
        Err(error) => {
            context
                .metrics
                .record_shared_memory_process_failure(started_at.elapsed().as_micros() as u64);
            response_worker_supervisor_error(id, error)
        }
    }
}

fn response_shared_memory_error(id: Value, error: SharedMemoryStreamError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

fn response_pump_error(id: Value, error: SharedMemoryPumpError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

fn descriptor_with_worker(
    descriptor: crate::stream_shared_memory::SharedMemoryStreamDescriptor,
    worker: Value,
) -> Value {
    let mut value = json!(descriptor);
    if let Some(object) = value.as_object_mut() {
        object.insert("worker".to_string(), worker);
    }
    value
}
