use std::time::Duration;

use serde_json::{Value, json};

use super::super::{ControlContext, response_error, response_instance_error, response_result};
use crate::events::BridgeEventKind;
use crate::instance_registry::StreamLifecycleParams;

const STREAM_CLOSE_DRAIN_TIMEOUT: Duration = Duration::from_millis(500);

pub fn handle_stream_open(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<StreamLifecycleParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid stream open params: {error}"));
        }
    };

    match context.instances.open_stream(params) {
        Ok(record) => {
            context.stream_tracker.reset(record.stream_id);
            context.events.emit(BridgeEventKind::StreamOpened {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
            });
            response_result(id, json!(record))
        }
        Err(error) => response_instance_error(id, error),
    }
}

pub async fn handle_stream_close(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<StreamLifecycleParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid stream close params: {error}"));
        }
    };

    match context.instances.close_stream(params) {
        Ok(record) => {
            context.events.emit(BridgeEventKind::StreamClosing {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
            });
            let drained = context
                .audio_in_flight
                .wait_until_idle(record.stream_id, STREAM_CLOSE_DRAIN_TIMEOUT)
                .await;
            let _ = context
                .shared_memory_pumps
                .stop_by_instance(record.instance_id);
            let _ = context
                .workers
                .detach_shared_memory(record.instance_id)
                .await;
            let _ = context.shared_memory.destroy_by_stream_id(record.stream_id);
            context.stream_tracker.reset(record.stream_id);
            context.events.emit(BridgeEventKind::StreamClosed {
                instance_id: record.instance_id,
                plugin_id: record.plugin_id.clone(),
                stream_id: record.stream_id,
                drain_timed_out: !drained,
            });
            response_result(id, json!(record))
        }
        Err(error) => response_instance_error(id, error),
    }
}
