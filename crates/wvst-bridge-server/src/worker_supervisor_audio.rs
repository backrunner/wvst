use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use wvst_protocol::{
    AudioFrameHeader, WORKER_AUDIO_IPC_HEADER_LEN, WorkerAudioIpcHeader, WorkerAudioIpcMessage,
    WorkerAudioMessageKind,
};

use super::{WorkerSupervisor, WorkerSupervisorError};

#[derive(Debug)]
pub(super) struct WorkerAudioConnection {
    stream: TcpStream,
}

impl WorkerAudioConnection {
    pub(super) fn new(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl WorkerSupervisor {
    pub async fn process_audio_frame(
        &self,
        instance_id: u64,
        frame: Vec<u8>,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let mut process_guard = process.lock().await;
        if process_guard.audio.is_none() {
            return Err(WorkerSupervisorError::AudioIpcUnavailable { instance_id });
        }

        let sequence = AudioFrameHeader::decode(&frame)
            .map_err(|error| process_guard.protocol_error(error.to_string()))?
            .sequence;
        let response = process_guard
            .audio
            .as_mut()
            .expect("checked audio connection")
            .process_frame(sequence, frame, self.timeout)
            .await;

        match response {
            Ok(frame) => Ok(frame),
            Err(error) => {
                let audit = process_guard.shutdown().await;
                self.record_shutdown(audit);
                drop(process_guard);
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}

impl WorkerAudioConnection {
    async fn process_frame(
        &mut self,
        sequence: u64,
        frame: Vec<u8>,
        timeout_duration: std::time::Duration,
    ) -> Result<Vec<u8>, WorkerSupervisorError> {
        let request = WorkerAudioIpcMessage::process_request(sequence, frame).map_err(|error| {
            WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            }
        })?;
        let bytes = request
            .encode()
            .map_err(|error| WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            })?;

        timeout(timeout_duration, self.stream.write_all(&bytes))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;
        timeout(timeout_duration, self.stream.flush())
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        let response = self.read_response(timeout_duration).await?;
        if response.header.sequence != sequence {
            return Err(WorkerSupervisorError::Protocol {
                message: format!(
                    "audio response sequence mismatch: expected {sequence}, got {}",
                    response.header.sequence
                ),
                stderr: String::new(),
            });
        }

        match response.header.kind {
            WorkerAudioMessageKind::ProcessResponse => Ok(response.body),
            WorkerAudioMessageKind::ProcessError => Err(WorkerSupervisorError::WorkerRejected {
                code: i64::from(response.header.status_code),
                message: String::from_utf8_lossy(&response.body).into_owned(),
                stderr: String::new(),
            }),
            WorkerAudioMessageKind::ProcessRequest => Err(WorkerSupervisorError::Protocol {
                message: "worker returned request on response path".to_string(),
                stderr: String::new(),
            }),
        }
    }

    async fn read_response(
        &mut self,
        timeout_duration: std::time::Duration,
    ) -> Result<WorkerAudioIpcMessage, WorkerSupervisorError> {
        let mut header_bytes = [0; WORKER_AUDIO_IPC_HEADER_LEN];
        timeout(timeout_duration, self.stream.read_exact(&mut header_bytes))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        let header = WorkerAudioIpcHeader::decode(&header_bytes).map_err(|error| {
            WorkerSupervisorError::Protocol {
                message: error.to_string(),
                stderr: String::new(),
            }
        })?;
        let mut body = vec![0; header.body_len as usize];
        timeout(timeout_duration, self.stream.read_exact(&mut body))
            .await
            .map_err(|_| WorkerSupervisorError::Timeout {
                method: "audio.processFrame",
                timeout_ms: timeout_duration.as_millis(),
                stderr: String::new(),
            })?
            .map_err(|error| WorkerSupervisorError::Io(error.to_string()))?;

        Ok(WorkerAudioIpcMessage { header, body })
    }
}
