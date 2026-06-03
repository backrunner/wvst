#![forbid(unsafe_code)]

mod cursor;
mod error;
mod io;
mod layout;
mod memory;

pub use cursor::{
    SHARED_AUDIO_CURSOR_BLOCK_BYTES, SHARED_AUDIO_CURSOR_DROPPED_FRAMES_OFFSET,
    SHARED_AUDIO_CURSOR_FLAGS_OFFSET, SHARED_AUDIO_CURSOR_GENERATION_OFFSET,
    SHARED_AUDIO_CURSOR_OVERRUN_FRAMES_OFFSET, SHARED_AUDIO_CURSOR_READ_FRAME_OFFSET,
    SHARED_AUDIO_CURSOR_UNDERRUN_FRAMES_OFFSET, SHARED_AUDIO_CURSOR_WRITE_FRAME_OFFSET,
    SharedAudioRingCursorState,
};
pub use error::SharedAudioLayoutError;
pub use io::SharedAudioRingIoReport;
pub use layout::{
    CACHE_LINE_BYTES, F32_SAMPLE_BYTES, MAX_SHARED_AUDIO_CHANNELS, SHARED_AUDIO_DESCRIPTOR_BYTES,
    SHARED_AUDIO_HEADER_BYTES, SHARED_AUDIO_LAYOUT_MAGIC, SHARED_AUDIO_LAYOUT_VERSION,
    SHARED_AUDIO_RING_CURSOR_BYTES, SHARED_AUDIO_RING_DESCRIPTOR_BYTES, SharedAudioRingCursor,
    SharedAudioRingLayout, SharedAudioRingRole, SharedAudioRingSpanPlan,
    SharedAudioTransportConfig, SharedAudioTransportLayout,
};
pub use memory::{SharedAudioByteRange, SharedAudioRingByteRanges};
