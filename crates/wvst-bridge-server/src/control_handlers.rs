use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wvst_core::ProtocolVersion;
use wvst_protocol::{AUDIO_FRAME_VERSION, negotiate_protocol};

use super::{
    ControlContext, ControlResponse, response_error, response_host_worker_error, response_result,
};

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

pub(super) fn handle_plugin_scan(id: Value, params: Value, context: ControlContext<'_>) -> String {
    let params = parse_params::<PluginScanParams>(params).unwrap_or_default();
    let report = if params.paths.is_empty() {
        context.plugins.scan_default_paths()
    } else {
        context.plugins.scan_paths(paths_from_strings(params.paths))
    };

    response_result(id, json!(report))
}

pub(super) fn handle_bridge_events(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> String {
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

pub(super) fn handle_plugin_list(id: Value, params: Value, context: ControlContext<'_>) -> String {
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

pub(super) async fn handle_plugin_factory_info(
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

pub(super) fn handle_hello(
    id: Value,
    params: Value,
    context: ControlContext<'_>,
) -> ControlResponse {
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
