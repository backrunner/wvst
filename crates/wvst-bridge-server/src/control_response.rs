use serde_json::{Value, json};

use crate::host_worker::HostWorkerError;
use crate::instance_registry::InstanceError;
use crate::worker_supervisor::WorkerSupervisorError;

pub(crate) fn response_result(id: Value, result: Value) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

pub(crate) fn response_error(id: Value, code: i64, message: impl Into<String>) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    }))
}

pub(crate) fn response_host_worker_error(id: Value, error: HostWorkerError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

pub(crate) fn response_instance_error(id: Value, error: InstanceError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

pub(crate) fn response_worker_supervisor_error(id: Value, error: WorkerSupervisorError) -> String {
    response_error_data(id, error.rpc_code(), error.rpc_message(), error.rpc_data())
}

pub(crate) fn response_error_data(
    id: Value,
    code: i64,
    message: impl Into<String>,
    data: Value,
) -> String {
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
