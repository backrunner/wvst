use crate::ProtocolError;

pub const VST3_OUTPUT_EVENT_FIXED_FIELDS_LEN: usize = 40;
pub const VST3_OUTPUT_EVENT_PAYLOAD_BYTES: usize = 64;
pub const VST3_OUTPUT_EVENT_LEN: usize =
    VST3_OUTPUT_EVENT_FIXED_FIELDS_LEN + VST3_OUTPUT_EVENT_PAYLOAD_BYTES;
pub const VST3_OUTPUT_EVENT_PAYLOAD_FLAG_TRUNCATED: u8 = 1 << 0;
pub const VST3_OUTPUT_EVENT_PAYLOAD_FLAG_UNAVAILABLE: u8 = 1 << 1;
pub const VST3_OUTPUT_EVENT_PAYLOAD_FLAG_INVALID_TEXT: u8 = 1 << 2;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Vst3OutputEventKind {
    Data = 1,
    NoteExpressionValue = 2,
    NoteExpressionText = 3,
    Chord = 4,
    Scale = 5,
    NoteExpressionIntValue = 6,
}

impl TryFrom<u8> for Vst3OutputEventKind {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Data),
            2 => Ok(Self::NoteExpressionValue),
            3 => Ok(Self::NoteExpressionText),
            4 => Ok(Self::Chord),
            5 => Ok(Self::Scale),
            6 => Ok(Self::NoteExpressionIntValue),
            other => Err(ProtocolError::InvalidVst3OutputEventKind(other)),
        }
    }
}

impl From<Vst3OutputEventKind> for u8 {
    fn from(value: Vst3OutputEventKind) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Vst3OutputEventPayloadEncoding {
    None = 0,
    RawBytes = 1,
    Utf8 = 2,
}

impl TryFrom<u8> for Vst3OutputEventPayloadEncoding {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::RawBytes),
            2 => Ok(Self::Utf8),
            other => Err(ProtocolError::InvalidVst3OutputEventPayloadEncoding(other)),
        }
    }
}

impl From<Vst3OutputEventPayloadEncoding> for u8 {
    fn from(value: Vst3OutputEventPayloadEncoding) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vst3OutputEvent {
    pub sample_offset: u16,
    pub kind: Vst3OutputEventKind,
    pub vst3_event_type: u16,
    pub bus_index: i32,
    pub data1: i32,
    pub data2: i32,
    pub value: f64,
    pub data_size: u32,
    pub data_type: u32,
    pub payload_size: u16,
    pub payload_encoding: Vst3OutputEventPayloadEncoding,
    pub payload_flags: u8,
    pub payload: [u8; VST3_OUTPUT_EVENT_PAYLOAD_BYTES],
}

impl Vst3OutputEvent {
    pub fn payload_bytes(&self) -> &[u8] {
        let payload_size = usize::from(self.payload_size).min(VST3_OUTPUT_EVENT_PAYLOAD_BYTES);
        &self.payload[..payload_size]
    }

    pub fn encode(self, destination: &mut [u8]) -> Result<usize, ProtocolError> {
        if destination.len() < VST3_OUTPUT_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: VST3_OUTPUT_EVENT_LEN,
                actual: destination.len(),
            });
        }
        if !self.value.is_finite() {
            return Err(ProtocolError::InvalidVst3OutputEventValue(self.value));
        }
        if usize::from(self.payload_size) > VST3_OUTPUT_EVENT_PAYLOAD_BYTES {
            return Err(ProtocolError::InvalidVst3OutputEventPayloadLength(
                self.payload_size,
            ));
        }

        destination[..VST3_OUTPUT_EVENT_LEN].fill(0);
        destination[0..2].copy_from_slice(&self.sample_offset.to_le_bytes());
        destination[2] = self.kind.into();
        destination[4..6].copy_from_slice(&self.vst3_event_type.to_le_bytes());
        destination[8..12].copy_from_slice(&self.bus_index.to_le_bytes());
        destination[12..16].copy_from_slice(&self.data1.to_le_bytes());
        destination[16..20].copy_from_slice(&self.data2.to_le_bytes());
        destination[20..28].copy_from_slice(&self.value.to_le_bytes());
        destination[28..32].copy_from_slice(&self.data_size.to_le_bytes());
        destination[32..36].copy_from_slice(&self.data_type.to_le_bytes());
        destination[36..38].copy_from_slice(&self.payload_size.to_le_bytes());
        destination[38] = self.payload_encoding.into();
        destination[39] = self.payload_flags;
        let payload_size = usize::from(self.payload_size);
        destination
            [VST3_OUTPUT_EVENT_FIXED_FIELDS_LEN..VST3_OUTPUT_EVENT_FIXED_FIELDS_LEN + payload_size]
            .copy_from_slice(&self.payload[..payload_size]);

        Ok(VST3_OUTPUT_EVENT_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < VST3_OUTPUT_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: VST3_OUTPUT_EVENT_LEN,
                actual: source.len(),
            });
        }

        let event = Self {
            sample_offset: u16::from_le_bytes([source[0], source[1]]),
            kind: Vst3OutputEventKind::try_from(source[2])?,
            vst3_event_type: u16::from_le_bytes([source[4], source[5]]),
            bus_index: i32::from_le_bytes([source[8], source[9], source[10], source[11]]),
            data1: i32::from_le_bytes([source[12], source[13], source[14], source[15]]),
            data2: i32::from_le_bytes([source[16], source[17], source[18], source[19]]),
            value: f64::from_le_bytes([
                source[20], source[21], source[22], source[23], source[24], source[25], source[26],
                source[27],
            ]),
            data_size: u32::from_le_bytes([source[28], source[29], source[30], source[31]]),
            data_type: u32::from_le_bytes([source[32], source[33], source[34], source[35]]),
            payload_size: u16::from_le_bytes([source[36], source[37]]),
            payload_encoding: Vst3OutputEventPayloadEncoding::try_from(source[38])?,
            payload_flags: source[39],
            payload: {
                let mut payload = [0; VST3_OUTPUT_EVENT_PAYLOAD_BYTES];
                payload.copy_from_slice(
                    &source[VST3_OUTPUT_EVENT_FIXED_FIELDS_LEN..VST3_OUTPUT_EVENT_LEN],
                );
                payload
            },
        };
        if !event.value.is_finite() {
            return Err(ProtocolError::InvalidVst3OutputEventValue(event.value));
        }
        if usize::from(event.payload_size) > VST3_OUTPUT_EVENT_PAYLOAD_BYTES {
            return Err(ProtocolError::InvalidVst3OutputEventPayloadLength(
                event.payload_size,
            ));
        }

        Ok(event)
    }
}

impl Default for Vst3OutputEvent {
    fn default() -> Self {
        Self {
            sample_offset: 0,
            kind: Vst3OutputEventKind::Data,
            vst3_event_type: 0,
            bus_index: 0,
            data1: 0,
            data2: 0,
            value: 0.0,
            data_size: 0,
            data_type: 0,
            payload_size: 0,
            payload_encoding: Vst3OutputEventPayloadEncoding::None,
            payload_flags: 0,
            payload: [0; VST3_OUTPUT_EVENT_PAYLOAD_BYTES],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vst3_output_event_round_trips() {
        let event = Vst3OutputEvent {
            sample_offset: 12,
            kind: Vst3OutputEventKind::NoteExpressionIntValue,
            vst3_event_type: 8,
            bus_index: 1,
            data1: 10,
            data2: 123,
            value: 0.0,
            data_size: 0,
            data_type: 44,
            payload_size: 4,
            payload_encoding: Vst3OutputEventPayloadEncoding::RawBytes,
            payload_flags: VST3_OUTPUT_EVENT_PAYLOAD_FLAG_TRUNCATED,
            payload: {
                let mut payload = [0; VST3_OUTPUT_EVENT_PAYLOAD_BYTES];
                payload[..4].copy_from_slice(&[1, 2, 3, 4]);
                payload
            },
        };
        let mut bytes = [0; VST3_OUTPUT_EVENT_LEN];

        assert_eq!(event.encode(&mut bytes), Ok(VST3_OUTPUT_EVENT_LEN));
        assert_eq!(Vst3OutputEvent::decode(&bytes), Ok(event));
        assert_eq!(event.payload_bytes(), &[1, 2, 3, 4]);
    }
}
