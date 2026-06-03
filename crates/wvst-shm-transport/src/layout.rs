use serde::{Deserialize, Serialize};

use crate::SharedAudioLayoutError;

pub const SHARED_AUDIO_LAYOUT_MAGIC: u32 = u32::from_le_bytes(*b"WVSM");
pub const SHARED_AUDIO_LAYOUT_VERSION: u16 = 1;
pub const CACHE_LINE_BYTES: u64 = 64;
pub const F32_SAMPLE_BYTES: u64 = 4;
pub const MAX_SHARED_AUDIO_CHANNELS: u16 = 64;
pub const SHARED_AUDIO_HEADER_BYTES: u16 = 64;
pub const SHARED_AUDIO_RING_DESCRIPTOR_BYTES: u16 = 64;
pub const SHARED_AUDIO_RING_CURSOR_BYTES: u32 = 64;
pub const SHARED_AUDIO_RING_COUNT: u16 = 2;
pub const SHARED_AUDIO_DESCRIPTOR_BYTES: u16 =
    SHARED_AUDIO_HEADER_BYTES + SHARED_AUDIO_RING_DESCRIPTOR_BYTES * SHARED_AUDIO_RING_COUNT;

const INPUT_RING_DESCRIPTOR_OFFSET: u32 = SHARED_AUDIO_HEADER_BYTES as u32;
const OUTPUT_RING_DESCRIPTOR_OFFSET: u32 =
    INPUT_RING_DESCRIPTOR_OFFSET + SHARED_AUDIO_RING_DESCRIPTOR_BYTES as u32;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioTransportConfig {
    pub sample_rate_hz: u32,
    pub block_frames: u16,
    pub capacity_blocks: u32,
    pub input_channels: u16,
    pub output_channels: u16,
}

impl SharedAudioTransportConfig {
    pub const fn new(
        sample_rate_hz: u32,
        block_frames: u16,
        capacity_blocks: u32,
        input_channels: u16,
        output_channels: u16,
    ) -> Self {
        Self {
            sample_rate_hz,
            block_frames,
            capacity_blocks,
            input_channels,
            output_channels,
        }
    }

    pub fn layout(self) -> Result<SharedAudioTransportLayout, SharedAudioLayoutError> {
        validate_config(self)?;

        let input = SharedAudioRingLayout::new(
            SharedAudioRingRole::Input,
            self.block_frames,
            self.capacity_blocks,
            self.input_channels,
            align_up(u64::from(SHARED_AUDIO_DESCRIPTOR_BYTES), CACHE_LINE_BYTES)?,
        )?;
        let output = SharedAudioRingLayout::new(
            SharedAudioRingRole::Output,
            self.block_frames,
            self.capacity_blocks,
            self.output_channels,
            align_up(input.end_offset(), CACHE_LINE_BYTES)?,
        )?;

        Ok(SharedAudioTransportLayout {
            config: self,
            input,
            output,
            total_bytes: align_up(output.end_offset(), CACHE_LINE_BYTES)?,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioTransportLayout {
    pub config: SharedAudioTransportConfig,
    pub input: SharedAudioRingLayout,
    pub output: SharedAudioRingLayout,
    pub total_bytes: u64,
}

impl SharedAudioTransportLayout {
    pub fn descriptor_bytes(&self) -> [u8; SHARED_AUDIO_DESCRIPTOR_BYTES as usize] {
        let mut output = [0_u8; SHARED_AUDIO_DESCRIPTOR_BYTES as usize];
        put_u32(&mut output, 0, SHARED_AUDIO_LAYOUT_MAGIC);
        put_u16(&mut output, 4, SHARED_AUDIO_LAYOUT_VERSION);
        put_u16(&mut output, 6, SHARED_AUDIO_HEADER_BYTES);
        put_u16(&mut output, 8, SHARED_AUDIO_DESCRIPTOR_BYTES);
        put_u16(&mut output, 10, SHARED_AUDIO_RING_COUNT);
        put_u32(&mut output, 12, self.config.sample_rate_hz);
        put_u16(&mut output, 16, self.config.block_frames);
        put_u32(&mut output, 20, self.config.capacity_blocks);
        put_u64(&mut output, 24, self.total_bytes);
        put_u32(&mut output, 32, INPUT_RING_DESCRIPTOR_OFFSET);
        put_u32(&mut output, 36, OUTPUT_RING_DESCRIPTOR_OFFSET);

        self.input
            .encode_descriptor(&mut output, INPUT_RING_DESCRIPTOR_OFFSET as usize);
        self.output
            .encode_descriptor(&mut output, OUTPUT_RING_DESCRIPTOR_OFFSET as usize);
        output
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingLayout {
    pub role: SharedAudioRingRole,
    pub block_frames: u16,
    pub capacity_blocks: u32,
    pub channels: u16,
    pub capacity_frames: u64,
    pub capacity_samples: u64,
    pub cursor_offset: u64,
    pub cursor_bytes: u32,
    pub audio_offset: u64,
    pub audio_bytes: u64,
}

impl SharedAudioRingLayout {
    fn new(
        role: SharedAudioRingRole,
        block_frames: u16,
        capacity_blocks: u32,
        channels: u16,
        start_offset: u64,
    ) -> Result<Self, SharedAudioLayoutError> {
        let cursor_offset = align_up(start_offset, CACHE_LINE_BYTES)?;
        let audio_offset = align_up(
            checked_add(cursor_offset, u64::from(SHARED_AUDIO_RING_CURSOR_BYTES))?,
            CACHE_LINE_BYTES,
        )?;
        let capacity_frames = checked_mul(u64::from(block_frames), u64::from(capacity_blocks))?;
        let capacity_samples = checked_mul(capacity_frames, u64::from(channels))?;
        let audio_bytes = checked_mul(capacity_samples, F32_SAMPLE_BYTES)?;

        Ok(Self {
            role,
            block_frames,
            capacity_blocks,
            channels,
            capacity_frames,
            capacity_samples,
            cursor_offset,
            cursor_bytes: SHARED_AUDIO_RING_CURSOR_BYTES,
            audio_offset,
            audio_bytes,
        })
    }

    pub fn end_offset(self) -> u64 {
        self.audio_offset.saturating_add(self.audio_bytes)
    }

    pub fn readable_frames(
        self,
        cursor: SharedAudioRingCursor,
    ) -> Result<u64, SharedAudioLayoutError> {
        cursor
            .pending_frames()
            .map(|pending| pending.min(self.capacity_frames))
    }

    pub fn writable_frames(
        self,
        cursor: SharedAudioRingCursor,
    ) -> Result<u64, SharedAudioLayoutError> {
        let pending = cursor.pending_frames()?.min(self.capacity_frames);
        Ok(self.capacity_frames.saturating_sub(pending))
    }

    pub fn read_plan(
        self,
        cursor: SharedAudioRingCursor,
        frames: u64,
    ) -> Result<SharedAudioRingSpanPlan, SharedAudioLayoutError> {
        let available = self.readable_frames(cursor)?;
        if frames > available {
            return Err(SharedAudioLayoutError::InsufficientReadableFrames {
                requested: frames,
                available,
            });
        }
        self.plan(cursor.read_frame, frames)
    }

    pub fn write_plan(
        self,
        cursor: SharedAudioRingCursor,
        frames: u64,
    ) -> Result<SharedAudioRingSpanPlan, SharedAudioLayoutError> {
        let available = self.writable_frames(cursor)?;
        if frames > available {
            return Err(SharedAudioLayoutError::InsufficientWritableFrames {
                requested: frames,
                available,
            });
        }
        self.plan(cursor.write_frame, frames)
    }

    fn plan(
        self,
        start_frame: u64,
        frames: u64,
    ) -> Result<SharedAudioRingSpanPlan, SharedAudioLayoutError> {
        let frame_offset = start_frame % self.capacity_frames;
        let first_frames = frames.min(self.capacity_frames - frame_offset);
        let second_frames = frames.saturating_sub(first_frames);
        let first_sample_offset = checked_mul(frame_offset, u64::from(self.channels))?;
        let second_sample_offset = 0;
        let first_samples = checked_mul(first_frames, u64::from(self.channels))?;
        let second_samples = checked_mul(second_frames, u64::from(self.channels))?;
        let first_byte_offset = checked_add(
            self.audio_offset,
            checked_mul(first_sample_offset, F32_SAMPLE_BYTES)?,
        )?;
        let second_byte_offset = checked_add(
            self.audio_offset,
            checked_mul(second_sample_offset, F32_SAMPLE_BYTES)?,
        )?;

        Ok(SharedAudioRingSpanPlan {
            start_frame,
            frame_offset,
            frames,
            channels: self.channels,
            first_frames,
            second_frames,
            first_byte_offset,
            first_bytes: checked_mul(first_samples, F32_SAMPLE_BYTES)?,
            second_byte_offset,
            second_bytes: checked_mul(second_samples, F32_SAMPLE_BYTES)?,
        })
    }

    fn encode_descriptor(self, output: &mut [u8], offset: usize) {
        put_u16(output, offset, self.role as u16);
        put_u16(output, offset + 2, self.channels);
        put_u16(output, offset + 4, self.block_frames);
        put_u32(output, offset + 8, self.capacity_blocks);
        put_u64(output, offset + 16, self.cursor_offset);
        put_u64(output, offset + 24, self.audio_offset);
        put_u64(output, offset + 32, self.audio_bytes);
        put_u64(output, offset + 40, self.capacity_frames);
        put_u64(output, offset + 48, self.capacity_samples);
        put_u32(output, offset + 56, self.cursor_bytes);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[repr(u16)]
pub enum SharedAudioRingRole {
    Input = 1,
    Output = 2,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingCursor {
    pub read_frame: u64,
    pub write_frame: u64,
}

impl SharedAudioRingCursor {
    pub const fn new(read_frame: u64, write_frame: u64) -> Self {
        Self {
            read_frame,
            write_frame,
        }
    }

    fn pending_frames(self) -> Result<u64, SharedAudioLayoutError> {
        self.write_frame.checked_sub(self.read_frame).ok_or(
            SharedAudioLayoutError::CursorOrderInvalid {
                read_frame: self.read_frame,
                write_frame: self.write_frame,
            },
        )
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingSpanPlan {
    pub start_frame: u64,
    pub frame_offset: u64,
    pub frames: u64,
    pub channels: u16,
    pub first_frames: u64,
    pub second_frames: u64,
    pub first_byte_offset: u64,
    pub first_bytes: u64,
    pub second_byte_offset: u64,
    pub second_bytes: u64,
}

fn validate_config(config: SharedAudioTransportConfig) -> Result<(), SharedAudioLayoutError> {
    if config.sample_rate_hz == 0 {
        return Err(SharedAudioLayoutError::ZeroSampleRate);
    }
    if config.block_frames == 0 {
        return Err(SharedAudioLayoutError::ZeroBlockFrames);
    }
    if config.capacity_blocks == 0 {
        return Err(SharedAudioLayoutError::ZeroCapacityBlocks);
    }
    validate_channels(config.input_channels)?;
    validate_channels(config.output_channels)?;
    Ok(())
}

fn validate_channels(channels: u16) -> Result<(), SharedAudioLayoutError> {
    if channels > MAX_SHARED_AUDIO_CHANNELS {
        return Err(SharedAudioLayoutError::ChannelCountTooLarge {
            channels,
            max: MAX_SHARED_AUDIO_CHANNELS,
        });
    }
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, SharedAudioLayoutError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    checked_add(value, alignment - remainder)
}

fn checked_add(left: u64, right: u64) -> Result<u64, SharedAudioLayoutError> {
    left.checked_add(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

fn checked_mul(left: u64, right: u64) -> Result<u64, SharedAudioLayoutError> {
    left.checked_mul(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
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

    #[test]
    fn computes_aligned_stereo_layout() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");

        assert_eq!(layout.input.cursor_offset, 192);
        assert_eq!(layout.input.audio_offset, 256);
        assert_eq!(layout.input.capacity_frames, 512);
        assert_eq!(layout.input.audio_bytes, 4_096);
        assert_eq!(layout.output.cursor_offset, 4_352);
        assert_eq!(layout.output.audio_offset, 4_416);
        assert_eq!(layout.output.audio_bytes, 4_096);
        assert_eq!(layout.total_bytes, 8_512);
        assert_eq!(layout.input.cursor_offset % CACHE_LINE_BYTES, 0);
        assert_eq!(layout.output.audio_offset % CACHE_LINE_BYTES, 0);
    }

    #[test]
    fn supports_zero_input_instrument_layout() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 0, 2)
            .layout()
            .expect("layout");

        assert_eq!(layout.input.capacity_samples, 0);
        assert_eq!(layout.input.audio_bytes, 0);
        assert_eq!(layout.output.cursor_offset, layout.input.audio_offset);
        assert!(layout.total_bytes > layout.output.audio_offset);
    }

    #[test]
    fn descriptor_encodes_header_and_ring_offsets() {
        let layout = SharedAudioTransportConfig::new(44_100, 64, 3, 1, 2)
            .layout()
            .expect("layout");
        let descriptor = layout.descriptor_bytes();

        assert_eq!(read_u32(&descriptor, 0), SHARED_AUDIO_LAYOUT_MAGIC);
        assert_eq!(read_u16(&descriptor, 4), SHARED_AUDIO_LAYOUT_VERSION);
        assert_eq!(read_u16(&descriptor, 8), SHARED_AUDIO_DESCRIPTOR_BYTES);
        assert_eq!(read_u16(&descriptor, 10), SHARED_AUDIO_RING_COUNT);
        assert_eq!(read_u32(&descriptor, 12), 44_100);
        assert_eq!(read_u16(&descriptor, 16), 64);
        assert_eq!(read_u32(&descriptor, 20), 3);
        assert_eq!(read_u64(&descriptor, 24), layout.total_bytes);
        assert_eq!(read_u16(&descriptor, 64), SharedAudioRingRole::Input as u16);
        assert_eq!(read_u16(&descriptor, 66), 1);
        assert_eq!(read_u64(&descriptor, 80), layout.input.cursor_offset);
        assert_eq!(
            read_u16(&descriptor, 128),
            SharedAudioRingRole::Output as u16
        );
        assert_eq!(read_u16(&descriptor, 130), 2);
        assert_eq!(read_u64(&descriptor, 152), layout.output.audio_offset);
    }

    #[test]
    fn rejects_invalid_config() {
        assert_eq!(
            SharedAudioTransportConfig::new(0, 128, 2, 2, 2).layout(),
            Err(SharedAudioLayoutError::ZeroSampleRate)
        );
        assert_eq!(
            SharedAudioTransportConfig::new(48_000, 0, 2, 2, 2).layout(),
            Err(SharedAudioLayoutError::ZeroBlockFrames)
        );
        assert_eq!(
            SharedAudioTransportConfig::new(48_000, 128, 0, 2, 2).layout(),
            Err(SharedAudioLayoutError::ZeroCapacityBlocks)
        );
        assert_eq!(
            SharedAudioTransportConfig::new(48_000, 128, 2, MAX_SHARED_AUDIO_CHANNELS + 1, 2,)
                .layout(),
            Err(SharedAudioLayoutError::ChannelCountTooLarge {
                channels: MAX_SHARED_AUDIO_CHANNELS + 1,
                max: MAX_SHARED_AUDIO_CHANNELS,
            })
        );
    }

    #[test]
    fn plans_contiguous_read_and_write_spans() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");
        let cursor = SharedAudioRingCursor::new(0, 128);

        assert_eq!(layout.input.readable_frames(cursor), Ok(128));
        assert_eq!(layout.input.writable_frames(cursor), Ok(384));

        let read = layout.input.read_plan(cursor, 128).expect("read");
        assert_eq!(read.first_byte_offset, layout.input.audio_offset);
        assert_eq!(read.first_bytes, 128 * 2 * F32_SAMPLE_BYTES);
        assert_eq!(read.second_bytes, 0);

        let write = layout.input.write_plan(cursor, 128).expect("write");
        assert_eq!(
            write.first_byte_offset,
            layout.input.audio_offset + 128 * 2 * F32_SAMPLE_BYTES
        );
        assert_eq!(write.first_frames, 128);
        assert_eq!(write.second_frames, 0);
    }

    #[test]
    fn plans_wrapped_write_spans() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");
        let cursor = SharedAudioRingCursor::new(64, 448);

        let plan = layout.input.write_plan(cursor, 128).expect("write");

        assert_eq!(plan.frame_offset, 448);
        assert_eq!(plan.first_frames, 64);
        assert_eq!(plan.second_frames, 64);
        assert_eq!(
            plan.first_byte_offset,
            layout.input.audio_offset + 448 * 2 * F32_SAMPLE_BYTES
        );
        assert_eq!(plan.first_bytes, 64 * 2 * F32_SAMPLE_BYTES);
        assert_eq!(plan.second_byte_offset, layout.input.audio_offset);
        assert_eq!(plan.second_bytes, 64 * 2 * F32_SAMPLE_BYTES);
    }

    #[test]
    fn rejects_invalid_cursor_and_backpressure() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 2, 2)
            .layout()
            .expect("layout");

        assert_eq!(
            layout
                .input
                .readable_frames(SharedAudioRingCursor::new(10, 4)),
            Err(SharedAudioLayoutError::CursorOrderInvalid {
                read_frame: 10,
                write_frame: 4,
            })
        );
        assert_eq!(
            layout
                .input
                .write_plan(SharedAudioRingCursor::new(0, 256), 128),
            Err(SharedAudioLayoutError::InsufficientWritableFrames {
                requested: 128,
                available: 0,
            })
        );
        assert_eq!(
            layout
                .input
                .read_plan(SharedAudioRingCursor::new(0, 64), 128),
            Err(SharedAudioLayoutError::InsufficientReadableFrames {
                requested: 128,
                available: 64,
            })
        );
    }

    #[test]
    fn zero_channel_plan_keeps_frame_span_without_sample_bytes() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 0, 2)
            .layout()
            .expect("layout");
        let cursor = SharedAudioRingCursor::new(0, 0);

        let plan = layout.input.write_plan(cursor, 128).expect("write");

        assert_eq!(plan.frames, 128);
        assert_eq!(plan.channels, 0);
        assert_eq!(plan.first_frames, 128);
        assert_eq!(plan.second_frames, 0);
        assert_eq!(plan.first_bytes, 0);
        assert_eq!(plan.second_bytes, 0);
        assert_eq!(plan.first_byte_offset, layout.input.audio_offset);
    }

    fn read_u16(input: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(input[offset..offset + 2].try_into().expect("u16 bytes"))
    }

    fn read_u32(input: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(input[offset..offset + 4].try_into().expect("u32 bytes"))
    }

    fn read_u64(input: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(input[offset..offset + 8].try_into().expect("u64 bytes"))
    }
}
