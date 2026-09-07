use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use url::{Host, Url};
use wvst_process_supervision::DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS;
use wvst_protocol::WORKER_CONTROL_IPC_MAX_BODY_LEN;

use crate::{BridgeError, BridgeResult};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:35876";
const DEFAULT_MAX_WORKER_INSTANCES: usize = 64;
const DEFAULT_MAX_CONTROL_MESSAGE_BYTES: usize = WORKER_CONTROL_IPC_MAX_BODY_LEN as usize;
const DEFAULT_WORKER_QUARANTINE_FAILURE_THRESHOLD: u32 = 3;

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    bind_addr: SocketAddr,
    token: Option<String>,
    allowed_origins: Vec<String>,
    allow_loopback_origins: bool,
    worker_auto_restart: bool,
    worker_load_timeout: Duration,
    max_worker_instances: usize,
    worker_quarantine_failure_threshold: u32,
    max_control_message_bytes: usize,
    worker_memory_limit_bytes: Option<u64>,
    worker_cpu_time_limit_seconds: Option<u64>,
    worker_linux_cgroup_parent: Option<PathBuf>,
    worker_linux_cgroup_memory_max_bytes: Option<u64>,
    worker_linux_cgroup_cpu_quota_micros: Option<u64>,
    worker_linux_cgroup_cpu_period_micros: u64,
}

impl BridgeConfig {
    pub fn from_env() -> BridgeResult<Self> {
        Self::from_env_with_token_requirement(true)
    }

    pub fn from_env_allow_missing_token() -> BridgeResult<Self> {
        Self::from_env_with_token_requirement(false)
    }

    fn from_env_with_token_requirement(require_token: bool) -> BridgeResult<Self> {
        let bind_addr = std::env::var("WVST_BIND_ADDR")
            .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
            .parse()
            .map_err(|_| BridgeError::InvalidBindAddress(DEFAULT_BIND_ADDR.to_string()))?;

        let token = std::env::var("WVST_TOKEN")
            .ok()
            .filter(|value| !value.is_empty());
        if require_token && token.is_none() {
            return Err(BridgeError::MissingToken);
        }

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
        let worker_load_timeout = std::env::var("WVST_WORKER_LOAD_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_secs(120));
        let max_worker_instances = std::env::var("WVST_MAX_WORKER_INSTANCES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_WORKER_INSTANCES);
        let worker_quarantine_failure_threshold = std::env::var("WVST_WORKER_QUARANTINE_FAILURES")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_WORKER_QUARANTINE_FAILURE_THRESHOLD);
        let max_control_message_bytes = std::env::var("WVST_MAX_CONTROL_MESSAGE_BYTES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_CONTROL_MESSAGE_BYTES);
        let worker_memory_limit_bytes = std::env::var("WVST_WORKER_MEMORY_LIMIT_BYTES")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0);
        let worker_cpu_time_limit_seconds = std::env::var("WVST_WORKER_CPU_TIME_LIMIT_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0);
        let worker_linux_cgroup_parent = std::env::var_os("WVST_WORKER_LINUX_CGROUP_PARENT")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let worker_linux_cgroup_memory_max_bytes =
            std::env::var("WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0);
        let worker_linux_cgroup_cpu_quota_micros =
            std::env::var("WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0);
        let worker_linux_cgroup_cpu_period_micros =
            std::env::var("WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS);

        Ok(Self {
            bind_addr,
            token,
            allowed_origins,
            allow_loopback_origins,
            worker_auto_restart,
            worker_load_timeout,
            max_worker_instances,
            worker_quarantine_failure_threshold,
            max_control_message_bytes,
            worker_memory_limit_bytes,
            worker_cpu_time_limit_seconds,
            worker_linux_cgroup_parent,
            worker_linux_cgroup_memory_max_bytes,
            worker_linux_cgroup_cpu_quota_micros,
            worker_linux_cgroup_cpu_period_micros,
        })
    }

    pub fn development(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            token: None,
            allowed_origins: Vec::new(),
            allow_loopback_origins: true,
            worker_auto_restart: true,
            worker_load_timeout: Duration::from_secs(120),
            max_worker_instances: DEFAULT_MAX_WORKER_INSTANCES,
            worker_quarantine_failure_threshold: DEFAULT_WORKER_QUARANTINE_FAILURE_THRESHOLD,
            max_control_message_bytes: DEFAULT_MAX_CONTROL_MESSAGE_BYTES,
            worker_memory_limit_bytes: None,
            worker_cpu_time_limit_seconds: None,
            worker_linux_cgroup_parent: None,
            worker_linux_cgroup_memory_max_bytes: None,
            worker_linux_cgroup_cpu_quota_micros: None,
            worker_linux_cgroup_cpu_period_micros: DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS,
        }
    }

    pub fn with_worker_load_timeout(mut self, timeout: Duration) -> Self {
        self.worker_load_timeout = timeout.max(Duration::from_millis(1));
        self
    }

    pub fn worker_load_timeout(&self) -> Duration {
        self.worker_load_timeout
    }

    pub fn with_worker_auto_restart(mut self, enabled: bool) -> Self {
        self.worker_auto_restart = enabled;
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.token = (!token.is_empty()).then_some(token);
        self
    }

    pub fn without_token(mut self) -> Self {
        self.token = None;
        self
    }

    pub fn with_allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = origins
            .into_iter()
            .map(Into::into)
            .map(|origin| origin.trim().to_string())
            .filter(|origin| !origin.is_empty())
            .collect();
        self
    }

    pub fn with_loopback_origins(mut self, allowed: bool) -> Self {
        self.allow_loopback_origins = allowed;
        self
    }

    pub fn with_max_worker_instances(mut self, max_instances: usize) -> Self {
        self.max_worker_instances = max_instances.max(1);
        self
    }

    pub fn with_worker_quarantine_failure_threshold(mut self, failures: u32) -> Self {
        self.worker_quarantine_failure_threshold = failures.max(1);
        self
    }

    pub fn with_max_control_message_bytes(mut self, max_bytes: usize) -> Self {
        self.max_control_message_bytes = max_bytes.max(1);
        self
    }

    pub fn with_worker_memory_limit_bytes(mut self, bytes: u64) -> Self {
        self.worker_memory_limit_bytes = Some(bytes.max(1));
        self
    }

    pub fn without_worker_memory_limit(mut self) -> Self {
        self.worker_memory_limit_bytes = None;
        self
    }

    pub fn with_worker_cpu_time_limit_seconds(mut self, seconds: u64) -> Self {
        self.worker_cpu_time_limit_seconds = Some(seconds.max(1));
        self
    }

    pub fn without_worker_cpu_time_limit(mut self) -> Self {
        self.worker_cpu_time_limit_seconds = None;
        self
    }

    pub fn with_worker_linux_cgroup_parent(mut self, parent: impl Into<PathBuf>) -> Self {
        self.worker_linux_cgroup_parent = Some(parent.into());
        self
    }

    pub fn without_worker_linux_cgroup(mut self) -> Self {
        self.worker_linux_cgroup_parent = None;
        self.worker_linux_cgroup_memory_max_bytes = None;
        self.worker_linux_cgroup_cpu_quota_micros = None;
        self.worker_linux_cgroup_cpu_period_micros = DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS;
        self
    }

    pub fn with_worker_linux_cgroup_memory_max_bytes(mut self, bytes: u64) -> Self {
        self.worker_linux_cgroup_memory_max_bytes = Some(bytes.max(1));
        self
    }

    pub fn without_worker_linux_cgroup_memory_max(mut self) -> Self {
        self.worker_linux_cgroup_memory_max_bytes = None;
        self
    }

    pub fn with_worker_linux_cgroup_cpu_max_micros(
        mut self,
        quota_micros: u64,
        period_micros: u64,
    ) -> Self {
        self.worker_linux_cgroup_cpu_quota_micros = Some(quota_micros.max(1));
        self.worker_linux_cgroup_cpu_period_micros = period_micros.max(1);
        self
    }

    pub fn without_worker_linux_cgroup_cpu_max(mut self) -> Self {
        self.worker_linux_cgroup_cpu_quota_micros = None;
        self.worker_linux_cgroup_cpu_period_micros = DEFAULT_LINUX_CGROUP_CPU_PERIOD_MICROS;
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

    pub fn worker_quarantine_failure_threshold(&self) -> u32 {
        self.worker_quarantine_failure_threshold
    }

    pub fn max_control_message_bytes(&self) -> usize {
        self.max_control_message_bytes
    }

    pub fn worker_memory_limit_bytes(&self) -> Option<u64> {
        self.worker_memory_limit_bytes
    }

    pub fn worker_cpu_time_limit_seconds(&self) -> Option<u64> {
        self.worker_cpu_time_limit_seconds
    }

    pub fn worker_linux_cgroup_parent(&self) -> Option<&Path> {
        self.worker_linux_cgroup_parent.as_deref()
    }

    pub fn worker_linux_cgroup_memory_max_bytes(&self) -> Option<u64> {
        self.worker_linux_cgroup_memory_max_bytes
    }

    pub fn worker_linux_cgroup_cpu_max_micros(&self) -> Option<(u64, u64)> {
        self.worker_linux_cgroup_cpu_quota_micros
            .map(|quota| (quota, self.worker_linux_cgroup_cpu_period_micros))
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
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
            .with_token("secret");

        assert!(config.token_required());
        assert!(config.token_is_valid(Some("secret")));
        assert!(!config.token_is_valid(None));
        assert!(!config.token_is_valid(Some("wrong")));
    }

    #[test]
    fn clears_token_when_disabled_or_empty() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
            .with_token("secret");

        assert!(!config.clone().without_token().token_required());
        assert!(!config.with_token("").token_required());
    }

    #[test]
    fn configures_allowed_origins_and_loopback_policy() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"))
            .with_allowed_origins([" https://app.example ", "", "https://admin.example"])
            .with_loopback_origins(false);

        assert_eq!(
            config.allowed_origins(),
            ["https://app.example", "https://admin.example"]
        );
        assert!(config.origin_is_allowed(Some("https://app.example")));
        assert!(config.origin_is_allowed(Some("https://admin.example")));
        assert!(!config.origin_is_allowed(Some("http://localhost:5173")));
        assert!(!config.origin_is_allowed(Some("https://example.com")));
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
    fn sets_default_and_overridden_worker_quarantine_threshold() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(
            config.worker_quarantine_failure_threshold(),
            DEFAULT_WORKER_QUARANTINE_FAILURE_THRESHOLD
        );
        assert_eq!(
            config
                .with_worker_quarantine_failure_threshold(2)
                .worker_quarantine_failure_threshold(),
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

    #[test]
    fn configures_optional_worker_memory_limit() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(config.worker_memory_limit_bytes(), None);
        assert_eq!(
            config
                .clone()
                .with_worker_memory_limit_bytes(128 * 1024 * 1024)
                .worker_memory_limit_bytes(),
            Some(128 * 1024 * 1024)
        );
        assert_eq!(
            config
                .with_worker_memory_limit_bytes(128 * 1024 * 1024)
                .without_worker_memory_limit()
                .worker_memory_limit_bytes(),
            None
        );
    }

    #[test]
    fn configures_optional_worker_cpu_time_limit() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(config.worker_cpu_time_limit_seconds(), None);
        assert_eq!(
            config
                .clone()
                .with_worker_cpu_time_limit_seconds(30)
                .worker_cpu_time_limit_seconds(),
            Some(30)
        );
        assert_eq!(
            config
                .with_worker_cpu_time_limit_seconds(30)
                .without_worker_cpu_time_limit()
                .worker_cpu_time_limit_seconds(),
            None
        );
    }

    #[test]
    fn configures_optional_linux_cgroup_limits() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));

        assert_eq!(config.worker_linux_cgroup_parent(), None);
        assert_eq!(config.worker_linux_cgroup_memory_max_bytes(), None);
        assert_eq!(config.worker_linux_cgroup_cpu_max_micros(), None);

        let configured = config
            .clone()
            .with_worker_linux_cgroup_parent("/sys/fs/cgroup/wvst")
            .with_worker_linux_cgroup_memory_max_bytes(256 * 1024 * 1024)
            .with_worker_linux_cgroup_cpu_max_micros(50_000, 100_000);
        assert_eq!(
            configured.worker_linux_cgroup_parent(),
            Some(Path::new("/sys/fs/cgroup/wvst"))
        );
        assert_eq!(
            configured.worker_linux_cgroup_memory_max_bytes(),
            Some(256 * 1024 * 1024)
        );
        assert_eq!(
            configured.worker_linux_cgroup_cpu_max_micros(),
            Some((50_000, 100_000))
        );

        let cleared = configured.without_worker_linux_cgroup();
        assert_eq!(cleared.worker_linux_cgroup_parent(), None);
        assert_eq!(cleared.worker_linux_cgroup_memory_max_bytes(), None);
        assert_eq!(cleared.worker_linux_cgroup_cpu_max_micros(), None);
    }
}
