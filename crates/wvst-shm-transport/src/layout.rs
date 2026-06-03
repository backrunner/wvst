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
    pub fn from_descriptor_bytes(input: &[u8]) -> Result<Self, SharedAudioLayoutError> {
        if input.len() < usize::from(SHARED_AUDIO_DESCRIPTOR_BYTES) {
            return Err(SharedAudioLayoutError::DescriptorTooShort {
                min: usize::from(SHARED_AUDIO_DESCRIPTOR_BYTES),
                actual: input.len(),
            });
        }

        let magic = read_u32(input, 0);
        if magic != SHARED_AUDIO_LAYOUT_MAGIC {
            return Err(SharedAudioLayoutError::InvalidDescriptorMagic { actual: magic });
        }

        let version = read_u16(input, 4);
        if version != SHARED_AUDIO_LAYOUT_VERSION {
            return Err(SharedAudioLayoutError::UnsupportedDescriptorVersion { actual: version });
        }

        expect_descriptor_field(
            "headerBytes",
            u64::from(SHARED_AUDIO_HEADER_BYTES),
            u64::from(read_u16(input, 6)),
        )?;
        expect_descriptor_field(
            "descriptorBytes",
            u64::from(SHARED_AUDIO_DESCRIPTOR_BYTES),
            u64::from(read_u16(input, 8)),
        )?;
        expect_descriptor_field(
            "ringCount",
            u64::from(SHARED_AUDIO_RING_COUNT),
            u64::from(read_u16(input, 10)),
        )?;
        expect_descriptor_field(
            "inputRingDescriptorOffset",
            u64::from(INPUT_RING_DESCRIPTOR_OFFSET),
            u64::from(read_u32(input, 32)),
        )?;
        expect_descriptor_field(
            "outputRingDescriptorOffset",
            u64::from(OUTPUT_RING_DESCRIPTOR_OFFSET),
            u64::from(read_u32(input, 36)),
        )?;

        let input_ring = SharedAudioRingLayout::decode_descriptor(
            input,
            INPUT_RING_DESCRIPTOR_OFFSET as usize,
            SharedAudioRingRole::Input,
        )?;
        let output_ring = SharedAudioRingLayout::decode_descriptor(
            input,
            OUTPUT_RING_DESCRIPTOR_OFFSET as usize,
            SharedAudioRingRole::Output,
        )?;
        let config = SharedAudioTransportConfig::new(
            read_u32(input, 12),
            read_u16(input, 16),
            read_u32(input, 20),
            input_ring.channels,
            output_ring.channels,
        );
        let expected = config.layout()?;

        validate_ring("input", input_ring, expected.input)?;
        validate_ring("output", output_ring, expected.output)?;
        expect_descriptor_field("totalBytes", expected.total_bytes, read_u64(input, 24))?;

        Ok(expected)
    }

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

    fn decode_descriptor(
        input: &[u8],
        offset: usize,
        expected_role: SharedAudioRingRole,
    ) -> Result<Self, SharedAudioLayoutError> {
        expect_descriptor_field(
            "ring.role",
            expected_role as u64,
            u64::from(read_u16(input, offset)),
        )?;

        Ok(Self {
            role: expected_role,
            channels: read_u16(input, offset + 2),
            block_frames: read_u16(input, offset + 4),
            capacity_blocks: read_u32(input, offset + 8),
            cursor_offset: read_u64(input, offset + 16),
            audio_offset: read_u64(input, offset + 24),
            audio_bytes: read_u64(input, offset + 32),
            capacity_frames: read_u64(input, offset + 40),
            capacity_samples: read_u64(input, offset + 48),
            cursor_bytes: read_u32(input, offset + 56),
        })
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

fn validate_ring(
    label: &'static str,
    actual: SharedAudioRingLayout,
    expected: SharedAudioRingLayout,
) -> Result<(), SharedAudioLayoutError> {
    expect_descriptor_field(
        ring_field(label, "blockFrames"),
        u64::from(expected.block_frames),
        u64::from(actual.block_frames),
    )?;
    expect_descriptor_field(
        ring_field(label, "capacityBlocks"),
        u64::from(expected.capacity_blocks),
        u64::from(actual.capacity_blocks),
    )?;
    expect_descriptor_field(
        ring_field(label, "channels"),
        u64::from(expected.channels),
        u64::from(actual.channels),
    )?;
    expect_descriptor_field(
        ring_field(label, "capacityFrames"),
        expected.capacity_frames,
        actual.capacity_frames,
    )?;
    expect_descriptor_field(
        ring_field(label, "capacitySamples"),
        expected.capacity_samples,
        actual.capacity_samples,
    )?;
    expect_descriptor_field(
        ring_field(label, "cursorOffset"),
        expected.cursor_offset,
        actual.cursor_offset,
    )?;
    expect_descriptor_field(
        ring_field(label, "cursorBytes"),
        u64::from(expected.cursor_bytes),
        u64::from(actual.cursor_bytes),
    )?;
    expect_descriptor_field(
        ring_field(label, "audioOffset"),
        expected.audio_offset,
        actual.audio_offset,
    )?;
    expect_descriptor_field(
        ring_field(label, "audioBytes"),
        expected.audio_bytes,
        actual.audio_bytes,
    )?;
    Ok(())
}

fn ring_field(label: &'static str, field: &'static str) -> &'static str {
    match (label, field) {
        ("input", "blockFrames") => "input.blockFrames",
        ("input", "capacityBlocks") => "input.capacityBlocks",
        ("input", "channels") => "input.channels",
        ("input", "capacityFrames") => "input.capacityFrames",
        ("input", "capacitySamples") => "input.capacitySamples",
        ("input", "cursorOffset") => "input.cursorOffset",
        ("input", "cursorBytes") => "input.cursorBytes",
        ("input", "audioOffset") => "input.audioOffset",
        ("input", "audioBytes") => "input.audioBytes",
        ("output", "blockFrames") => "output.blockFrames",
        ("output", "capacityBlocks") => "output.capacityBlocks",
        ("output", "channels") => "output.channels",
        ("output", "capacityFrames") => "output.capacityFrames",
        ("output", "capacitySamples") => "output.capacitySamples",
        ("output", "cursorOffset") => "output.cursorOffset",
        ("output", "cursorBytes") => "output.cursorBytes",
        ("output", "audioOffset") => "output.audioOffset",
        ("output", "audioBytes") => "output.audioBytes",
        _ => "ring.unknown",
    }
}

fn expect_descriptor_field(
    field: &'static str,
    expected: u64,
    actual: u64,
) -> Result<(), SharedAudioLayoutError> {
    if expected == actual {
        return Ok(());
    }
    Err(SharedAudioLayoutError::InvalidDescriptorField {
        field,
        expected,
        actual,
    })
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
#[path = "layout_tests.rs"]
mod tests;
