use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use wvst_protocol::{
    WORKER_CONTROL_IPC_HEADER_LEN, WorkerControlIpcBatch, WorkerControlIpcBatchRole,
    WorkerControlIpcHeader, WorkerControlIpcMessage, WorkerControlMessageKind,
};

use super::{
    WorkerProcess, WorkerResponse, WorkerSupervisor, WorkerSupervisorError,
    validate_framed_response_body_len, validate_framed_response_header,
};

#[derive(Debug, Clone)]
pub(super) struct WorkerBatchRequest {
    pub method: &'static str,
    pub params: Value,
}

impl WorkerBatchRequest {
    pub(super) fn new(method: &'static str, params: Value) -> Self {
        Self { method, params }
    }
}

impl WorkerSupervisor {
    pub(super) async fn instance_batch_request(
        &self,
        instance_id: u64,
        requests: Vec<WorkerBatchRequest>,
    ) -> Result<Vec<Value>, WorkerSupervisorError> {
        let Some(process) = self.processes.lock().await.get(&instance_id).cloned() else {
            return Err(WorkerSupervisorError::WorkerMissing { instance_id });
        };

        let result = process
            .lock()
            .await
            .request_batch(&requests, self.timeout)
            .await;

        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                let audit = process.lock().await.shutdown().await;
                self.record_shutdown(audit);
                self.remove_process_if_same(instance_id, &process).await;
                Err(error)
            }
        }
    }
}

impl WorkerProcess {
    async fn request_batch(
        &mut self,
        requests: &[WorkerBatchRequest],
        timeout_duration: Duration,
    ) -> Result<Vec<Value>, WorkerSupervisorError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        if !self.use_framed_control_ipc {
            let mut results = Vec::with_capacity(requests.len());
            for request in requests {
                results.push(
                    self.request(request.method, request.params.clone(), timeout_duration)
                        .await?,
                );
            }
            return Ok(results);
        }

        let mut request_ids = Vec::with_capacity(requests.len());
        let mut child_frames = Vec::with_capacity(requests.len());
        for request in requests {
            let id = self.next_request_id;
            self.next_request_id = self.next_request_id.saturating_add(1);
            request_ids.push((id, request.method));
            let body = serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": request.method,
                "params": request.params,
            }))
            .map_err(|error| self.protocol_error(error.to_string()))?;
            child_frames.push(
                WorkerControlIpcMessage::request(id, body.into_bytes())
                    .map_err(|error| self.protocol_error(error.to_string()))?,
            );
        }

        let batch_sequence = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        let batch_frame = WorkerControlIpcBatch::request_frame(batch_sequence, child_frames)
            .map_err(|error| self.protocol_error(error.to_string()))?
            .encode()
            .map_err(|error| self.protocol_error(error.to_string()))?;
        self.write_control_frame(batch_frame, timeout_duration)
            .await?;
        let messages = self
            .read_batch_response_messages(batch_sequence, timeout_duration)
            .await?;

        if messages.len() != request_ids.len() {
            return Err(self.protocol_error(format!(
                "batch response count mismatch: expected {}, got {}",
                request_ids.len(),
                messages.len()
            )));
        }

        let mut results = Vec::with_capacity(messages.len());
        for ((id, method), message) in request_ids.into_iter().zip(messages) {
            validate_framed_response_header(&message.header, id, method)
                .map_err(|message| self.protocol_error(message))?;
            let body = String::from_utf8(message.body).map_err(|error| {
                self.protocol_error(format!("invalid framed batch response utf8: {error}"))
            })?;
            results.push(self.parse_response_body(id, method, &body).await?);
        }

        Ok(results)
    }

    async fn write_control_frame(
        &mut self,
        frame: Vec<u8>,
        timeout_duration: Duration,
    ) -> Result<(), WorkerSupervisorError> {
        match timeout(timeout_duration, self.stdin.write_all(&frame)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self
                    .timeout_error("control.batch.write", timeout_duration)
                    .await;
                return Err(error);
            }
        }
        match timeout(timeout_duration, self.stdin.flush()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self
                    .timeout_error("control.batch.flush", timeout_duration)
                    .await;
                return Err(error);
            }
        }

        Ok(())
    }

    async fn read_batch_response_messages(
        &mut self,
        batch_sequence: u64,
        timeout_duration: Duration,
    ) -> Result<Vec<WorkerControlIpcMessage>, WorkerSupervisorError> {
        let message = self
            .read_framed_message("control.batch", timeout_duration)
            .await?;
        validate_framed_batch_response_header(&message.header, batch_sequence)
            .map_err(|message| self.protocol_error(message))?;
        WorkerControlIpcBatch::decode_body(WorkerControlIpcBatchRole::Response, &message.body)
            .map(WorkerControlIpcBatch::into_messages)
            .map_err(|error| self.protocol_error(error.to_string()))
    }

    async fn read_framed_message(
        &mut self,
        method: &'static str,
        timeout_duration: Duration,
    ) -> Result<WorkerControlIpcMessage, WorkerSupervisorError> {
        let mut header_bytes = [0; WORKER_CONTROL_IPC_HEADER_LEN];
        match timeout(
            timeout_duration,
            self.stdout.get_mut().read_exact(&mut header_bytes),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                return Err(error);
            }
        }
        let header = WorkerControlIpcHeader::decode(&header_bytes)
            .map_err(|error| self.protocol_error(error.to_string()))?;
        validate_framed_response_body_len(header.body_len)
            .map_err(|message| self.protocol_error(message))?;
        let body_len = usize::try_from(header.body_len)
            .map_err(|_| self.protocol_error("framed response body too large".to_string()))?;
        let mut body = vec![0; body_len];
        match timeout(
            timeout_duration,
            self.stdout.get_mut().read_exact(&mut body),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(WorkerSupervisorError::Io(error.to_string())),
            Err(_) => {
                let error = self.timeout_error(method, timeout_duration).await;
                return Err(error);
            }
        }

        WorkerControlIpcMessage::new(header.kind, header.status_code, header.sequence, body)
            .map_err(|error| self.protocol_error(error.to_string()))
    }

    async fn parse_response_body(
        &self,
        id: u64,
        method: &'static str,
        body: &str,
    ) -> Result<Value, WorkerSupervisorError> {
        let response = serde_json::from_str::<WorkerResponse>(body)
            .map_err(|error| WorkerSupervisorError::InvalidJson(error.to_string()))?;

        if response.id != json!(id) {
            return Err(self.protocol_error(format!("response id mismatch for {method}")));
        }

        if let Some(error) = response.error {
            return Err(WorkerSupervisorError::WorkerRejected {
                code: error.code,
                message: error.message,
                data: error.data,
                stderr: self.stderr.snapshot().await,
            });
        }

        response
            .result
            .ok_or_else(|| self.protocol_error("missing result".to_string()))
    }
}

fn validate_framed_batch_response_header(
    header: &WorkerControlIpcHeader,
    sequence: u64,
) -> Result<(), String> {
    if header.sequence != sequence {
        return Err("framed batch response sequence mismatch".to_string());
    }
    match header.kind {
        WorkerControlMessageKind::BatchResponse if header.status_code == 0 => Ok(()),
        WorkerControlMessageKind::BatchResponse => Err(format!(
            "framed batch response used nonzero status {}",
            header.status_code
        )),
        other => Err(format!(
            "worker sent {other:?} frame while responding to control batch"
        )),
    }
}
