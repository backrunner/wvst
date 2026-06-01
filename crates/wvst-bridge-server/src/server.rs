use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::config::BridgeConfig;
use crate::control::{ControlContext, handle_control_text};
use crate::error::BridgeResult;
use crate::host_worker::HostWorkerClient;
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;

#[derive(Debug, Clone)]
struct BridgeState {
    config: Arc<BridgeConfig>,
    host_worker: Arc<HostWorkerClient>,
    metrics: Arc<BridgeMetrics>,
    plugins: Arc<PluginRegistry>,
}

pub struct BridgeServer {
    listener: TcpListener,
    state: BridgeState,
}

impl BridgeServer {
    pub async fn bind(config: BridgeConfig) -> BridgeResult<Self> {
        let listener = TcpListener::bind(config.bind_addr()).await?;

        Ok(Self {
            listener,
            state: BridgeState {
                config: Arc::new(config),
                host_worker: Arc::new(HostWorkerClient::from_env()),
                metrics: Arc::new(BridgeMetrics::new()),
                plugins: Arc::new(PluginRegistry::new()),
            },
        })
    }

    pub fn local_addr(&self) -> BridgeResult<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve(self) -> BridgeResult<()> {
        self.serve_until(std::future::pending::<()>()).await
    }

    pub async fn serve_until<F>(self, shutdown: F) -> BridgeResult<()>
    where
        F: Future<Output = ()>,
    {
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => return Ok(()),
                accepted = self.listener.accept() => {
                    let (stream, _) = accepted?;
                    let state = self.state.clone();

                    tokio::spawn(async move {
                        if let Err(error) = handle_connection(stream, state).await {
                            eprintln!("wvst bridge connection error: {error}");
                        }
                    });
                }
            }
        }
    }
}

#[allow(clippy::result_large_err)]
async fn handle_connection(stream: TcpStream, state: BridgeState) -> BridgeResult<()> {
    let origin = Arc::new(Mutex::new(None));
    let captured_origin = Arc::clone(&origin);
    let websocket = accept_hdr_async(stream, move |request: &Request, response: Response| {
        let request_origin = request
            .headers()
            .get("origin")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        if let Ok(mut origin) = captured_origin.lock() {
            *origin = request_origin;
        }

        Ok(response)
    })
    .await?;

    state.metrics.increment_websocket_connections();

    let origin = origin.lock().ok().and_then(|value| value.clone());
    let (mut sender, mut receiver) = websocket.split();
    let mut session_authorized = false;

    while let Some(message) = receiver.next().await {
        match message? {
            Message::Text(text) => {
                let response = handle_control_text(
                    text.as_ref(),
                    ControlContext {
                        config: &state.config,
                        host_worker: &state.host_worker,
                        metrics: &state.metrics,
                        plugins: &state.plugins,
                        origin: origin.as_deref(),
                        session_authorized,
                    },
                )
                .await;
                session_authorized = response.session_authorized;
                sender.send(Message::Text(response.text.into())).await?;
            }
            Message::Binary(payload) => {
                state.metrics.increment_binary_frames();
                sender.send(Message::Binary(payload)).await?;
            }
            Message::Ping(payload) => sender.send(Message::Pong(payload)).await?,
            Message::Pong(_) => {}
            Message::Close(frame) => {
                let _ = sender.send(Message::Close(frame)).await;
                return Ok(());
            }
            Message::Frame(_) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::sync::oneshot;
    use tokio_tungstenite::connect_async;

    #[tokio::test]
    async fn responds_to_hello_and_echoes_binary_frames() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("valid bind addr"));
        let server = BridgeServer::bind(config).await.expect("server binds");
        let addr = server.local_addr().expect("local addr");
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        tokio::spawn(async move {
            let _ = server
                .serve_until(async {
                    let _ = shutdown_receiver.await;
                })
                .await;
        });

        let (mut websocket, _) = connect_async(format!("ws://{addr}"))
            .await
            .expect("client connects");

        websocket
            .send(Message::Text(r#"{"id":1,"method":"bridge.hello","params":{"clientName":"test","clientVersion":"0.1.0","protocolMin":{"major":1,"minor":0},"protocolMax":{"major":1,"minor":0},"audioFrameVersion":1}}"#.into()))
            .await
            .expect("hello sends");

        let hello = websocket
            .next()
            .await
            .expect("hello response")
            .expect("valid websocket message");
        assert!(hello.to_text().expect("text").contains("wvst-bridge"));

        websocket
            .send(Message::Binary(vec![1, 2, 3].into()))
            .await
            .expect("binary sends");

        let echoed = websocket
            .next()
            .await
            .expect("echoed response")
            .expect("valid websocket message");
        assert_eq!(echoed.into_data(), vec![1, 2, 3]);

        let _ = shutdown_sender.send(());
    }
}
