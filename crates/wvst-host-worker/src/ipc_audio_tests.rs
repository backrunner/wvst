use std::sync::{Arc, Mutex};

use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameChannelCount, AudioFrameFlags, AudioFrameHeader, MidiEvent,
    MidiEventKind, WorkerAudioIpcMessage,
};

use super::*;

#[test]
fn processes_audio_frame_for_registered_stream() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);
    start_processing(&mut state);

    let request =
        WorkerAudioIpcMessage::process_request(11, audio_frame(9)).expect("request message");
    let state = Arc::new(Mutex::new(state));
    let response = process_message(request, &state).expect("processed");
    let header = AudioFrameHeader::decode(&response).expect("response header");

    assert_eq!(header.stream_id.get(), 9);
    assert_eq!(
        read_f32_payload(&response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.1, 0.2, 0.3, 0.4]
    );
}

#[test]
fn processes_zero_input_instrument_audio_frame() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 0, 2);
    start_processing(&mut state);

    let request = WorkerAudioIpcMessage::process_request(11, zero_input_audio_frame(9))
        .expect("request message");
    let state = Arc::new(Mutex::new(state));
    let response = process_message(request, &state).expect("processed");
    let header = AudioFrameHeader::decode(&response).expect("response header");

    assert_eq!(header.channels.get(), 2);
    assert_eq!(
        read_f32_payload(&response[AUDIO_FRAME_HEADER_LEN..]).expect("payload"),
        vec![0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn rejects_audio_frame_before_processing_starts() {
    let mut state = WorkerIpcState::default();
    create_instance(&mut state, 2, 2);

    let request =
        WorkerAudioIpcMessage::process_request(11, audio_frame(9)).expect("request message");
    let state = Arc::new(Mutex::new(state));
    let error = process_message(request, &state).expect_err("not processing");

    assert_eq!(error.status_code, AUDIO_ERROR_INVALID_REQUEST);
    assert!(error.message.contains("not processing"));
}

fn create_instance(state: &mut WorkerIpcState, input_channels: usize, output_channels: usize) {
    let request = format!(
        r#"{{"id":1,"method":"instance.create","params":{{"instanceId":7,"streamId":9,"pluginId":"vst3:test","pluginPath":"/tmp/Test.vst3","classId":"class-a","className":"Test","sampleRate":48000,"maxBlockFrames":128,"inputChannels":{input_channels},"outputChannels":{output_channels}}}}}"#,
    );
    let response = super::super::handle_ipc_line(&request, state);

    assert!(response.contains(r#""result""#), "{response}");
}

fn start_processing(state: &mut WorkerIpcState) {
    let response = super::super::handle_ipc_line(
        r#"{"id":2,"method":"instance.startProcessing","params":{"instanceId":7}}"#,
        state,
    );

    assert!(response.contains(r#""result""#), "{response}");
}

fn audio_frame(stream_id: u64) -> Vec<u8> {
    let samples = [0.1_f32, 0.2, 0.3, 0.4];
    let header = AudioFrameHeader::new_f32(
        StreamId::new(stream_id),
        11,
        512,
        SampleRate::new(48_000).expect("sample rate"),
        FrameCount::new(2).expect("frames"),
        ChannelCount::new(2).expect("channels"),
        AudioFrameFlags::empty(),
    )
    .expect("header");
    let mut frame = vec![0; AUDIO_FRAME_HEADER_LEN + header.payload_len as usize];
    header
        .encode(&mut frame[..AUDIO_FRAME_HEADER_LEN])
        .expect("encode header");
    let mut offset = AUDIO_FRAME_HEADER_LEN;
    for sample in samples {
        frame[offset..offset + 4].copy_from_slice(&sample.to_le_bytes());
        offset += 4;
    }
    frame
}

fn zero_input_audio_frame(stream_id: u64) -> Vec<u8> {
    let header = AudioFrameHeader::new_f32_with_audio_channels(
        StreamId::new(stream_id),
        11,
        512,
        SampleRate::new(48_000).expect("sample rate"),
        FrameCount::new(2).expect("frames"),
        AudioFrameChannelCount::new(0).expect("zero channels"),
        AudioFrameFlags::empty(),
    )
    .expect("header")
    .with_event_count(1)
    .expect("event header");
    let mut frame = vec![0; AUDIO_FRAME_HEADER_LEN + header.payload_len as usize];
    header
        .encode(&mut frame[..AUDIO_FRAME_HEADER_LEN])
        .expect("encode header");
    let event = MidiEvent::new(0, MidiEventKind::NoteOn, 0, 60, 100).expect("MIDI event");
    event
        .encode(&mut frame[AUDIO_FRAME_HEADER_LEN..])
        .expect("encode event");
    frame
}
