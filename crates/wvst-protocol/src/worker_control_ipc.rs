use crate::ProtocolError;

pub const WORKER_CONTROL_IPC_MAGIC: u32 = u32::from_le_bytes(*b"WVCI");
pub const WORKER_CONTROL_IPC_VERSION: u16 = 1;
pub const WORKER_CONTROL_IPC_HEADER_LEN: usize = 24;
pub const WORKER_CONTROL_IPC_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u16)]
pub enum WorkerControlMessageKind {
    Request = 1,
    Response = 2,
    ErrorResponse = 3,
}

impl TryFrom<u16> for WorkerControlMessageKind {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Request),
            2 => Ok(Self::Response),
            3 => Ok(Self::ErrorResponse),
            other => Err(ProtocolError::InvalidWorkerControlMessageKind(other)),
        }
    }
}

impl From<WorkerControlMessageKind> for u16 {
    fn from(value: WorkerControlMessageKind) -> Self {
        value as u16
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkerControlIpcHeader {
    pub kind: WorkerControlMessageKind,
    pub status_code: u16,
    pub sequence: u64,
    pub body_len: u32,
}

impl WorkerControlIpcHeader {
    pub fn new(
        kind: WorkerControlMessageKind,
        status_code: u16,
        sequence: u64,
        body_len: u32,
    ) -> Self {
        Self {
            kind,
            status_code,
            sequence,
            body_len,
        }
    }

    pub fn encode(self, destination: &mut [u8]) -> Result<usize, ProtocolError> {
        if destination.len() < WORKER_CONTROL_IPC_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: WORKER_CONTROL_IPC_HEADER_LEN,
                actual: destination.len(),
            });
        }

        destination[..WORKER_CONTROL_IPC_HEADER_LEN].fill(0);
        put_u32(destination, 0, WORKER_CONTROL_IPC_MAGIC);
        put_u16(destination, 4, WORKER_CONTROL_IPC_VERSION);
        put_u16(destination, 6, WORKER_CONTROL_IPC_HEADER_LEN as u16);
        put_u16(destination, 8, self.kind.into());
        put_u16(destination, 10, self.status_code);
        put_u32(destination, 12, self.body_len);
        put_u64(destination, 16, self.sequence);

        Ok(WORKER_CONTROL_IPC_HEADER_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < WORKER_CONTROL_IPC_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: WORKER_CONTROL_IPC_HEADER_LEN,
                actual: source.len(),
            });
        }

        let magic = read_u32(source, 0);
        if magic != WORKER_CONTROL_IPC_MAGIC {
            return Err(ProtocolError::InvalidWorkerControlIpcMagic(magic));
        }

        let version = read_u16(source, 4);
        if version != WORKER_CONTROL_IPC_VERSION {
            return Err(ProtocolError::UnsupportedWorkerControlIpcVersion(version));
        }

        let header_len = read_u16(source, 6);
        if usize::from(header_len) != WORKER_CONTROL_IPC_HEADER_LEN {
            return Err(ProtocolError::InvalidHeaderLength(header_len));
        }

        Ok(Self {
            kind: WorkerControlMessageKind::try_from(read_u16(source, 8))?,
            status_code: read_u16(source, 10),
            body_len: read_u32(source, 12),
            sequence: read_u64(source, 16),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkerControlIpcMessage {
    pub header: WorkerControlIpcHeader,
    pub body: Vec<u8>,
}

impl WorkerControlIpcMessage {
    pub fn request(sequence: u64, body: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::new(WorkerControlMessageKind::Request, 0, sequence, body)
    }

    pub fn response(sequence: u64, body: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::new(WorkerControlMessageKind::Response, 0, sequence, body)
    }

    pub fn error(sequence: u64, status_code: u16, body: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::new(
            WorkerControlMessageKind::ErrorResponse,
            status_code,
            sequence,
            body,
        )
    }

    pub fn new(
        kind: WorkerControlMessageKind,
        status_code: u16,
        sequence: u64,
        body: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        let body_len = u32::try_from(body.len()).map_err(|_| ProtocolError::PayloadTooLarge)?;

        Ok(Self {
            header: WorkerControlIpcHeader::new(kind, status_code, sequence, body_len),
            body,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut frame = vec![0; WORKER_CONTROL_IPC_HEADER_LEN + self.body.len()];
        self.header
            .encode(&mut frame[..WORKER_CONTROL_IPC_HEADER_LEN])?;
        frame[WORKER_CONTROL_IPC_HEADER_LEN..].copy_from_slice(&self.body);
        Ok(frame)
    }
}

fn put_u16(destination: &mut [u8], offset: usize, value: u16) {
    destination[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(destination: &mut [u8], offset: usize, value: u32) {
    destination[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(destination: &mut [u8], offset: usize, value: u64) {
    destination[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(read_array(source, offset))
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array(source, offset))
}

fn read_u64(source: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(read_array(source, offset))
}

fn read_array<const N: usize>(source: &[u8], offset: usize) -> [u8; N] {
    let mut output = [0; N];
    output.copy_from_slice(&source[offset..offset + N]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_control_ipc_message_round_trips() {
        let message =
            WorkerControlIpcMessage::request(42, br#"{"method":"worker.hello"}"#.to_vec())
                .expect("message");
        let bytes = message.encode().expect("encoded");
        let header = WorkerControlIpcHeader::decode(&bytes).expect("header");

        assert_eq!(header.kind, WorkerControlMessageKind::Request);
        assert_eq!(header.sequence, 42);
        assert_eq!(header.body_len, 25);
        assert_eq!(
            &bytes[WORKER_CONTROL_IPC_HEADER_LEN..],
            br#"{"method":"worker.hello"}"#
        );
    }

    #[test]
    fn rejects_invalid_control_message_kind() {
        let message = WorkerControlIpcMessage::response(1, Vec::new()).expect("message");
        let mut bytes = message.encode().expect("encoded");
        put_u16(&mut bytes, 8, 99);

        assert!(matches!(
            WorkerControlIpcHeader::decode(&bytes),
            Err(ProtocolError::InvalidWorkerControlMessageKind(99))
        ));
    }
}
