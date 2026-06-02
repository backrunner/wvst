use std::sync::{Arc, Mutex};

use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameChannelCount, AudioFrameFlags, AudioFrameHeader,
    MIDI_EVENT_LEN, MidiEvent, MidiEventKind, WorkerAudioIpcMessage,
};
use wvst_vst3_host::Vst3InputEvent;

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

#[test]
fn maps_midi_note_events_to_vst3_input_events() {
    let header = test_header_with_events(2, 2);
    let events = [
        MidiEvent::new(0, MidiEventKind::NoteOn, 1, 60, 100).expect("note on"),
        MidiEvent::raw_midi(1, 0x80, 60, 64, 3).expect("raw note off"),
    ];
    let payload = midi_event_payload(&events);
    let mut destination = Vec::new();

    decode_midi_events_into(header, &payload, 2, &mut destination).expect("events");

    assert_eq!(destination.len(), 2);
    assert!(matches!(
        destination[0],
        Vst3InputEvent::NoteOn(event)
            if event.sample_offset == 0
                && event.channel == 1
                && event.pitch == 60
                && event.note_id == -1
    ));
    assert!(matches!(
        destination[1],
        Vst3InputEvent::NoteOff(event)
            if event.sample_offset == 1 && event.channel == 0 && event.pitch == 60
    ));
}

#[test]
fn rejects_midi_event_sample_offsets_outside_block() {
    let header = test_header_with_events(2, 1);
    let event = MidiEvent::new(2, MidiEventKind::NoteOn, 0, 60, 100).expect("note on");
    let payload = midi_event_payload(&[event]);
    let mut destination = Vec::new();

    let error =
        decode_midi_events_into(header, &payload, 2, &mut destination).expect_err("bad offset");

    assert!(error.contains("outside block"));
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

fn test_header_with_events(frames: u16, event_count: u16) -> AudioFrameHeader {
    AudioFrameHeader::new_f32(
        StreamId::new(9),
        11,
        512,
        SampleRate::new(48_000).expect("sample rate"),
        FrameCount::new(frames).expect("frames"),
        ChannelCount::new(2).expect("channels"),
        AudioFrameFlags::empty(),
    )
    .expect("header")
    .with_event_count(event_count)
    .expect("event header")
}

fn midi_event_payload(events: &[MidiEvent]) -> Vec<u8> {
    let mut payload = vec![0; events.len() * MIDI_EVENT_LEN];
    for (index, event) in events.iter().enumerate() {
        event
            .encode(&mut payload[index * MIDI_EVENT_LEN..])
            .expect("encode event");
    }
    payload
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
