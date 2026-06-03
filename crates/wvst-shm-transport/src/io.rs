use serde::{Deserialize, Serialize};

use crate::{
    F32_SAMPLE_BYTES, SHARED_AUDIO_DESCRIPTOR_BYTES, SharedAudioLayoutError,
    SharedAudioRingCursorState, SharedAudioRingLayout, SharedAudioRingSpanPlan,
    SharedAudioTransportLayout,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingIoReport {
    pub frames: u64,
    pub samples: usize,
    pub cursor: SharedAudioRingCursorState,
}

impl SharedAudioTransportLayout {
    pub fn initialize_memory(&self, memory: &mut [u8]) -> Result<(), SharedAudioLayoutError> {
        self.validate_memory_len(memory.len() as u64)?;
        let total_bytes = checked_usize(self.total_bytes)?;
        memory[..total_bytes].fill(0);
        memory[..usize::from(SHARED_AUDIO_DESCRIPTOR_BYTES)]
            .copy_from_slice(&self.descriptor_bytes());
        self.input
            .set_cursor_state(memory, SharedAudioRingCursorState::default())?;
        self.output
            .set_cursor_state(memory, SharedAudioRingCursorState::default())?;
        Ok(())
    }
}

impl SharedAudioRingLayout {
    pub fn cursor_state(
        self,
        memory: &[u8],
    ) -> Result<SharedAudioRingCursorState, SharedAudioLayoutError> {
        let range = self.cursor_range().validate_within(memory.len() as u64)?;
        let start = checked_usize(range.offset)?;
        let end = checked_usize(range.end_offset()?)?;
        SharedAudioRingCursorState::from_bytes(&memory[start..end])?.validate_order()
    }

    pub fn set_cursor_state(
        self,
        memory: &mut [u8],
        state: SharedAudioRingCursorState,
    ) -> Result<(), SharedAudioLayoutError> {
        state.validate_order()?;
        let range = self.cursor_range().validate_within(memory.len() as u64)?;
        let start = checked_usize(range.offset)?;
        let end = checked_usize(range.end_offset()?)?;
        memory[start..end].copy_from_slice(&state.to_bytes());
        Ok(())
    }

    pub fn write_interleaved_f32(
        self,
        memory: &mut [u8],
        samples: &[f32],
        frames: u64,
    ) -> Result<SharedAudioRingIoReport, SharedAudioLayoutError> {
        let sample_count = self.validate_sample_len(samples.len(), frames)?;
        let mut cursor = self.cursor_state(memory)?;
        let available = self.writable_frames(cursor.cursor())?;
        if frames > available {
            cursor.overrun_frames = checked_add_u64(cursor.overrun_frames, frames - available)?;
            cursor.generation = checked_add_u64(cursor.generation, 1)?;
            self.set_cursor_state(memory, cursor)?;
            return Err(SharedAudioLayoutError::InsufficientWritableFrames {
                requested: frames,
                available,
            });
        }

        let plan = self.write_plan(cursor.cursor(), frames)?;
        write_samples(memory, plan, samples)?;
        cursor.write_frame = checked_add_u64(cursor.write_frame, frames)?;
        cursor.generation = checked_add_u64(cursor.generation, 1)?;
        self.set_cursor_state(memory, cursor)?;
        Ok(SharedAudioRingIoReport {
            frames,
            samples: sample_count,
            cursor,
        })
    }

    pub fn read_interleaved_f32(
        self,
        memory: &mut [u8],
        samples: &mut [f32],
        frames: u64,
    ) -> Result<SharedAudioRingIoReport, SharedAudioLayoutError> {
        let sample_count = self.validate_sample_len(samples.len(), frames)?;
        let mut cursor = self.cursor_state(memory)?;
        let available = self.readable_frames(cursor.cursor())?;
        if frames > available {
            cursor.underrun_frames = checked_add_u64(cursor.underrun_frames, frames - available)?;
            cursor.generation = checked_add_u64(cursor.generation, 1)?;
            self.set_cursor_state(memory, cursor)?;
            return Err(SharedAudioLayoutError::InsufficientReadableFrames {
                requested: frames,
                available,
            });
        }

        let plan = self.read_plan(cursor.cursor(), frames)?;
        read_samples(memory, plan, samples)?;
        cursor.read_frame = checked_add_u64(cursor.read_frame, frames)?;
        cursor.generation = checked_add_u64(cursor.generation, 1)?;
        self.set_cursor_state(memory, cursor)?;
        Ok(SharedAudioRingIoReport {
            frames,
            samples: sample_count,
            cursor,
        })
    }

    fn validate_sample_len(
        self,
        actual: usize,
        frames: u64,
    ) -> Result<usize, SharedAudioLayoutError> {
        let expected = checked_usize(checked_mul(frames, u64::from(self.channels))?)?;
        if actual != expected {
            return Err(SharedAudioLayoutError::SampleBufferLengthMismatch { expected, actual });
        }
        Ok(expected)
    }
}

fn write_samples(
    memory: &mut [u8],
    plan: SharedAudioRingSpanPlan,
    samples: &[f32],
) -> Result<(), SharedAudioLayoutError> {
    plan.byte_ranges().validate_within(memory.len() as u64)?;
    let first_samples = sample_count_from_bytes(plan.first_bytes)?;
    write_sample_range(memory, plan.first_byte_offset, &samples[..first_samples])?;
    write_sample_range(memory, plan.second_byte_offset, &samples[first_samples..])?;
    Ok(())
}

fn read_samples(
    memory: &[u8],
    plan: SharedAudioRingSpanPlan,
    samples: &mut [f32],
) -> Result<(), SharedAudioLayoutError> {
    plan.byte_ranges().validate_within(memory.len() as u64)?;
    let first_samples = sample_count_from_bytes(plan.first_bytes)?;
    read_sample_range(
        memory,
        plan.first_byte_offset,
        &mut samples[..first_samples],
    )?;
    read_sample_range(
        memory,
        plan.second_byte_offset,
        &mut samples[first_samples..],
    )?;
    Ok(())
}

fn write_sample_range(
    memory: &mut [u8],
    offset: u64,
    samples: &[f32],
) -> Result<(), SharedAudioLayoutError> {
    let start = checked_usize(offset)?;
    for (index, sample) in samples.iter().enumerate() {
        let byte_offset = checked_mul_usize(index, F32_SAMPLE_BYTES as usize)?;
        let sample_offset = checked_add_usize(start, byte_offset)?;
        memory[sample_offset..sample_offset + F32_SAMPLE_BYTES as usize]
            .copy_from_slice(&sample.to_le_bytes());
    }
    Ok(())
}

fn read_sample_range(
    memory: &[u8],
    offset: u64,
    samples: &mut [f32],
) -> Result<(), SharedAudioLayoutError> {
    let start = checked_usize(offset)?;
    for (index, sample) in samples.iter_mut().enumerate() {
        let byte_offset = checked_mul_usize(index, F32_SAMPLE_BYTES as usize)?;
        let sample_offset = checked_add_usize(start, byte_offset)?;
        *sample = f32::from_le_bytes(
            memory[sample_offset..sample_offset + F32_SAMPLE_BYTES as usize]
                .try_into()
                .expect("f32 sample bytes"),
        );
    }
    Ok(())
}

fn sample_count_from_bytes(bytes: u64) -> Result<usize, SharedAudioLayoutError> {
    checked_usize(bytes / F32_SAMPLE_BYTES)
}

fn checked_usize(value: u64) -> Result<usize, SharedAudioLayoutError> {
    usize::try_from(value).map_err(|_| SharedAudioLayoutError::LayoutOverflow)
}

fn checked_add_u64(left: u64, right: u64) -> Result<u64, SharedAudioLayoutError> {
    left.checked_add(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

fn checked_add_usize(left: usize, right: usize) -> Result<usize, SharedAudioLayoutError> {
    left.checked_add(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

fn checked_mul(left: u64, right: u64) -> Result<u64, SharedAudioLayoutError> {
    left.checked_mul(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

fn checked_mul_usize(left: usize, right: usize) -> Result<usize, SharedAudioLayoutError> {
    left.checked_mul(right)
        .ok_or(SharedAudioLayoutError::LayoutOverflow)
}

#[cfg(test)]
#[path = "io_tests.rs"]
mod tests;
