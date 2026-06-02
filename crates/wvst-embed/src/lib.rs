use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::SocketAddr;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use wvst_bridge_server::{BridgeConfig, BridgeError, BridgeServer};

pub type EmbedResult<T> = Result<T, EmbedError>;

#[derive(Debug)]
pub struct BridgeRuntime {
    config: BridgeConfig,
}

#[derive(Debug)]
pub struct BridgeHandle {
    local_addr: SocketAddr,
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
        Self { config }
    }

    pub async fn start(self) -> EmbedResult<BridgeHandle> {
        let server = BridgeServer::bind(self.config).await?;
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
            shutdown: Some(shutdown_tx),
            join,
        })
    }
}

impl BridgeHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn shutdown(mut self) -> EmbedResult<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.join.await??;
        Ok(())
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

    #[tokio::test]
    async fn starts_and_shuts_down_embedded_bridge() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("bind addr"));
        let handle = BridgeRuntime::new(config).start().await.expect("runtime");

        assert!(handle.local_addr().port() > 0);
        handle.shutdown().await.expect("shutdown");
    }
}
