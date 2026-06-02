pub mod audio_frame;
pub mod control;
pub mod error;
pub mod midi_event;
pub mod worker_audio_ipc;

pub use audio_frame::{
    AUDIO_FRAME_HEADER_LEN, AUDIO_FRAME_MAGIC, AUDIO_FRAME_VERSION, AudioFrameChannelCount,
    AudioFrameFlags, AudioFrameHeader, AudioSampleFormat,
};
pub use control::{HELLO_METHOD, HelloRequest, HelloResponse, negotiate_protocol};
pub use error::ProtocolError;
pub use midi_event::{MIDI_EVENT_LEN, MidiEvent, MidiEventKind};
pub use worker_audio_ipc::{
    WORKER_AUDIO_IPC_HEADER_LEN, WORKER_AUDIO_IPC_MAGIC, WORKER_AUDIO_IPC_VERSION,
    WorkerAudioIpcHeader, WorkerAudioIpcMessage, WorkerAudioMessageKind,
};
