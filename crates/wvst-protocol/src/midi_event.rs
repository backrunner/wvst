use crate::ProtocolError;

pub const MIDI_EVENT_LEN: usize = 16;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum MidiEventKind {
    NoteOn = 1,
    NoteOff = 2,
    ControlChange = 3,
    PitchBend = 4,
    ChannelAftertouch = 5,
    PolyAftertouch = 6,
    RawMidi = 255,
}

impl TryFrom<u8> for MidiEventKind {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::NoteOn),
            2 => Ok(Self::NoteOff),
            3 => Ok(Self::ControlChange),
            4 => Ok(Self::PitchBend),
            5 => Ok(Self::ChannelAftertouch),
            6 => Ok(Self::PolyAftertouch),
            255 => Ok(Self::RawMidi),
            other => Err(ProtocolError::InvalidMidiEventKind(other)),
        }
    }
}

impl From<MidiEventKind> for u8 {
    fn from(value: MidiEventKind) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MidiEvent {
    pub sample_offset: u16,
    pub kind: MidiEventKind,
    pub channel: u8,
    pub data1: u8,
    pub data2: u8,
    pub data3: u8,
    pub data_len: u8,
    pub note_id: u32,
}

impl MidiEvent {
    pub fn new(
        sample_offset: u16,
        kind: MidiEventKind,
        channel: u8,
        data1: u8,
        data2: u8,
    ) -> Result<Self, ProtocolError> {
        validate_channel(channel)?;
        validate_data7("data1", data1)?;
        validate_data7("data2", data2)?;

        Ok(Self {
            sample_offset,
            kind,
            channel,
            data1,
            data2,
            data3: 0,
            data_len: 2,
            note_id: 0,
        })
    }

    pub fn raw_midi(
        sample_offset: u16,
        status: u8,
        data1: u8,
        data2: u8,
        data_len: u8,
    ) -> Result<Self, ProtocolError> {
        if !(1..=3).contains(&data_len) {
            return Err(ProtocolError::InvalidMidiData {
                field: "data_len",
                value: data_len,
            });
        }

        Ok(Self {
            sample_offset,
            kind: MidiEventKind::RawMidi,
            channel: status & 0x0f,
            data1: status,
            data2: data1,
            data3: data2,
            data_len,
            note_id: 0,
        })
    }

    pub fn encode(self, destination: &mut [u8]) -> Result<usize, ProtocolError> {
        if destination.len() < MIDI_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: MIDI_EVENT_LEN,
                actual: destination.len(),
            });
        }

        destination[..MIDI_EVENT_LEN].fill(0);
        destination[0..2].copy_from_slice(&self.sample_offset.to_le_bytes());
        destination[2] = self.kind.into();
        destination[3] = self.channel;
        destination[4] = self.data1;
        destination[5] = self.data2;
        destination[6] = self.data3;
        destination[7] = self.data_len;
        destination[8..12].copy_from_slice(&self.note_id.to_le_bytes());

        Ok(MIDI_EVENT_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < MIDI_EVENT_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: MIDI_EVENT_LEN,
                actual: source.len(),
            });
        }

        let event = Self {
            sample_offset: u16::from_le_bytes([source[0], source[1]]),
            kind: MidiEventKind::try_from(source[2])?,
            channel: source[3],
            data1: source[4],
            data2: source[5],
            data3: source[6],
            data_len: source[7],
            note_id: u32::from_le_bytes([source[8], source[9], source[10], source[11]]),
        };
        event.validate()?;
        Ok(event)
    }

    fn validate(self) -> Result<(), ProtocolError> {
        validate_channel(self.channel)?;
        if self.kind == MidiEventKind::RawMidi {
            if !(1..=3).contains(&self.data_len) {
                return Err(ProtocolError::InvalidMidiData {
                    field: "data_len",
                    value: self.data_len,
                });
            }
            return Ok(());
        }

        validate_data7("data1", self.data1)?;
        validate_data7("data2", self.data2)?;
        validate_data7("data3", self.data3)
    }
}

fn validate_channel(channel: u8) -> Result<(), ProtocolError> {
    if channel > 15 {
        return Err(ProtocolError::InvalidMidiChannel(channel));
    }

    Ok(())
}

fn validate_data7(field: &'static str, value: u8) -> Result<(), ProtocolError> {
    if value > 127 {
        return Err(ProtocolError::InvalidMidiData { field, value });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_event_round_trips() {
        let event =
            MidiEvent::new(12, MidiEventKind::NoteOn, 1, 60, 100).expect("valid midi event");
        let mut bytes = [0; MIDI_EVENT_LEN];

        assert_eq!(event.encode(&mut bytes), Ok(MIDI_EVENT_LEN));
        assert_eq!(MidiEvent::decode(&bytes), Ok(event));
    }

    #[test]
    fn raw_midi_event_round_trips() {
        let event = MidiEvent::raw_midi(4, 0x90, 60, 100, 3).expect("valid raw midi");
        let mut bytes = [0; MIDI_EVENT_LEN];

        event.encode(&mut bytes).expect("event encodes");
        assert_eq!(MidiEvent::decode(&bytes), Ok(event));
    }

    #[test]
    fn rejects_invalid_channel() {
        assert!(matches!(
            MidiEvent::new(0, MidiEventKind::NoteOn, 16, 60, 100),
            Err(ProtocolError::InvalidMidiChannel(16))
        ));
    }
}
