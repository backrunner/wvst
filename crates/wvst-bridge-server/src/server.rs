use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::Message;
use wvst_core::ChannelCount;
use wvst_process_supervision::{LinuxCgroupLimits, WorkerResourceLimits};
use wvst_protocol::{AUDIO_FRAME_HEADER_LEN, AudioFrameFlags, AudioFrameHeader};

use crate::audio_in_flight::AudioInFlightLimiter;
use crate::audio_stream_tracker::AudioStreamTracker;
use crate::component_handler_events::ComponentHandlerEventPublisher;
use crate::config::BridgeConfig;
use crate::control::{ControlContext, handle_control_text};
use crate::error::BridgeResult;
use crate::events::{BridgeEvent, BridgeEventBus, BridgeEventKind, bridge_event_notification};
use crate::host_worker::HostWorkerClient;
use crate::instance_registry::{InstanceRecord, InstanceRegistry, InstanceState, StreamState};
use crate::metrics::{BridgeMetrics, BridgeMetricsHandle};
use crate::plugin_registry::PluginRegistry;
use crate::stream_shared_memory::SharedMemoryStreamRegistry;
use crate::stream_shared_memory_pump::SharedMemoryPumpRegistry;
use crate::worker_supervisor::{WorkerSupervisor, WorkerSupervisorOptions};

#[derive(Debug, Clone)]
struct BridgeState {
    config: Arc<BridgeConfig>,
    host_worker: Arc<HostWorkerClient>,
    instances: Arc<InstanceRegistry>,
    component_handler_events: Arc<ComponentHandlerEventPublisher>,
    events: BridgeEventBus,
    metrics: Arc<BridgeMetrics>,
    plugins: Arc<PluginRegistry>,
    stream_tracker: Arc<AudioStreamTracker>,
    audio_in_flight: Arc<AudioInFlightLimiter>,
    shared_memory: Arc<SharedMemoryStreamRegistry>,
    shared_memory_pumps: Arc<SharedMemoryPumpRegistry>,
    workers: Arc<WorkerSupervisor>,
}

pub struct BridgeServer {
    listener: TcpListener,
    state: BridgeState,
}

impl BridgeServer {
    pub async fn bind(config: BridgeConfig) -> BridgeResult<Self> {
        Self::bind_with_host_worker(config, HostWorkerClient::from_env()).await
    }

    pub async fn bind_with_host_worker(
        config: BridgeConfig,
        host_worker: HostWorkerClient,
    ) -> BridgeResult<Self> {
        Self::bind_with_host_worker_and_events(config, host_worker, BridgeEventBus::new()).await
    }

    pub async fn bind_with_host_worker_and_events(
        config: BridgeConfig,
        host_worker: HostWorkerClient,
        events: BridgeEventBus,
    ) -> BridgeResult<Self> {
        let listener = TcpListener::bind(config.bind_addr()).await?;

        events.emit(BridgeEventKind::ServerStarting);
        let metrics = Arc::new(BridgeMetrics::new());
        let resource_limits = worker_resource_limits(&config);
        let workers = WorkerSupervisor::with_options(
            WorkerSupervisorOptions::new(host_worker.executable_path().to_path_buf())
                .with_timeout(host_worker.timeout())
                .with_max_instances(config.max_worker_instances())
                .with_quarantine_failure_threshold(config.worker_quarantine_failure_threshold())
                .with_resource_limits(resource_limits)
                .with_metrics(Arc::clone(&metrics)),
        );
        if let Ok(local_addr) = listener.local_addr() {
            events.emit(BridgeEventKind::ServerStarted { local_addr });
        }

        Ok(Self {
            listener,
            state: BridgeState {
                config: Arc::new(config),
                host_worker: Arc::new(host_worker),
                instances: Arc::new(InstanceRegistry::new()),
                component_handler_events: Arc::new(ComponentHandlerEventPublisher::new()),
                events,
                metrics,
                plugins: Arc::new(PluginRegistry::new()),
                stream_tracker: Arc::new(AudioStreamTracker::new()),
                audio_in_flight: Arc::new(AudioInFlightLimiter::new()),
                shared_memory: Arc::new(SharedMemoryStreamRegistry::new()),
                shared_memory_pumps: Arc::new(SharedMemoryPumpRegistry::default()),
                workers: Arc::new(workers),
            },
        })
    }

    pub fn local_addr(&self) -> BridgeResult<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<BridgeEvent> {
        self.state.events.subscribe()
    }

    pub fn recent_events(&self, after_sequence: Option<u64>) -> Vec<BridgeEvent> {
        self.state.events.recent_since(after_sequence)
    }

    pub fn metrics_handle(&self) -> BridgeMetricsHandle {
        BridgeMetricsHandle::new(Arc::clone(&self.state.metrics))
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
                _ = &mut shutdown => {
                    self.state.events.emit(BridgeEventKind::ServerStopping);
                    self.state.events.emit(BridgeEventKind::ServerStopped);
                    return Ok(());
                },
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

fn worker_resource_limits(config: &BridgeConfig) -> WorkerResourceLimits {
    let mut limits = WorkerResourceLimits::none();
    if let Some(bytes) = config.worker_memory_limit_bytes() {
        limits = limits.with_address_space_bytes(bytes);
    }
    if let Some(seconds) = config.worker_cpu_time_limit_seconds() {
        limits = limits.with_cpu_time_seconds(seconds);
    }
    if let Some(parent) = config.worker_linux_cgroup_parent() {
        let mut cgroup = LinuxCgroupLimits::new(parent.to_path_buf());
        if let Some(bytes) = config.worker_linux_cgroup_memory_max_bytes() {
            cgroup = cgroup.with_memory_max_bytes(bytes);
        }
        if let Some((quota, period)) = config.worker_linux_cgroup_cpu_max_micros() {
            cgroup = cgroup.with_cpu_max_micros(quota, period);
        }
        if !cgroup.is_empty() {
            limits = limits.with_linux_cgroup(cgroup);
        }
    }
    limits
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
    let mut event_receiver = state.events.subscribe();

    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(message) = message else {
                    return Ok(());
                };
                let Some(authorized) =
                    handle_client_message(message?, &mut sender, &state, origin.as_deref(), session_authorized).await?
                else {
                    return Ok(());
                };
                if !session_authorized && authorized {
                    event_receiver = state.events.subscribe();
                }
                session_authorized = authorized;
            }
            event = event_receiver.recv(), if session_authorized => {
                match event {
                    Ok(event) => send_event_notification(&mut sender, event).await?,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
        }
    }
}

async fn handle_client_message<S>(
    message: Message,
    sender: &mut S,
    state: &BridgeState,
    origin: Option<&str>,
    session_authorized: bool,
) -> BridgeResult<Option<bool>>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    match message {
        Message::Text(text) => {
            if text.len() > state.config.max_control_message_bytes() {
                let response = control_message_too_large_response(
                    state.config.max_control_message_bytes(),
                    text.len(),
                );
                sender
                    .send(Message::Text(response.into()))
                    .await
                    .map_err(crate::error::BridgeError::from)?;
                return Ok(Some(session_authorized));
            }

            let response = handle_control_text(
                text.as_ref(),
                ControlContext {
                    config: &state.config,
                    host_worker: &state.host_worker,
                    instances: &state.instances,
                    component_handler_events: &state.component_handler_events,
                    events: &state.events,
                    metrics: &state.metrics,
                    plugins: &state.plugins,
                    stream_tracker: &state.stream_tracker,
                    audio_in_flight: &state.audio_in_flight,
                    shared_memory: &state.shared_memory,
                    shared_memory_pumps: &state.shared_memory_pumps,
                    origin,
                    session_authorized,
                    workers: &state.workers,
                },
            )
            .await;
            sender
                .send(Message::Text(response.text.into()))
                .await
                .map_err(crate::error::BridgeError::from)?;
            Ok(Some(response.session_authorized))
        }
        Message::Binary(payload) => {
            let response = process_binary_payload(payload.to_vec(), state).await;
            sender
                .send(Message::Binary(response.into()))
                .await
                .map_err(crate::error::BridgeError::from)?;
            Ok(Some(session_authorized))
        }
        Message::Ping(payload) => {
            sender
                .send(Message::Pong(payload))
                .await
                .map_err(crate::error::BridgeError::from)?;
            Ok(Some(session_authorized))
        }
        Message::Pong(_) | Message::Frame(_) => Ok(Some(session_authorized)),
        Message::Close(frame) => {
            let _ = sender.send(Message::Close(frame)).await;
            Ok(None)
        }
    }
}

fn control_message_too_large_response(max_bytes: usize, actual_bytes: usize) -> String {
    serialize_json(json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": {
            "code": 4130,
            "message": format!(
                "control message too large: max {max_bytes} bytes, got {actual_bytes}"
            ),
            "data": {
                "kind": "control-message-too-large",
                "maxBytes": max_bytes,
                "actualBytes": actual_bytes
            }
        }
    }))
}

async fn send_event_notification<S>(sender: &mut S, event: BridgeEvent) -> BridgeResult<()>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    sender
        .send(Message::Text(
            serialize_json(bridge_event_notification(event)).into(),
        ))
        .await
        .map_err(crate::error::BridgeError::from)?;
    Ok(())
}

fn serialize_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap_or_else(|_| {
        "{\"jsonrpc\":\"2.0\",\"method\":\"bridge.event\",\"params\":{\"event\":null}}".to_string()
    })
}

async fn process_binary_payload(payload: Vec<u8>, state: &BridgeState) -> Vec<u8> {
    state.metrics.increment_binary_frames();
    let started_at = Instant::now();

    let response = match route_audio_frame(&payload, state).await {
        AudioRouteResult::Routed(response) => {
            state.metrics.increment_audio_frames_routed();
            response
        }
        AudioRouteResult::Diagnostic(response) => {
            state.metrics.increment_audio_frame_route_failures();
            response
        }
        AudioRouteResult::Fallback => {
            state.metrics.increment_audio_frame_fallbacks();
            payload
        }
    };
    state
        .metrics
        .record_audio_route_latency_us(started_at.elapsed().as_micros() as u64);
    response
}

async fn route_audio_frame(payload: &[u8], state: &BridgeState) -> AudioRouteResult {
    let Ok(header) = AudioFrameHeader::decode(payload) else {
        return AudioRouteResult::Fallback;
    };
    let Some(expected_len) = AUDIO_FRAME_HEADER_LEN.checked_add(header.payload_len as usize) else {
        return AudioRouteResult::Fallback;
    };
    if payload.len() != expected_len {
        return AudioRouteResult::Fallback;
    }

    let Some(instance) = state.instances.find_by_stream_id(header.stream_id.get()) else {
        return AudioRouteResult::Fallback;
    };
    if instance.stream_state != StreamState::Open {
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::END_OF_STREAM,
        );
    }
    if instance.state != InstanceState::Processing {
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::PROCESS_ERROR,
        );
    }

    state
        .metrics
        .record_audio_stream_observation(state.stream_tracker.observe(header));
    let Some(_in_flight) = state.audio_in_flight.try_acquire(header.stream_id.get()) else {
        state.metrics.increment_audio_backpressure_drops();
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::LATE,
        );
    };

    let processed = match state
        .workers
        .process_audio_frame(instance.instance_id, payload)
        .await
    {
        Ok(processed) => processed,
        Err(error) => {
            state.metrics.increment_worker_failures();
            let _ = state.instances.mark_worker_failed(instance.instance_id);
            state.events.emit(BridgeEventKind::WorkerFailed {
                instance_id: instance.instance_id,
                plugin_id: instance.plugin_id.clone(),
                code: error.rpc_code(),
                message: error.rpc_message(),
                error_data: Some(error.rpc_data()),
            });
            return diagnostic_silence_frame(
                header,
                &instance,
                AudioFrameFlags::SILENCE | AudioFrameFlags::PROCESS_ERROR,
            );
        }
    };

    let Ok(processed_header) = AudioFrameHeader::decode(&processed) else {
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::PROCESS_ERROR,
        );
    };
    let Some(processed_len) =
        AUDIO_FRAME_HEADER_LEN.checked_add(processed_header.payload_len as usize)
    else {
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::PROCESS_ERROR,
        );
    };
    if processed.len() != processed_len || processed_header.stream_id != header.stream_id {
        return diagnostic_silence_frame(
            header,
            &instance,
            AudioFrameFlags::SILENCE | AudioFrameFlags::PROCESS_ERROR,
        );
    }

    AudioRouteResult::Routed(processed)
}

fn diagnostic_silence_frame(
    input_header: AudioFrameHeader,
    instance: &InstanceRecord,
    flags: AudioFrameFlags,
) -> AudioRouteResult {
    match encode_silence_frame(input_header, instance.output_channels, flags) {
        Ok(frame) => AudioRouteResult::Diagnostic(frame),
        Err(_) => AudioRouteResult::Fallback,
    }
}

fn encode_silence_frame(
    input_header: AudioFrameHeader,
    output_channels: u16,
    flags: AudioFrameFlags,
) -> Result<Vec<u8>, String> {
    let channels = ChannelCount::new(output_channels).map_err(|error| error.to_string())?;
    let header = AudioFrameHeader::new_f32(
        input_header.stream_id,
        input_header.sequence,
        input_header.sent_frame_time,
        input_header.sample_rate,
        input_header.frames,
        channels,
        input_header.flags | flags,
    )
    .map_err(|error| error.to_string())?;
    let mut frame = vec![0; AUDIO_FRAME_HEADER_LEN + header.payload_len as usize];
    header
        .encode(&mut frame[..AUDIO_FRAME_HEADER_LEN])
        .map_err(|error| error.to_string())?;

    Ok(frame)
}

enum AudioRouteResult {
    Routed(Vec<u8>),
    Diagnostic(Vec<u8>),
    Fallback,
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "server_audio_failure_tests.rs"]
mod audio_failure_tests;
