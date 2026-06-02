use std::net::SocketAddr;

use url::{Host, Url};
use wvst_protocol::WORKER_CONTROL_IPC_MAX_BODY_LEN;

use crate::{BridgeError, BridgeResult};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:35876";
const DEFAULT_MAX_WORKER_INSTANCES: usize = 64;
const DEFAULT_MAX_CONTROL_MESSAGE_BYTES: usize = WORKER_CONTROL_IPC_MAX_BODY_LEN as usize;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    bind_addr: SocketAddr,
    token: Option<String>,
    allowed_origins: Vec<String>,
    allow_loopback_origins: bool,
    worker_auto_restart: bool,
    max_worker_instances: usize,
    max_control_message_bytes: usize,
}

impl BridgeConfig {
    pub fn from_env() -> BridgeResult<Self> {
        let bind_addr = std::env::var("WVST_BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse()
            .map_err(|_| BridgeError::InvalidBindAddress(DEFAULT_BIND_ADDR.to_string()))?;

        let token = std::env::var("WVST_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());

        let allowed_origins = std::env::var("WVST_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let allow_loopback_origins = std::env::var("WVST_ALLOW_LOOPBACK_ORIGINS")
            .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let worker_auto_restart = std::env::var("WVST_WORKER_AUTO_RESTART")
            .map(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let max_worker_instances = std::env::var("WVST_MAX_WORKER_INSTANCES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_WORKER_INSTANCES);
        let max_control_message_bytes = std::env::var("WVST_MAX_CONTROL_MESSAGE_BYTES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_CONTROL_MESSAGE_BYTES);

        Ok(Self {
            bind_addr,
            token,
            allowed_origins,
            allow_loopback_origins,
            worker_auto_restart,
            max_worker_instances,
            max_control_message_bytes,
        })
    }

    pub fn development(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            token: None,
            allowed_origins: Vec::new(),
            allow_loopback_origins: true,
            worker_auto_restart: true,
            max_worker_instances: DEFAULT_MAX_WORKER_INSTANCES,
            max_control_message_bytes: DEFAULT_MAX_CONTROL_MESSAGE_BYTES,
        }
    }

    pub fn with_worker_auto_restart(mut self, enabled: bool) -> Self {
        self.worker_auto_restart = enabled;
        self
    }

    pub fn with_max_worker_instances(mut self, max_instances: usize) -> Self {
        self.max_worker_instances = max_instances.max(1);
        self
    }

    pub fn with_max_control_message_bytes(mut self, max_bytes: usize) -> Self {
        self.max_control_message_bytes = max_bytes.max(1);
        self
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn token_required(&self) -> bool {
        self.token.is_some()
    }

    pub fn token_is_valid(&self, provided: Option<&str>) -> bool {
        match self.token.as_deref() {
            Some(expected) => provided == Some(expected),
            None => true,
        }
    }

    pub fn origin_is_allowed(&self, origin: Option<&str>) -> bool {
        let Some(origin) = origin else {
            return true;
        };

        if self.allowed_origins.iter().any(|allowed| allowed == origin) {
            return true;
        }

        self.allow_loopback_origins && is_loopback_origin(origin)
    }

    pub fn allowed_origins(&self) -> &[String] {
        &self.allowed_origins
    }

    pub fn worker_auto_restart_enabled(&self) -> bool {
        self.worker_auto_restart
    }

    pub fn max_worker_instances(&self) -> usize {
        self.max_worker_instances
    }

    pub fn max_control_message_bytes(&self) -> usize {
        self.max_control_message_bytes
    }
}

fn is_loopback_origin(origin: &str) -> bool {
    let Ok(url) = Url::parse(origin) else {
        return false;
    };

    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }

    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_loopback_origins_by_default() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert!(config.origin_is_allowed(Some("http://localhost:5173")));
        assert!(config.origin_is_allowed(Some("https://127.0.0.1:8443")));
        assert!(!config.origin_is_allowed(Some("https://example.com")));
    }

    #[test]
    fn validates_token_when_configured() {
        let mut config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
        config.token = Some("secret".to_string());

        assert!(config.token_is_valid(Some("secret")));
        assert!(!config.token_is_valid(None));
        assert!(!config.token_is_valid(Some("wrong")));
    }

    #[test]
    fn enables_worker_auto_restart_by_default() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert!(config.worker_auto_restart_enabled());
        assert!(
            !config
                .with_worker_auto_restart(false)
                .worker_auto_restart_enabled()
        );
    }

    #[test]
    fn sets_default_and_overridden_worker_instance_limit() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(config.max_worker_instances(), DEFAULT_MAX_WORKER_INSTANCES);
        assert_eq!(
            config.with_max_worker_instances(2).max_worker_instances(),
            2
        );
    }

    #[test]
    fn sets_default_and_overridden_control_message_limit() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(
            config.max_control_message_bytes(),
            DEFAULT_MAX_CONTROL_MESSAGE_BYTES
        );
        assert_eq!(
            config
                .with_max_control_message_bytes(1024)
                .max_control_message_bytes(),
            1024
        );
    }
}
