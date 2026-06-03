pub mod audio_frame;
pub mod control;
pub mod error;
pub mod midi_event;
pub mod parameter_event;
pub mod vst3_output_event;
pub mod worker_audio_ipc;
pub mod worker_control_batch;
pub mod worker_control_ipc;

pub use audio_frame::{
    AUDIO_FRAME_HEADER_LEN, AUDIO_FRAME_MAGIC, AUDIO_FRAME_VERSION, AudioFrameChannelCount,
    AudioFrameFlags, AudioFrameHeader, AudioSampleFormat,
};
pub use control::{HELLO_METHOD, HelloRequest, HelloResponse, negotiate_protocol};
pub use error::ProtocolError;
pub use midi_event::{MIDI_EVENT_LEN, MidiEvent, MidiEventKind};
pub use parameter_event::{PARAMETER_AUTOMATION_EVENT_LEN, ParameterAutomationEvent};
pub use vst3_output_event::{
    VST3_OUTPUT_EVENT_LEN, VST3_OUTPUT_EVENT_PAYLOAD_BYTES,
    VST3_OUTPUT_EVENT_PAYLOAD_FLAG_INVALID_TEXT, VST3_OUTPUT_EVENT_PAYLOAD_FLAG_TRUNCATED,
    VST3_OUTPUT_EVENT_PAYLOAD_FLAG_UNAVAILABLE, Vst3OutputEvent, Vst3OutputEventKind,
    Vst3OutputEventPayloadEncoding,
};
pub use worker_audio_ipc::{
    WORKER_AUDIO_IPC_HEADER_LEN, WORKER_AUDIO_IPC_MAGIC, WORKER_AUDIO_IPC_MAX_BODY_LEN,
    WORKER_AUDIO_IPC_VERSION, WorkerAudioIpcHeader, WorkerAudioIpcMessage, WorkerAudioMessageKind,
};
pub use worker_control_batch::{
    WORKER_CONTROL_IPC_MAX_BATCH_FRAMES, WorkerControlIpcBatch, WorkerControlIpcBatchRole,
};
pub use worker_control_ipc::{
    WORKER_CONTROL_IPC_HEADER_LEN, WORKER_CONTROL_IPC_MAGIC, WORKER_CONTROL_IPC_MAX_BODY_LEN,
    WORKER_CONTROL_IPC_SCHEMA_VERSION, WORKER_CONTROL_IPC_VERSION, WorkerControlIpcHeader,
    WorkerControlIpcMessage, WorkerControlMessageKind,
};
