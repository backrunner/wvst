use std::ops::{BitOr, BitOrAssign};

use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId, f32_payload_len};

use crate::ProtocolError;

pub const AUDIO_FRAME_MAGIC: u32 = u32::from_le_bytes(*b"WVST");
pub const AUDIO_FRAME_VERSION: u16 = 1;
pub const AUDIO_FRAME_HEADER_LEN: usize = 56;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum AudioSampleFormat {
    F32Le = 1,
}

impl TryFrom<u8> for AudioSampleFormat {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::F32Le),
            other => Err(ProtocolError::InvalidSampleFormat(other)),
        }
    }
}

impl From<AudioSampleFormat> for u8 {
    fn from(value: AudioSampleFormat) -> Self {
        value as u8
    }
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct AudioFrameFlags(u16);

impl AudioFrameFlags {
    pub const SILENCE: Self = Self(1 << 0);
    pub const MIDI_ONLY: Self = Self(1 << 1);
    pub const END_OF_STREAM: Self = Self(1 << 2);
    pub const LATE: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }
}

impl BitOr for AudioFrameFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for AudioFrameFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AudioFrameHeader {
    pub stream_id: StreamId,
    pub sequence: u64,
    pub sent_frame_time: u64,
    pub sample_rate: SampleRate,
    pub frames: FrameCount,
    pub channels: ChannelCount,
    pub format: AudioSampleFormat,
    pub flags: AudioFrameFlags,
    pub event_count: u16,
    pub payload_len: u32,
}

impl AudioFrameHeader {
    pub fn new_f32(
        stream_id: StreamId,
        sequence: u64,
        sent_frame_time: u64,
        sample_rate: SampleRate,
        frames: FrameCount,
        channels: ChannelCount,
        flags: AudioFrameFlags,
    ) -> Result<Self, ProtocolError> {
        let payload_len = f32_payload_len(frames, channels)?;

        Ok(Self {
            stream_id,
            sequence,
            sent_frame_time,
            sample_rate,
            frames,
            channels,
            format: AudioSampleFormat::F32Le,
            flags,
            event_count: 0,
            payload_len,
        })
    }

    pub fn encode(self, destination: &mut [u8]) -> Result<usize, ProtocolError> {
        if destination.len() < AUDIO_FRAME_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: AUDIO_FRAME_HEADER_LEN,
                actual: destination.len(),
            });
        }

        destination[..AUDIO_FRAME_HEADER_LEN].fill(0);
        put_u32(destination, 0, AUDIO_FRAME_MAGIC);
        put_u16(destination, 4, AUDIO_FRAME_VERSION);
        put_u16(destination, 6, AUDIO_FRAME_HEADER_LEN as u16);
        put_u64(destination, 8, self.stream_id.get());
        put_u64(destination, 16, self.sequence);
        put_u64(destination, 24, self.sent_frame_time);
        put_u32(destination, 32, self.sample_rate.get());
        put_u32(destination, 36, self.payload_len);
        put_u16(destination, 40, self.frames.get());
        put_u16(destination, 42, self.channels.get());
        destination[44] = self.format.into();
        destination[45] = 0;
        put_u16(destination, 46, self.flags.bits());
        put_u16(destination, 48, self.event_count);

        Ok(AUDIO_FRAME_HEADER_LEN)
    }

    pub fn decode(source: &[u8]) -> Result<Self, ProtocolError> {
        if source.len() < AUDIO_FRAME_HEADER_LEN {
            return Err(ProtocolError::BufferTooSmall {
                min: AUDIO_FRAME_HEADER_LEN,
                actual: source.len(),
            });
        }

        let magic = read_u32(source, 0);
        if magic != AUDIO_FRAME_MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        let version = read_u16(source, 4);
        if version != AUDIO_FRAME_VERSION {
            return Err(ProtocolError::UnsupportedAudioFrameVersion(version));
        }

        let header_len = read_u16(source, 6);
        if usize::from(header_len) != AUDIO_FRAME_HEADER_LEN {
            return Err(ProtocolError::InvalidHeaderLength(header_len));
        }

        let sample_rate = SampleRate::new(read_u32(source, 32))?;
        let payload_len = read_u32(source, 36);
        let frames = FrameCount::new(read_u16(source, 40))?;
        let channels = ChannelCount::new(read_u16(source, 42))?;
        let format = AudioSampleFormat::try_from(source[44])?;
        let expected_payload_len = f32_payload_len(frames, channels)?;

        if payload_len != expected_payload_len {
            return Err(ProtocolError::InvalidPayloadLength {
                expected: expected_payload_len,
                actual: payload_len,
            });
        }

        Ok(Self {
            stream_id: StreamId::new(read_u64(source, 8)),
            sequence: read_u64(source, 16),
            sent_frame_time: read_u64(source, 24),
            sample_rate,
            frames,
            channels,
            format,
            flags: AudioFrameFlags::from_bits(read_u16(source, 46)),
            event_count: read_u16(source, 48),
            payload_len,
        })
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

    fn test_header() -> AudioFrameHeader {
        AudioFrameHeader::new_f32(
            StreamId::new(42),
            7,
            1_024,
            SampleRate::new(48_000).expect("valid sample rate"),
            FrameCount::new(64).expect("valid frame count"),
            ChannelCount::new(2).expect("valid channel count"),
            AudioFrameFlags::SILENCE,
        )
        .expect("valid header")
    }

    #[test]
    fn audio_frame_header_round_trips() {
        let header = test_header();
        let mut bytes = [0; AUDIO_FRAME_HEADER_LEN];

        assert_eq!(header.encode(&mut bytes), Ok(AUDIO_FRAME_HEADER_LEN));
        assert_eq!(AudioFrameHeader::decode(&bytes), Ok(header));
    }

    #[test]
    fn rejects_short_buffers() {
        let mut short = [0; AUDIO_FRAME_HEADER_LEN - 1];

        assert!(test_header().encode(&mut short).is_err());
        assert!(AudioFrameHeader::decode(&short).is_err());
    }

    #[test]
    fn rejects_invalid_payload_len() {
        let header = test_header();
        let mut bytes = [0; AUDIO_FRAME_HEADER_LEN];

        header.encode(&mut bytes).expect("header encodes");
        put_u32(&mut bytes, 36, 999);

        assert!(matches!(
            AudioFrameHeader::decode(&bytes),
            Err(ProtocolError::InvalidPayloadLength { .. })
        ));
    }
}
