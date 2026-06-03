use serde::{Deserialize, Serialize};

use crate::SharedAudioLayoutError;
use crate::layout::{SHARED_AUDIO_RING_CURSOR_BYTES, SharedAudioRingCursor};

pub const SHARED_AUDIO_CURSOR_BLOCK_BYTES: usize = SHARED_AUDIO_RING_CURSOR_BYTES as usize;
pub const SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET: usize = 0;
pub const SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET: usize = 8;
pub const SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET: usize = 16;
pub const SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET: usize = 24;
pub const SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET: usize = 32;
pub const SHARED_AUDIO_CURSOR_GENERATION_OFFSET: usize = 40;
pub const SHARED_AUDIO_CURSOR_FLAGS_OFFSET: usize = 48;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingCursorState {
    pub read_frame: u64,
    pub write_frame: u64,
    pub dropped_frames: u64,
    pub underrun_frames: u64,
    pub overrun_frames: u64,
    pub generation: u64,
    pub flags: u32,
}

impl SharedAudioRingCursorState {
    pub const fn new(read_frame: u64, write_frame: u64) -> Self {
        Self {
            read_frame,
            write_frame,
            dropped_frames: 0,
            underrun_frames: 0,
            overrun_frames: 0,
            generation: 0,
            flags: 0,
        }
    }

    pub const fn cursor(self) -> SharedAudioRingCursor {
        SharedAudioRingCursor::new(self.read_frame, self.write_frame)
    }

    pub fn pending_frames(self) -> Result<u64, SharedAudioLayoutError> {
        self.write_frame.checked_sub(self.read_frame).ok_or(
            SharedAudioLayoutError::CursorOrderInvalid {
                read_frame: self.read_frame,
                write_frame: self.write_frame,
            },
        )
    }

    pub fn validate_order(self) -> Result<Self, SharedAudioLayoutError> {
        self.pending_frames()?;
        Ok(self)
    }

    pub fn from_bytes(input: &[u8]) -> Result<Self, SharedAudioLayoutError> {
        if input.len() < SHARED_AUDIO_CURSOR_BLOCK_BYTES {
            return Err(SharedAudioLayoutError::CursorBlockTooShort {
                min: SHARED_AUDIO_CURSOR_BLOCK_BYTES,
                actual: input.len(),
            });
        }

        Ok(Self {
            read_frame: read_u64(input, SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET),
            write_frame: read_u64(input, SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET),
            dropped_frames: read_u64(input, SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET),
            underrun_frames: read_u64(input, SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET),
            overrun_frames: read_u64(input, SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET),
            generation: read_u64(input, SHARED_AUDIO_CURSOR_GENERATION_OFFSET),
            flags: read_u32(input, SHARED_AUDIO_CURSOR_FLAGS_OFFSET),
        })
    }

    pub fn to_bytes(self) -> [u8; SHARED_AUDIO_CURSOR_BLOCK_BYTES] {
        let mut output = [0_u8; SHARED_AUDIO_CURSOR_BLOCK_BYTES];
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET,
            self.read_frame,
        );
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET,
            self.write_frame,
        );
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET,
            self.dropped_frames,
        );
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET,
            self.underrun_frames,
        );
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET,
            self.overrun_frames,
        );
        put_u64(
            &mut output,
            SHARED_AUDIO_CURSOR_GENERATION_OFFSET,
            self.generation,
        );
        put_u32(&mut output, SHARED_AUDIO_CURSOR_FLAGS_OFFSET, self.flags);
        output
    }
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().expect("u32 bytes"))
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(input[offset..offset + 8].try_into().expect("u64 bytes"))
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedAudioTransportConfig;

    #[test]
    fn cursor_state_round_trips_fixed_block_bytes() {
        let state = SharedAudioRingCursorState {
            read_frame: 128,
            write_frame: 384,
            dropped_frames: 4,
            underrun_frames: 2,
            overrun_frames: 1,
            generation: 9,
            flags: 0x05,
        };
        let bytes = state.to_bytes();

        assert_eq!(bytes.len(), SHARED_AUDIO_CURSOR_BLOCK_BYTES);
        assert_eq!(SharedAudioRingCursorState::from_bytes(&bytes), Ok(state));
        assert!(bytes[52..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn cursor_state_rejects_short_blocks() {
        assert_eq!(
            SharedAudioRingCursorState::from_bytes(&[0_u8; 8]),
            Err(SharedAudioLayoutError::CursorBlockTooShort {
                min: SHARED_AUDIO_CURSOR_BLOCK_BYTES,
                actual: 8,
            })
        );
    }

    #[test]
    fn cursor_state_validates_order() {
        assert_eq!(
            SharedAudioRingCursorState::new(256, 128).validate_order(),
            Err(SharedAudioLayoutError::CursorOrderInvalid {
                read_frame: 256,
                write_frame: 128,
            })
        );
        assert_eq!(
            SharedAudioRingCursorState::new(128, 384).pending_frames(),
            Ok(256)
        );
    }

    #[test]
    fn cursor_state_feeds_ring_span_planning() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");
        let state = SharedAudioRingCursorState::new(64, 448);

        let plan = layout
            .input
            .write_plan(state.cursor(), 128)
            .expect("write plan");

        assert_eq!(plan.first_frames, 64);
        assert_eq!(plan.second_frames, 64);
    }
}
