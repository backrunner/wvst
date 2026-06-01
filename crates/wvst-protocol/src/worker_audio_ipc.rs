use crate::ProtocolError;

pub const WORKER_AUDIO_IPC_MAGIC: u32 = u32::from_le_bytes(*b"WVAI");
pub const WORKER_AUDIO_IPC_VERSION: u16 = 1;
pub const WORKER_AUDIO_IPC_HEADER_LEN: usize = 24;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u16)]
pub enum WorkerAudioMessageKind {
    ProcessRequest = 1,
    ProcessResponse = 2,
    ProcessError = 3,
}

impl TryFrom<u16> for WorkerAudioMessageKind {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ProcessRequest),
            2 => Ok(Self::ProcessResponse),
            3 => Ok(Self::ProcessError),
            other => Err(ProtocolError::InvalidWorkerAudioMessageKind(other)),
        }
    }
}

impl From<WorkerAudioMessageKind> for u16 {
    fn from(value: WorkerAudioMessageKind) -> Self {
        value as u16
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkerAudioIpcHeader {
    pub kind: WorkerAudioMessageKind,
    pub status_code: u16,
    pub sequence: u64,
    pub body_len: u32,
}

impl WorkerAudioIpcHeader {
    pub fn new(
        kind: WorkerAudioMessageKind,
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
        if destination.len() < WORKER_AUDIO_IPC_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: WORKER_AUDIO_IPC_HEADER_LEN,
                actual: destination.len(),
            });
        }

        destination[..WORKER_AUDIO_IPC_HEADER_LEN].fill(0);
        put_u32(destination, 0, WORKER_AUDIO_IPC_MAGIC);
        put_u16(destination, 4, WORKER_AUDIO_IPC_VERSION);
        put_u16(destination, 6, WORKER_AUDIO_IPC_HEADER_LEN as u16);
        put_u16(destination, 8, self.kind.into());
        put_u16(destination, 10, self.status_code);
        put_u32(destination, 12, self.body_len);
        put_u64(destination, 16, self.sequence);

        Ok(WORKER_AUDIO_IPC_HEADER_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < WORKER_AUDIO_IPC_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: WORKER_AUDIO_IPC_HEADER_LEN,
                actual: source.len(),
            });
        }

        let magic = read_u32(source, 0);
        if magic != WORKER_AUDIO_IPC_MAGIC {
            return Err(ProtocolError::InvalidWorkerAudioIpcMagic(magic));
        }

        let version = read_u16(source, 4);
        if version != WORKER_AUDIO_IPC_VERSION {
            return Err(ProtocolError::UnsupportedWorkerAudioIpcVersion(version));
        }

        let header_len = read_u16(source, 6);
        if usize::from(header_len) != WORKER_AUDIO_IPC_HEADER_LEN {
            return Err(ProtocolError::InvalidHeaderLength(header_len));
        }

        Ok(Self {
            kind: WorkerAudioMessageKind::try_from(read_u16(source, 8))?,
            status_code: read_u16(source, 10),
            body_len: read_u32(source, 12),
            sequence: read_u64(source, 16),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkerAudioIpcMessage {
    pub header: WorkerAudioIpcHeader,
    pub body: Vec<u8>,
}

impl WorkerAudioIpcMessage {
    pub fn process_request(sequence: u64, body: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::new(WorkerAudioMessageKind::ProcessRequest, 0, sequence, body)
    }

    pub fn process_response(sequence: u64, body: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::new(WorkerAudioMessageKind::ProcessResponse, 0, sequence, body)
    }

    pub fn error(sequence: u64, status_code: u16, message: String) -> Result<Self, ProtocolError> {
        Self::new(
            WorkerAudioMessageKind::ProcessError,
            status_code,
            sequence,
            message.into_bytes(),
        )
    }

    pub fn new(
        kind: WorkerAudioMessageKind,
        status_code: u16,
        sequence: u64,
        body: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        let body_len = u32::try_from(body.len()).map_err(|_| ProtocolError::PayloadTooLarge)?;

        Ok(Self {
            header: WorkerAudioIpcHeader::new(kind, status_code, sequence, body_len),
            body,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut frame = vec![0; WORKER_AUDIO_IPC_HEADER_LEN + self.body.len()];
        self.header
            .encode(&mut frame[..WORKER_AUDIO_IPC_HEADER_LEN])?;
        frame[WORKER_AUDIO_IPC_HEADER_LEN..].copy_from_slice(&self.body);
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
    fn worker_audio_ipc_message_round_trips() {
        let message =
            WorkerAudioIpcMessage::process_request(42, vec![1, 2, 3, 4]).expect("message");
        let bytes = message.encode().expect("encoded");
        let header = WorkerAudioIpcHeader::decode(&bytes).expect("header");

        assert_eq!(header.kind, WorkerAudioMessageKind::ProcessRequest);
        assert_eq!(header.sequence, 42);
        assert_eq!(header.body_len, 4);
        assert_eq!(&bytes[WORKER_AUDIO_IPC_HEADER_LEN..], &[1, 2, 3, 4]);
    }

    #[test]
    fn rejects_invalid_message_kind() {
        let message = WorkerAudioIpcMessage::process_request(1, Vec::new()).expect("message");
        let mut bytes = message.encode().expect("encoded");
        put_u16(&mut bytes, 8, 99);

        assert!(matches!(
            WorkerAudioIpcHeader::decode(&bytes),
            Err(ProtocolError::InvalidWorkerAudioMessageKind(99))
        ));
    }
}
