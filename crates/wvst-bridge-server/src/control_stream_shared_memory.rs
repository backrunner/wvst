use std::time::Instant;

use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_error_data, response_instance_error, response_result,
    response_worker_supervisor_error,
};
use crate::stream_shared_memory::{
    SharedMemoryStreamError, StreamSharedMemoryCreateParams, StreamSharedMemoryDestroyParams,
    StreamSharedMemoryProcessParams,
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
        .process_shared_memory(params.instance_id, params.frames)
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
