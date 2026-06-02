use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::{
    WorkerIpcState,
    ipc_payload::{
        MAX_CONNECTION_ATTRIBUTE_KEY_BYTES, MAX_CONNECTION_ATTRIBUTE_STRING_BYTES,
        MAX_CONNECTION_MESSAGE_ATTRIBUTES, MAX_CONNECTION_MESSAGE_ID_BYTES,
        decode_message_attribute_base64,
    },
    response_backend_error, response_error, response_result,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceConnectionNotifyParams {
    instance_id: u64,
    message_id: String,
    #[serde(default)]
    attributes: Option<BTreeMap<String, Vst3MessageAttribute>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Vst3MessageAttribute {
    Int {
        value: i64,
    },
    Float {
        value: f64,
    },
    String {
        value: String,
    },
    Binary {
        #[serde(rename = "valueBase64")]
        value_base64: String,
    },
}

pub(super) fn handle_instance_notify_component(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    handle_instance_connection_notify(id, params, state, ConnectionNotifyTarget::Component)
}

pub(super) fn handle_instance_notify_controller(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
) -> String {
    handle_instance_connection_notify(id, params, state, ConnectionNotifyTarget::Controller)
}

#[derive(Debug, Clone, Copy)]
enum ConnectionNotifyTarget {
    Component,
    Controller,
}

fn handle_instance_connection_notify(
    id: Value,
    params: Value,
    state: &mut WorkerIpcState,
    target: ConnectionNotifyTarget,
) -> String {
    let params = match serde_json::from_value::<InstanceConnectionNotifyParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(
                id,
                -32602,
                format!("invalid connection notify params: {error}"),
            );
        }
    };
    if params.message_id.is_empty() {
        return response_error(id, -32602, "messageId is required");
    }
    if params.message_id.len() > MAX_CONNECTION_MESSAGE_ID_BYTES {
        return response_error(
            id,
            4220,
            format!(
                "messageId exceeds byte limit: max {}",
                MAX_CONNECTION_MESSAGE_ID_BYTES
            ),
        );
    }

    let Some(instance) = state.instances.get_mut(&params.instance_id) else {
        return response_error(
            id,
            4040,
            format!("instance not found: {}", params.instance_id),
        );
    };

    let raw_attributes = params.attributes.unwrap_or_default();
    let attributes = match decode_message_attributes(&raw_attributes) {
        Ok(attributes) => attributes,
        Err(error) => return response_error(id, 4220, error),
    };

    let result = match target {
        ConnectionNotifyTarget::Component => instance
            .backend
            .notify_component(&params.message_id, &attributes),
        ConnectionNotifyTarget::Controller => instance
            .backend
            .notify_controller(&params.message_id, &attributes),
    };

    match result {
        Ok(notified) => response_result(
            id,
            json!({
                "instanceId": params.instance_id,
                "target": match target {
                    ConnectionNotifyTarget::Component => "component",
                    ConnectionNotifyTarget::Controller => "controller",
                },
                "messageId": params.message_id,
                "attributeCount": attributes.len(),
                "notified": notified.is_some(),
            }),
        ),
        Err(error) => response_backend_error(id, 4220, &error),
    }
}

pub(super) struct DecodedMessageAttribute {
    pub(super) key: String,
    pub(super) value: DecodedMessageAttributeValue,
}

pub(super) enum DecodedMessageAttributeValue {
    Int(i64),
    Float(f64),
    String(String),
    Binary(Vec<u8>),
}

fn decode_message_attributes(
    attributes: &BTreeMap<String, Vst3MessageAttribute>,
) -> Result<Vec<DecodedMessageAttribute>, String> {
    if attributes.len() > MAX_CONNECTION_MESSAGE_ATTRIBUTES {
        return Err(format!(
            "message attributes exceed limit: max {}",
            MAX_CONNECTION_MESSAGE_ATTRIBUTES
        ));
    }

    attributes
        .iter()
        .map(|(key, attribute)| {
            if key.is_empty() {
                return Err("message attribute key must not be empty".to_string());
            }
            if key.len() > MAX_CONNECTION_ATTRIBUTE_KEY_BYTES {
                return Err(format!(
                    "message attribute key exceeds byte limit: max {}",
                    MAX_CONNECTION_ATTRIBUTE_KEY_BYTES
                ));
            }
            let value = match attribute {
                Vst3MessageAttribute::Int { value } => DecodedMessageAttributeValue::Int(*value),
                Vst3MessageAttribute::Float { value } => {
                    if !value.is_finite() {
                        return Err(format!("message attribute {key} float must be finite"));
                    }
                    DecodedMessageAttributeValue::Float(*value)
                }
                Vst3MessageAttribute::String { value } => {
                    if value.len() > MAX_CONNECTION_ATTRIBUTE_STRING_BYTES {
                        return Err(format!(
                            "message attribute {key} string exceeds byte limit: max {}",
                            MAX_CONNECTION_ATTRIBUTE_STRING_BYTES
                        ));
                    }
                    DecodedMessageAttributeValue::String(value.clone())
                }
                Vst3MessageAttribute::Binary { value_base64 } => {
                    DecodedMessageAttributeValue::Binary(decode_message_attribute_base64(
                        "message attribute valueBase64",
                        value_base64,
                    )?)
                }
            };
            Ok(DecodedMessageAttribute {
                key: key.clone(),
                value,
            })
        })
        .collect()
}
