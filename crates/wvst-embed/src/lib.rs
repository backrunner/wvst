use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;
use wvst_bridge_server::{
    BridgeConfig, BridgeError, BridgeEvent, BridgeEventBus, BridgeServer, HostWorkerClient,
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
    shutdown: Option<oneshot::Sender<()>>,
    join: JoinHandle<Result<(), BridgeError>>,
}

#[derive(Debug)]
pub enum EmbedError {
    Bridge(BridgeError),
    RuntimeTask(tokio::task::JoinError),
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

    pub async fn shutdown(mut self) -> EmbedResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.join.await??;
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

    #[test]
    fn builder_accepts_worker_executable_and_timeout() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let runtime = BridgeRuntime::builder(config)
            .worker_executable("/tmp/wvst-host-worker")
            .worker_timeout(Duration::from_millis(250))
            .build();

        assert_eq!(
            runtime.worker_executable.as_deref(),
            Some(std::path::Path::new("/tmp/wvst-host-worker"))
        );
        assert_eq!(runtime.worker_timeout, Duration::from_millis(250));
    }
}
