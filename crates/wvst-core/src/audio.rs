use crate::CoreError;

pub const MIN_SAMPLE_RATE: u32 = 8_000;
pub const MAX_SAMPLE_RATE: u32 = 384_000;
pub const MAX_FRAME_COUNT: u16 = 4_096;
pub const MAX_CHANNEL_COUNT: u16 = 256;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SampleRate(u32);

impl SampleRate {
    pub fn new(value: u32) -> Result<Self, CoreError> {
        if (MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(CoreError::InvalidSampleRate(value))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameCount(u16);

impl FrameCount {
    pub fn new(value: u16) -> Result<Self, CoreError> {
        if value == 0 || value > MAX_FRAME_COUNT {
            Err(CoreError::InvalidFrameCount(value))
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ChannelCount(u16);

impl ChannelCount {
    pub fn new(value: u16) -> Result<Self, CoreError> {
        if value == 0 || value > MAX_CHANNEL_COUNT {
            Err(CoreError::InvalidChannelCount(value))
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

pub fn f32_payload_len(frames: FrameCount, channels: ChannelCount) -> Result<u32, CoreError> {
    let samples = u32::from(frames.get())
        .checked_mul(u32::from(channels.get()))
        .ok_or(CoreError::PayloadTooLarge)?;

    samples.checked_mul(4).ok_or(CoreError::PayloadTooLarge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_audio_ranges() {
        assert!(SampleRate::new(48_000).is_ok());
        assert!(SampleRate::new(1).is_err());
        assert!(FrameCount::new(128).is_ok());
        assert!(FrameCount::new(0).is_err());
        assert!(ChannelCount::new(2).is_ok());
        assert!(ChannelCount::new(0).is_err());
    }

    #[test]
    fn computes_f32_payload_len() {
        let frames = FrameCount::new(128).expect("valid frame count");
        let channels = ChannelCount::new(2).expect("valid channel count");

        assert_eq!(f32_payload_len(frames, channels), Ok(1_024));
    }
}
