use crate::{
    ProtocolError, WORKER_CONTROL_IPC_HEADER_LEN, WorkerControlIpcHeader, WorkerControlIpcMessage,
    WorkerControlMessageKind,
};

pub const WORKER_CONTROL_IPC_MAX_BATCH_FRAMES: usize = 256;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WorkerControlIpcBatchRole {
    Request,
    Response,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkerControlIpcBatch {
    messages: Vec<WorkerControlIpcMessage>,
}

impl WorkerControlIpcBatch {
    pub fn new(
        role: WorkerControlIpcBatchRole,
        messages: Vec<WorkerControlIpcMessage>,
    ) -> Result<Self, ProtocolError> {
        validate_message_count(messages.len())?;
        for message in &messages {
            validate_child_kind(role, message.header.kind)?;
        }
        Ok(Self { messages })
    }

    pub fn messages(&self) -> &[WorkerControlIpcMessage] {
        &self.messages
    }

    pub fn into_messages(self) -> Vec<WorkerControlIpcMessage> {
        self.messages
    }

    pub fn request_frame(
        sequence: u64,
        messages: Vec<WorkerControlIpcMessage>,
    ) -> Result<WorkerControlIpcMessage, ProtocolError> {
        let batch = Self::new(WorkerControlIpcBatchRole::Request, messages)?;
        WorkerControlIpcMessage::new(
            WorkerControlMessageKind::BatchRequest,
            0,
            sequence,
            batch.encode_body()?,
        )
    }

    pub fn response_frame(
        sequence: u64,
        messages: Vec<WorkerControlIpcMessage>,
    ) -> Result<WorkerControlIpcMessage, ProtocolError> {
        let batch = Self::new(WorkerControlIpcBatchRole::Response, messages)?;
        WorkerControlIpcMessage::new(
            WorkerControlMessageKind::BatchResponse,
            0,
            sequence,
            batch.encode_body()?,
        )
    }

    pub fn decode_body(
        role: WorkerControlIpcBatchRole,
        body: &[u8],
    ) -> Result<Self, ProtocolError> {
        let mut offset = 0;
        let mut messages = Vec::new();

        while offset < body.len() {
            validate_message_count(messages.len() + 1)?;
            let header_end = offset.saturating_add(WORKER_CONTROL_IPC_HEADER_LEN);
            if header_end > body.len() {
                return Err(ProtocolError::BufferTooSmall {
                    min: header_end,
                    actual: body.len(),
                });
            }

            let header = WorkerControlIpcHeader::decode(&body[offset..header_end])?;
            validate_child_kind(role, header.kind)?;
            let body_len =
                usize::try_from(header.body_len).map_err(|_| ProtocolError::PayloadTooLarge)?;
            let body_end = header_end.saturating_add(body_len);
            if body_end > body.len() {
                return Err(ProtocolError::InvalidPayloadLength {
                    expected: header.body_len,
                    actual: u32::try_from(body.len().saturating_sub(header_end))
                        .unwrap_or(u32::MAX),
                });
            }

            messages.push(WorkerControlIpcMessage::new(
                header.kind,
                header.status_code,
                header.sequence,
                body[header_end..body_end].to_vec(),
            )?);
            offset = body_end;
        }

        Self::new(role, messages)
    }

    pub fn encode_body(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut body = Vec::new();
        for message in &self.messages {
            body.extend(message.encode()?);
        }
        Ok(body)
    }
}

fn validate_message_count(count: usize) -> Result<(), ProtocolError> {
    if count == 0 {
        return Err(ProtocolError::InvalidWorkerControlBatch(
            "worker control IPC batch cannot be empty".to_string(),
        ));
    }
    if count > WORKER_CONTROL_IPC_MAX_BATCH_FRAMES {
        return Err(ProtocolError::InvalidWorkerControlBatch(format!(
            "worker control IPC batch cannot exceed {WORKER_CONTROL_IPC_MAX_BATCH_FRAMES} frames"
        )));
    }
    Ok(())
}

fn validate_child_kind(
    role: WorkerControlIpcBatchRole,
    kind: WorkerControlMessageKind,
) -> Result<(), ProtocolError> {
    let valid = match role {
        WorkerControlIpcBatchRole::Request => kind == WorkerControlMessageKind::Request,
        WorkerControlIpcBatchRole::Response => {
            matches!(
                kind,
                WorkerControlMessageKind::Response | WorkerControlMessageKind::ErrorResponse
            )
        }
    };

    if valid {
        Ok(())
    } else {
        Err(ProtocolError::InvalidWorkerControlBatch(format!(
            "invalid worker control IPC batch child kind {kind:?} for {role:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_batch_round_trips_child_frames() {
        let first =
            WorkerControlIpcMessage::request(10, br#"{"id":1}"#.to_vec()).expect("first request");
        let second =
            WorkerControlIpcMessage::request(11, br#"{"id":2}"#.to_vec()).expect("second request");

        let frame = WorkerControlIpcBatch::request_frame(99, vec![first.clone(), second.clone()])
            .expect("batch frame");
        assert_eq!(frame.header.kind, WorkerControlMessageKind::BatchRequest);
        assert_eq!(frame.header.sequence, 99);

        let batch =
            WorkerControlIpcBatch::decode_body(WorkerControlIpcBatchRole::Request, &frame.body)
                .expect("decoded batch");
        assert_eq!(batch.messages(), &[first, second]);
    }

    #[test]
    fn response_batch_allows_error_children() {
        let ok = WorkerControlIpcMessage::response(10, br#"{"result":{}}"#.to_vec())
            .expect("ok response");
        let error = WorkerControlIpcMessage::error(11, 4220, br#"{"error":{}}"#.to_vec())
            .expect("error response");

        let frame = WorkerControlIpcBatch::response_frame(100, vec![ok.clone(), error.clone()])
            .expect("batch frame");
        assert_eq!(frame.header.kind, WorkerControlMessageKind::BatchResponse);

        let batch =
            WorkerControlIpcBatch::decode_body(WorkerControlIpcBatchRole::Response, &frame.body)
                .expect("decoded batch");
        assert_eq!(batch.into_messages(), vec![ok, error]);
    }

    #[test]
    fn rejects_invalid_child_kind_for_role() {
        let response = WorkerControlIpcMessage::response(10, Vec::new()).expect("response");

        assert!(matches!(
            WorkerControlIpcBatch::new(WorkerControlIpcBatchRole::Request, vec![response]),
            Err(ProtocolError::InvalidWorkerControlBatch(_))
        ));
    }
}
