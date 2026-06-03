#![forbid(unsafe_code)]

mod error;
mod layout;

pub use error::SharedAudioLayoutError;
pub use layout::{
    CACHE_LINE_BYTES, F32_SAMPLE_BYTES, MAX_SHARED_AUDIO_CHANNELS, SHARED_AUDIO_DESCRIPTOR_BYTES,
    SHARED_AUDIO_HEADER_BYTES, SHARED_AUDIO_LAYOUT_MAGIC, SHARED_AUDIO_LAYOUT_VERSION,
    SHARED_AUDIO_RING_CURSOR_BYTES, SHARED_AUDIO_RING_DESCRIPTOR_BYTES, SharedAudioRingLayout,
    SharedAudioRingRole, SharedAudioTransportConfig, SharedAudioTransportLayout,
};
