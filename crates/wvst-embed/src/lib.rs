use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;
use wvst_bridge_server::{
    BridgeConfig, BridgeError, BridgeEvent, BridgeEventBus, BridgeMetricsHandle,
    BridgeMetricsSnapshot, BridgeServer, HostWorkerClient,
};

pub type EmbedResult<T> = Result<T, EmbedError>;

#[derive(Debug)]
pub struct BridgeRuntime {
    config: BridgeConfig,
    worker_executable: Option<PathBuf>,
    worker_timeout: Duration,
    events: BridgeEventBus,
}

#[derive(Debug, Clone)]
pub struct BridgeRuntimeOptions {
    pub config: BridgeConfig,
    pub worker_executable: Option<PathBuf>,
    pub worker_timeout: Duration,
}

#[derive(Debug)]
pub struct BridgeRuntimeBuilder {
    options: BridgeRuntimeOptions,
    events: BridgeEventBus,
}

#[derive(Debug)]
pub struct BridgeHandle {
    local_addr: SocketAddr,
    events: BridgeEventBus,
    metrics: BridgeMetricsHandle,
    shutdown: Option<oneshot::Sender<()>>,
    join: JoinHandle<Result<(), BridgeError>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeRuntimeDiagnostics {
    pub local_addr: SocketAddr,
    pub metrics: BridgeMetricsSnapshot,
    pub recent_events: Vec<BridgeEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "recordType",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum BridgeRuntimeLogRecord {
    Event {
        event: BridgeEvent,
    },
    Diagnostics {
        diagnostics: Box<BridgeRuntimeDiagnostics>,
    },
}

#[derive(Debug)]
pub enum EmbedError {
    Bridge(BridgeError),
    RuntimeTask(tokio::task::JoinError),
}

#[derive(Debug)]
pub enum BridgeRuntimeLogError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl BridgeRuntime {
    pub fn new(config: BridgeConfig) -> Self {
        Self::builder(config).build()
    }

    pub fn builder(config: BridgeConfig) -> BridgeRuntimeBuilder {
        BridgeRuntimeBuilder::new(config)
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BridgeEvent> {
        self.events.subscribe()
    }

    pub fn recent_events(&self, after_sequence: Option<u64>) -> Vec<BridgeEvent> {
        self.events.recent_since(after_sequence)
    }

    pub async fn start(self) -> EmbedResult<BridgeHandle> {
        let worker = self
            .worker_executable
            .map(|executable| HostWorkerClient::new(executable, self.worker_timeout));
        let events = self.events.clone();
        let server = match worker {
            Some(worker) => {
                BridgeServer::bind_with_host_worker_and_events(self.config, worker, events.clone())
                    .await?
            }
            None => {
                BridgeServer::bind_with_host_worker_and_events(
                    self.config,
                    HostWorkerClient::from_env(),
                    events.clone(),
                )
                .await?
            }
        };
        let local_addr = server.local_addr()?;
        let metrics = server.metrics_handle();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let join = tokio::spawn(async move {
            server
                .serve_until(async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        Ok(BridgeHandle {
            local_addr,
            events,
            metrics,
            shutdown: Some(shutdown_tx),
            join,
        })
    }
}

impl BridgeHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BridgeEvent> {
        self.events.subscribe()
    }

    pub fn recent_events(&self, after_sequence: Option<u64>) -> Vec<BridgeEvent> {
        self.events.recent_since(after_sequence)
    }

    pub fn metrics_snapshot(&self) -> BridgeMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn diagnostics(&self, after_sequence: Option<u64>) -> BridgeRuntimeDiagnostics {
        BridgeRuntimeDiagnostics {
            local_addr: self.local_addr,
            metrics: self.metrics_snapshot(),
            recent_events: self.recent_events(after_sequence),
        }
    }

    pub async fn shutdown(mut self) -> EmbedResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.join.await??;
        Ok(())
    }
}

impl BridgeRuntimeLogRecord {
    pub const fn event(event: BridgeEvent) -> Self {
        Self::Event { event }
    }

    pub fn diagnostics(diagnostics: BridgeRuntimeDiagnostics) -> Self {
        Self::Diagnostics {
            diagnostics: Box::new(diagnostics),
        }
    }

    pub fn write_json_line(&self, writer: &mut impl Write) -> Result<(), BridgeRuntimeLogError> {
        serde_json::to_writer(&mut *writer, self)?;
        writer.write_all(b"\n")?;
        Ok(())
    }
}

impl BridgeRuntimeOptions {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            worker_executable: None,
            worker_timeout: Duration::from_secs(5),
        }
    }
}

impl BridgeRuntimeBuilder {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            options: BridgeRuntimeOptions::new(config),
            events: BridgeEventBus::new(),
        }
    }

    pub fn worker_executable(mut self, executable: impl Into<PathBuf>) -> Self {
        self.options.worker_executable = Some(executable.into());
        self
    }

    pub fn worker_timeout(mut self, timeout: Duration) -> Self {
        self.options.worker_timeout = timeout;
        self
    }

    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.options.config = self.options.config.with_token(token);
        self
    }

    pub fn without_token(mut self) -> Self {
        self.options.config = self.options.config.without_token();
        self
    }

    pub fn allowed_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.options.config = self.options.config.with_allowed_origins(origins);
        self
    }

    pub fn allow_loopback_origins(mut self, allowed: bool) -> Self {
        self.options.config = self.options.config.with_loopback_origins(allowed);
        self
    }

    pub fn worker_auto_restart(mut self, enabled: bool) -> Self {
        self.options.config = self.options.config.with_worker_auto_restart(enabled);
        self
    }

    pub fn max_worker_instances(mut self, max_instances: usize) -> Self {
        self.options.config = self.options.config.with_max_worker_instances(max_instances);
        self
    }

    pub fn worker_quarantine_failure_threshold(mut self, failures: u32) -> Self {
        self.options.config = self
            .options
            .config
            .with_worker_quarantine_failure_threshold(failures);
        self
    }

    pub fn max_control_message_bytes(mut self, max_bytes: usize) -> Self {
        self.options.config = self
            .options
            .config
            .with_max_control_message_bytes(max_bytes);
        self
    }

    pub fn worker_memory_limit_bytes(mut self, bytes: u64) -> Self {
        self.options.config = self.options.config.with_worker_memory_limit_bytes(bytes);
        self
    }

    pub fn without_worker_memory_limit(mut self) -> Self {
        self.options.config = self.options.config.without_worker_memory_limit();
        self
    }

    pub fn worker_cpu_time_limit_seconds(mut self, seconds: u64) -> Self {
        self.options.config = self
            .options
            .config
            .with_worker_cpu_time_limit_seconds(seconds);
        self
    }

    pub fn without_worker_cpu_time_limit(mut self) -> Self {
        self.options.config = self.options.config.without_worker_cpu_time_limit();
        self
    }

    pub fn worker_linux_cgroup_parent(mut self, parent: impl Into<PathBuf>) -> Self {
        self.options.config = self.options.config.with_worker_linux_cgroup_parent(parent);
        self
    }

    pub fn without_worker_linux_cgroup(mut self) -> Self {
        self.options.config = self.options.config.without_worker_linux_cgroup();
        self
    }

    pub fn worker_linux_cgroup_memory_max_bytes(mut self, bytes: u64) -> Self {
        self.options.config = self
            .options
            .config
            .with_worker_linux_cgroup_memory_max_bytes(bytes);
        self
    }

    pub fn worker_linux_cgroup_cpu_max_micros(
        mut self,
        quota_micros: u64,
        period_micros: u64,
    ) -> Self {
        self.options.config = self
            .options
            .config
            .with_worker_linux_cgroup_cpu_max_micros(quota_micros, period_micros);
        self
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<BridgeEvent> {
        self.events.subscribe()
    }

    pub fn build(self) -> BridgeRuntime {
        BridgeRuntime {
            config: self.options.config,
            worker_executable: self.options.worker_executable,
            worker_timeout: self.options.worker_timeout,
            events: self.events,
        }
    }
}

impl Display for EmbedError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bridge(error) => write!(formatter, "bridge runtime error: {error}"),
            Self::RuntimeTask(error) => write!(formatter, "bridge runtime task failed: {error}"),
        }
    }
}

impl Error for EmbedError {}

impl Display for BridgeRuntimeLogError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "bridge runtime log I/O error: {error}"),
            Self::Json(error) => write!(formatter, "bridge runtime log JSON error: {error}"),
        }
    }
}

impl Error for BridgeRuntimeLogError {}

impl From<BridgeError> for EmbedError {
    fn from(value: BridgeError) -> Self {
        Self::Bridge(value)
    }
}

impl From<tokio::task::JoinError> for EmbedError {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::RuntimeTask(value)
    }
}

impl From<std::io::Error> for BridgeRuntimeLogError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for BridgeRuntimeLogError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    use wvst_bridge_server::BridgeEventKind;

    #[tokio::test]
    async fn starts_and_shuts_down_embedded_bridge() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let handle = BridgeRuntime::new(config).start().await.expect("runtime");

        assert!(handle.local_addr().port() > 0);
        handle.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn exposes_runtime_events() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let builder = BridgeRuntime::builder(config);
        let mut events = builder.subscribe_events();
        let handle = builder.build().start().await.expect("runtime");

        let event = timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("event timeout")
            .expect("event");
        assert!(matches!(
            event.kind,
            BridgeEventKind::ServerStarting | BridgeEventKind::ServerStarted { .. }
        ));
        assert!(!handle.recent_events(None).is_empty());

        handle.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn exposes_handle_diagnostics() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let handle = BridgeRuntime::new(config).start().await.expect("runtime");

        let diagnostics = handle.diagnostics(None);

        assert_eq!(diagnostics.local_addr, handle.local_addr());
        assert_eq!(diagnostics.metrics.websocket_connections, 0);
        assert!(!diagnostics.recent_events.is_empty());

        handle.shutdown().await.expect("shutdown");
    }

    #[test]
    fn builder_accepts_worker_executable_and_timeout() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let runtime = BridgeRuntime::builder(config)
            .worker_executable("/tmp/wvst-host-worker")
            .worker_timeout(Duration::from_millis(250))
            .token("secret")
            .allowed_origins([" https://app.example ", "", "https://admin.example"])
            .allow_loopback_origins(false)
            .worker_auto_restart(false)
            .max_worker_instances(2)
            .worker_quarantine_failure_threshold(2)
            .max_control_message_bytes(4096)
            .worker_memory_limit_bytes(64 * 1024 * 1024)
            .worker_cpu_time_limit_seconds(30)
            .worker_linux_cgroup_parent("/sys/fs/cgroup/wvst")
            .worker_linux_cgroup_memory_max_bytes(128 * 1024 * 1024)
            .worker_linux_cgroup_cpu_max_micros(50_000, 100_000)
            .build();

        assert_eq!(
            runtime.worker_executable.as_deref(),
            Some(std::path::Path::new("/tmp/wvst-host-worker"))
        );
        assert_eq!(runtime.worker_timeout, Duration::from_millis(250));
        assert!(runtime.config.token_required());
        assert!(runtime.config.token_is_valid(Some("secret")));
        assert!(!runtime.config.token_is_valid(Some("wrong")));
        assert_eq!(
            runtime.config.allowed_origins(),
            ["https://app.example", "https://admin.example"]
        );
        assert!(
            runtime
                .config
                .origin_is_allowed(Some("https://app.example"))
        );
        assert!(
            !runtime
                .config
                .origin_is_allowed(Some("http://localhost:5173"))
        );
        assert!(!runtime.config.worker_auto_restart_enabled());
        assert_eq!(runtime.config.max_worker_instances(), 2);
        assert_eq!(runtime.config.worker_quarantine_failure_threshold(), 2);
        assert_eq!(runtime.config.max_control_message_bytes(), 4096);
        assert_eq!(
            runtime.config.worker_memory_limit_bytes(),
            Some(64 * 1024 * 1024)
        );
        assert_eq!(runtime.config.worker_cpu_time_limit_seconds(), Some(30));
        assert_eq!(
            runtime.config.worker_linux_cgroup_parent(),
            Some(std::path::Path::new("/sys/fs/cgroup/wvst"))
        );
        assert_eq!(
            runtime.config.worker_linux_cgroup_memory_max_bytes(),
            Some(128 * 1024 * 1024)
        );
        assert_eq!(
            runtime.config.worker_linux_cgroup_cpu_max_micros(),
            Some((50_000, 100_000))
        );
    }

    #[test]
    fn builder_clears_optional_bridge_policy() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let runtime = BridgeRuntime::builder(config)
            .token("secret")
            .without_token()
            .worker_memory_limit_bytes(64 * 1024 * 1024)
            .without_worker_memory_limit()
            .worker_cpu_time_limit_seconds(30)
            .without_worker_cpu_time_limit()
            .worker_linux_cgroup_parent("/sys/fs/cgroup/wvst")
            .worker_linux_cgroup_memory_max_bytes(128 * 1024 * 1024)
            .worker_linux_cgroup_cpu_max_micros(50_000, 100_000)
            .without_worker_linux_cgroup()
            .build();

        assert!(!runtime.config.token_required());
        assert_eq!(runtime.config.worker_memory_limit_bytes(), None);
        assert_eq!(runtime.config.worker_cpu_time_limit_seconds(), None);
        assert_eq!(runtime.config.worker_linux_cgroup_parent(), None);
        assert_eq!(runtime.config.worker_linux_cgroup_memory_max_bytes(), None);
        assert_eq!(runtime.config.worker_linux_cgroup_cpu_max_micros(), None);
    }

    #[tokio::test]
    async fn writes_event_and_diagnostics_json_lines() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let handle = BridgeRuntime::new(config).start().await.expect("runtime");
        let event = handle
            .recent_events(None)
            .into_iter()
            .next()
            .expect("recent event");
        let diagnostics = handle.diagnostics(None);
        let mut output = Vec::new();

        BridgeRuntimeLogRecord::event(event)
            .write_json_line(&mut output)
            .expect("event log line");
        BridgeRuntimeLogRecord::diagnostics(diagnostics)
            .write_json_line(&mut output)
            .expect("diagnostics log line");

        let text = String::from_utf8(output).expect("utf8");
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"recordType\":\"event\""));
        assert!(lines[1].contains("\"recordType\":\"diagnostics\""));

        handle.shutdown().await.expect("shutdown");
    }
}
