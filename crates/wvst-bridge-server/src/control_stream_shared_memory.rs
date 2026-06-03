use serde_json::{Value, json};

use super::{
    ControlContext, response_error, response_error_data, response_instance_error, response_result,
};
use crate::stream_shared_memory::{
    SharedMemoryStreamError, StreamSharedMemoryCreateParams, StreamSharedMemoryDestroyParams,
};

pub fn handle_stream_shared_memory_create(
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

    match context
        .shared_memory
        .create_for_instance(&record, params.capacity_blocks)
    {
        Ok(descriptor) => response_result(id, json!(descriptor)),
        Err(error) => response_shared_memory_error(id, error),
    }
}

pub fn handle_stream_shared_memory_destroy(
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

    match context
        .shared_memory
        .destroy_by_instance(params.instance_id)
    {
        Ok(descriptor) => response_result(
            id,
            json!({
                "destroyed": descriptor.is_some(),
                "descriptor": descriptor,
            }),
        ),
        Err(error) => response_shared_memory_error(id, error),
    }
}

fn response_shared_memory_error(id: Value, error: SharedMemoryStreamError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}
