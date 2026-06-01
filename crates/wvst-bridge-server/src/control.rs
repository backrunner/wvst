use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_core::ProtocolVersion;
use wvst_protocol::{AUDIO_FRAME_VERSION, negotiate_protocol};

use crate::config::BridgeConfig;
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;

pub struct ControlContext<'a> {
    pub config: &'a BridgeConfig,
    pub metrics: &'a BridgeMetrics,
    pub plugins: &'a PluginRegistry,
    pub origin: Option<&'a str>,
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

pub fn handle_control_text(text: &str, context: ControlContext<'_>) -> String {
    context.metrics.increment_control_messages();

    let request = match serde_json::from_str::<RpcRequest>(text) {
        Ok(request) => request,
        Err(error) => {
            return response_error(Value::Null, -32700, format!("parse error: {error}"));
        }
    };

    match request.method.as_str() {
        "bridge.hello" => handle_hello(request.id, request.params, context),
        "bridge.metrics" => response_result(request.id, json!(context.metrics.snapshot())),
        "plugin.scan" => handle_plugin_scan(request.id, request.params, context),
        "plugin.list" => handle_plugin_list(request.id, request.params, context),
        _ => response_error(
            request.id,
            -32601,
            format!("method not found: {}", request.method),
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

fn handle_hello(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = match serde_json::from_value::<HelloParams>(params) {
        Ok(params) => params,
        Err(error) => {
            return response_error(id, -32602, format!("invalid hello params: {error}"));
        }
    };

    let effective_origin = context.origin.or(params.origin.as_deref());
    if !context.config.origin_is_allowed(effective_origin) {
        return response_error(id, 4010, "origin denied");
    }

    if !context.config.token_is_valid(params.token.as_deref()) {
        return response_error(id, 4011, "invalid pairing token");
    }

    if params.audio_frame_version != AUDIO_FRAME_VERSION {
        return response_error(id, 4091, "unsupported audio frame version");
    }

    let Some(protocol) = negotiate_protocol(
        params.protocol_min.into(),
        params.protocol_max.into(),
        wvst_core::CURRENT_PROTOCOL_VERSION,
    ) else {
        return response_error(id, 4090, "protocol version mismatch");
    };

    context.metrics.increment_hello_requests();

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

fn serialize_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"internal error\"}}"
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::BridgeMetrics;
    use crate::plugin_registry::PluginRegistry;

    #[test]
    fn responds_to_hello() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
        let metrics = BridgeMetrics::new();
        let plugins = PluginRegistry::new();
        let context = ControlContext {
            config: &config,
            metrics: &metrics,
            plugins: &plugins,
            origin: Some("http://localhost:5173"),
        };

        let response = handle_control_text(
            r#"{"jsonrpc":"2.0","id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#,
            context,
        );
        let value: Value = serde_json::from_str(&response).expect("valid json");

        assert_eq!(value["id"], 1);
        assert_eq!(value["result"]["bridgeName"], "wvst-bridge");
    }

    #[test]
    fn rejects_denied_origin() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
        let metrics = BridgeMetrics::new();
        let plugins = PluginRegistry::new();
        let context = ControlContext {
            config: &config,
            metrics: &metrics,
            plugins: &plugins,
            origin: Some("https://example.com"),
        };

        let response = handle_control_text(
            r#"{"id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#,
            context,
        );
        let value: Value = serde_json::from_str(&response).expect("valid json");

        assert_eq!(value["error"]["code"], 4010);
    }

    #[test]
    fn lists_cached_plugins() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
        let metrics = BridgeMetrics::new();
        let plugins = PluginRegistry::new();
        let context = ControlContext {
            config: &config,
            metrics: &metrics,
            plugins: &plugins,
            origin: None,
        };

        let response =
            handle_control_text(r#"{"id":1,"method":"plugin.list","params":{}}"#, context);
        let value: Value = serde_json::from_str(&response).expect("valid json");

        assert_eq!(
            value["result"]["plugins"]
                .as_array()
                .expect("plugins")
                .len(),
            0
        );
    }
}
