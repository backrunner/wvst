use serde::{Deserialize, Serialize};

use crate::{SharedAudioLayoutError, SharedAudioRingSpanPlan, SharedAudioTransportLayout};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioByteRange {
    pub offset: u64,
    pub bytes: u64,
}

impl SharedAudioByteRange {
    pub const fn new(offset: u64, bytes: u64) -> Self {
        Self { offset, bytes }
    }

    pub fn end_offset(self) -> Result<u64, SharedAudioLayoutError> {
        self.offset
            .checked_add(self.bytes)
            .ok_or(SharedAudioLayoutError::LayoutOverflow)
    }

    pub fn validate_within(self, memory_len: u64) -> Result<Self, SharedAudioLayoutError> {
        let end_offset = self.end_offset()?;
        if end_offset > memory_len {
            return Err(SharedAudioLayoutError::MemoryTooShort {
                min: end_offset,
                actual: memory_len,
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedAudioRingByteRanges {
    pub first: Option<SharedAudioByteRange>,
    pub second: Option<SharedAudioByteRange>,
}

impl SharedAudioRingByteRanges {
    pub fn validate_within(self, memory_len: u64) -> Result<Self, SharedAudioLayoutError> {
        if let Some(first) = self.first {
            first.validate_within(memory_len)?;
        }
        if let Some(second) = self.second {
            second.validate_within(memory_len)?;
        }
        Ok(self)
    }
}

impl SharedAudioTransportLayout {
    pub fn validate_memory_len(&self, memory_len: u64) -> Result<(), SharedAudioLayoutError> {
        if memory_len < self.total_bytes {
            return Err(SharedAudioLayoutError::MemoryTooShort {
                min: self.total_bytes,
                actual: memory_len,
            });
        }
        Ok(())
    }
}

impl SharedAudioRingSpanPlan {
    pub const fn byte_ranges(self) -> SharedAudioRingByteRanges {
        SharedAudioRingByteRanges {
            first: non_empty_range(self.first_byte_offset, self.first_bytes),
            second: non_empty_range(self.second_byte_offset, self.second_bytes),
        }
    }
}

const fn non_empty_range(offset: u64, bytes: u64) -> Option<SharedAudioByteRange> {
    if bytes == 0 {
        None
    } else {
        Some(SharedAudioByteRange::new(offset, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{F32_SAMPLE_BYTES, SharedAudioRingCursor, SharedAudioTransportConfig};

    #[test]
    fn validates_transport_memory_length() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");

        assert_eq!(layout.validate_memory_len(layout.total_bytes), Ok(()));
        assert_eq!(
            layout.validate_memory_len(layout.total_bytes - 1),
            Err(SharedAudioLayoutError::MemoryTooShort {
                min: layout.total_bytes,
                actual: layout.total_bytes - 1,
            })
        );
    }

    #[test]
    fn converts_contiguous_span_plan_to_byte_range() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");
        let plan = layout
            .input
            .read_plan(SharedAudioRingCursor::new(0, 128), 128)
            .expect("read plan");

        assert_eq!(
            plan.byte_ranges(),
            SharedAudioRingByteRanges {
                first: Some(SharedAudioByteRange::new(
                    layout.input.audio_offset,
                    128 * 2 * F32_SAMPLE_BYTES,
                )),
                second: None,
            }
        );
    }

    #[test]
    fn converts_wrapped_span_plan_to_two_byte_ranges() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 4, 2, 2)
            .layout()
            .expect("layout");
        let plan = layout
            .input
            .write_plan(SharedAudioRingCursor::new(64, 448), 128)
            .expect("write plan");

        assert_eq!(
            plan.byte_ranges(),
            SharedAudioRingByteRanges {
                first: Some(SharedAudioByteRange::new(
                    layout.input.audio_offset + 448 * 2 * F32_SAMPLE_BYTES,
                    64 * 2 * F32_SAMPLE_BYTES,
                )),
                second: Some(SharedAudioByteRange::new(
                    layout.input.audio_offset,
                    64 * 2 * F32_SAMPLE_BYTES,
                )),
            }
        );
        assert_eq!(
            plan.byte_ranges().validate_within(layout.total_bytes),
            Ok(plan.byte_ranges())
        );
    }

    #[test]
    fn omits_zero_byte_ranges_for_zero_channel_audio() {
        let layout = SharedAudioTransportConfig::new(48_000, 128, 2, 0, 2)
            .layout()
            .expect("layout");
        let plan = layout
            .input
            .write_plan(SharedAudioRingCursor::new(0, 0), 128)
            .expect("write plan");

        assert_eq!(
            plan.byte_ranges(),
            SharedAudioRingByteRanges {
                first: None,
                second: None,
            }
        );
    }

    #[test]
    fn rejects_byte_range_outside_memory() {
        let range = SharedAudioByteRange::new(100, 28);

        assert_eq!(range.end_offset(), Ok(128));
        assert_eq!(
            range.validate_within(127),
            Err(SharedAudioLayoutError::MemoryTooShort {
                min: 128,
                actual: 127,
            })
        );
    }
}
