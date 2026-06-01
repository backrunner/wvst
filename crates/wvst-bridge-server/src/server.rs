use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::Message;
use wvst_protocol::{AUDIO_FRAME_HEADER_LEN, AudioFrameHeader};

use crate::config::BridgeConfig;
use crate::control::{ControlContext, handle_control_text};
use crate::error::BridgeResult;
use crate::host_worker::HostWorkerClient;
use crate::instance_registry::InstanceRegistry;
use crate::metrics::BridgeMetrics;
use crate::plugin_registry::PluginRegistry;
use crate::worker_supervisor::WorkerSupervisor;

#[derive(Debug, Clone)]
struct BridgeState {
    config: Arc<BridgeConfig>,
    host_worker: Arc<HostWorkerClient>,
    instances: Arc<InstanceRegistry>,
    metrics: Arc<BridgeMetrics>,
    plugins: Arc<PluginRegistry>,
    workers: Arc<WorkerSupervisor>,
}

pub struct BridgeServer {
    listener: TcpListener,
    state: BridgeState,
}

impl BridgeServer {
    pub async fn bind(config: BridgeConfig) -> BridgeResult<Self> {
        let listener = TcpListener::bind(config.bind_addr()).await?;

        let host_worker = HostWorkerClient::from_env();
        let workers = WorkerSupervisor::new(host_worker.executable_path().to_path_buf());

        Ok(Self {
            listener,
            state: BridgeState {
                config: Arc::new(config),
                host_worker: Arc::new(host_worker),
                instances: Arc::new(InstanceRegistry::new()),
                metrics: Arc::new(BridgeMetrics::new()),
                plugins: Arc::new(PluginRegistry::new()),
                workers: Arc::new(workers),
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
                        instances: &state.instances,
                        metrics: &state.metrics,
                        plugins: &state.plugins,
                        origin: origin.as_deref(),
                        session_authorized,
                        workers: &state.workers,
                    },
                )
                .await;
                session_authorized = response.session_authorized;
                sender.send(Message::Text(response.text.into())).await?;
            }
            Message::Binary(payload) => {
                let response = process_binary_payload(payload.to_vec(), &state).await;
                sender.send(Message::Binary(response.into())).await?;
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

async fn process_binary_payload(payload: Vec<u8>, state: &BridgeState) -> Vec<u8> {
    state.metrics.increment_binary_frames();

    match route_audio_frame(&payload, state).await {
        Some(response) => {
            state.metrics.increment_audio_frames_routed();
            response
        }
        None => {
            state.metrics.increment_audio_frame_fallbacks();
            payload
        }
    }
}

async fn route_audio_frame(payload: &[u8], state: &BridgeState) -> Option<Vec<u8>> {
    let header = AudioFrameHeader::decode(payload).ok()?;
    let expected_len = AUDIO_FRAME_HEADER_LEN.checked_add(header.payload_len as usize)?;
    if payload.len() != expected_len || header.event_count != 0 {
        return None;
    }

    let instance = state
        .instances
        .find_open_by_stream_id(header.stream_id.get())?;
    let processed = match state
        .workers
        .process_audio_frame(instance.instance_id, payload.to_vec())
        .await
    {
        Ok(processed) => processed,
        Err(_) => {
            state.metrics.increment_worker_failures();
            state.metrics.increment_audio_frame_route_failures();
            let _ = state.instances.mark_worker_failed(instance.instance_id);
            return None;
        }
    };

    let processed_header = AudioFrameHeader::decode(&processed).ok()?;
    let processed_len =
        AUDIO_FRAME_HEADER_LEN.checked_add(processed_header.payload_len as usize)?;
    if processed.len() != processed_len || processed_header.stream_id != header.stream_id {
        state.metrics.increment_audio_frame_route_failures();
        return None;
    }

    Some(processed)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
