use wvst_core::{CURRENT_PROTOCOL_VERSION, ProtocolVersion};

pub const HELLO_METHOD: &str = "bridge.hello";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct HelloRequest<'a> {
    pub client_name: &'a str,
    pub client_version: &'a str,
    pub protocol_min: ProtocolVersion,
    pub protocol_max: ProtocolVersion,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HelloResponse {
    pub bridge_name: String,
    pub bridge_version: String,
    pub protocol: ProtocolVersion,
    pub audio_frame_version: u16,
}

impl HelloResponse {
    pub fn current(bridge_version: impl Into<String>) -> Self {
        Self {
            bridge_name: "wvst-bridge".to_string(),
            bridge_version: bridge_version.into(),
            protocol: CURRENT_PROTOCOL_VERSION,
            audio_frame_version: crate::AUDIO_FRAME_VERSION,
        }
    }
}

pub fn negotiate_protocol(
    client_min: ProtocolVersion,
    client_max: ProtocolVersion,
    server: ProtocolVersion,
) -> Option<ProtocolVersion> {
    if client_min.major != server.major || client_max.major != server.major {
        return None;
    }

    if server.minor < client_min.minor || server.minor > client_max.minor {
        return None;
    }

    Some(server)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_matching_major_version() {
        let negotiated = negotiate_protocol(
            ProtocolVersion::new(1, 0),
            ProtocolVersion::new(1, 2),
            ProtocolVersion::new(1, 1),
        );

        assert_eq!(negotiated, Some(ProtocolVersion::new(1, 1)));
    }

    #[test]
    fn rejects_incompatible_major_version() {
        let negotiated = negotiate_protocol(
            ProtocolVersion::new(1, 0),
            ProtocolVersion::new(1, 2),
            ProtocolVersion::new(2, 0),
        );

        assert_eq!(negotiated, None);
    }
}
