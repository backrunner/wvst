use serde_json::{Value, json};

use crate::control::ControlContext;
use crate::stream_shared_memory::SharedMemoryStreamError;
use crate::stream_shared_memory_pump::SharedMemoryPumpError;

pub(crate) fn data_plane_snapshot(instance_id: u64, context: &ControlContext<'_>) -> Value {
    json!({
        "sharedMemory": shared_memory_snapshot(instance_id, context),
        "sharedMemoryPump": shared_memory_pump_snapshot(instance_id, context),
    })
}

fn shared_memory_snapshot(instance_id: u64, context: &ControlContext<'_>) -> Value {
    match context.shared_memory.status_by_instance(instance_id) {
        Ok(status) => json!({
            "attached": status.is_some(),
            "status": status,
        }),
        Err(error) => json!({
            "attached": false,
            "status": Value::Null,
            "error": shared_memory_error(error),
        }),
    }
}

fn shared_memory_pump_snapshot(instance_id: u64, context: &ControlContext<'_>) -> Value {
    match context.shared_memory_pumps.status_by_instance(instance_id) {
        Ok(status) => json!({
            "running": status.is_some(),
            "status": status,
        }),
        Err(error) => json!({
            "running": false,
            "status": Value::Null,
            "error": pump_error(error),
        }),
    }
}

fn shared_memory_error(error: SharedMemoryStreamError) -> Value {
    json!({
        "code": error.rpc_code(),
        "message": error.rpc_message(),
        "data": error.rpc_data(),
    })
}

fn pump_error(error: SharedMemoryPumpError) -> Value {
    json!({
        "code": error.rpc_code(),
        "message": error.rpc_message(),
        "data": error.rpc_data(),
    })
}
