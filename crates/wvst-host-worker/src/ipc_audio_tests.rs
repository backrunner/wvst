use std::io::Write;
use std::sync::{Arc, Mutex};

use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameChannelCount, AudioFrameFlags, AudioFrameHeader,
    MIDI_EVENT_LEN, MidiEvent, MidiEventKind, PARAMETER_AUTOMATION_EVENT_LEN,
    ParameterAutomationEvent, WORKER_AUDIO_IPC_HEADER_LEN, WORKER_AUDIO_IPC_MAX_BODY_LEN,
    WorkerAudioIpcHeader, WorkerAudioIpcMessage, WorkerAudioMessageKind,
};
use wvst_vst3_host::{
    VST3_MIDI_CONTROLLER_PITCH_BEND, Vst3InputEvent, Vst3NoteEvent, Vst3OutputEvent,
    Vst3ParameterChange, Vst3PolyPressureEvent,
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

#[test]
fn encodes_structured_process_error_body() {
    let error = AudioProcessError {
        status_code: AUDIO_ERROR_INVALID_REQUEST,
        message: "VST3 process failed".to_string(),
        data: Some(serde_json::json!({
            "kind": "vst3-runtime-process",
            "stage": "component.process",
            "hostError": "audio-processor-call-failed",
            "message": "VST3 process failed"
        })),
    };
    let body: serde_json::Value = serde_json::from_str(&error.body()).expect("json body");

    assert_eq!(body["message"], "VST3 process failed");
    assert_eq!(body["data"]["kind"], "vst3-runtime-process");
    assert_eq!(body["data"]["stage"], "component.process");
}

#[test]
fn rejects_oversized_audio_ipc_request_bodies_before_allocation() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let client = std::thread::spawn(move || {
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        let header = WorkerAudioIpcHeader::new(
            WorkerAudioMessageKind::ProcessRequest,
            0,
            77,
            WORKER_AUDIO_IPC_MAX_BODY_LEN + 1,
        );
        let mut header_bytes = [0; WORKER_AUDIO_IPC_HEADER_LEN];
        header.encode(&mut header_bytes).expect("encode header");
        stream
            .write_all(&header_bytes)
            .expect("write oversized header");
    });
    let (mut stream, _) = listener.accept().expect("accept");
    let mut body = Vec::new();

    let error = read_message_into(&mut stream, &mut body).expect_err("oversized body");

    client.join().expect("client thread");
    assert!(error.contains("worker audio body too large"));
    assert!(body.is_empty());
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
    let mut parameter_changes = Vec::new();

    decode_midi_events_into(
        header,
        &payload,
        2,
        &mut destination,
        &mut parameter_changes,
        |_, _| None,
    )
    .expect("events");

    assert_eq!(destination.len(), 2);
    assert!(parameter_changes.is_empty());
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
fn sorts_midi_note_events_by_sample_offset() {
    let header = test_header_with_events(8, 3);
    let events = [
        MidiEvent::new(6, MidiEventKind::NoteOn, 1, 60, 100).expect("note on"),
        MidiEvent::new(1, MidiEventKind::NoteOn, 1, 62, 100).expect("note on"),
        MidiEvent::raw_midi(4, 0x80, 60, 64, 3).expect("raw note off"),
    ];
    let payload = midi_event_payload(&events);
    let mut destination = Vec::new();
    let mut parameter_changes = Vec::new();

    decode_midi_events_into(
        header,
        &payload,
        8,
        &mut destination,
        &mut parameter_changes,
        |_, _| None,
    )
    .expect("events");

    assert_eq!(
        destination,
        vec![
            Vst3InputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 1,
                channel: 1,
                pitch: 62,
                velocity: 100.0 / 127.0,
                note_id: -1,
            }),
            Vst3InputEvent::NoteOff(Vst3NoteEvent {
                sample_offset: 4,
                channel: 0,
                pitch: 60,
                velocity: 64.0 / 127.0,
                note_id: -1,
            }),
            Vst3InputEvent::NoteOn(Vst3NoteEvent {
                sample_offset: 6,
                channel: 1,
                pitch: 60,
                velocity: 100.0 / 127.0,
                note_id: -1,
            }),
        ]
    );
}

#[test]
fn rejects_midi_event_sample_offsets_outside_block() {
    let header = test_header_with_events(2, 1);
    let event = MidiEvent::new(2, MidiEventKind::NoteOn, 0, 60, 100).expect("note on");
    let payload = midi_event_payload(&[event]);
    let mut destination = Vec::new();
    let mut parameter_changes = Vec::new();

    let error = decode_midi_events_into(
        header,
        &payload,
        2,
        &mut destination,
        &mut parameter_changes,
        |_, _| None,
    )
    .expect_err("bad offset");

    assert!(error.contains("outside block"));
}

#[test]
fn decodes_parameter_automation_events() {
    let header = test_header_with_event_counts(8, 0, 2);
    let events = [
        ParameterAutomationEvent::new(0, 10, 0.25).expect("parameter event"),
        ParameterAutomationEvent::new(7, 11, 0.75).expect("parameter event"),
    ];
    let payload = parameter_event_payload(&events);
    let mut destination = Vec::new();

    decode_parameter_events_into(header, &payload, 8, &mut destination).expect("parameter events");

    assert_eq!(
        destination,
        vec![
            Vst3ParameterChange {
                sample_offset: 0,
                parameter_id: 10,
                value_normalized: 0.25,
            },
            Vst3ParameterChange {
                sample_offset: 7,
                parameter_id: 11,
                value_normalized: 0.75,
            },
        ]
    );
}

#[test]
fn sorts_parameter_automation_events_by_sample_offset() {
    let header = test_header_with_event_counts(8, 0, 3);
    let events = [
        ParameterAutomationEvent::new(7, 12, 0.75).expect("parameter event"),
        ParameterAutomationEvent::new(1, 20, 0.25).expect("parameter event"),
        ParameterAutomationEvent::new(1, 10, 0.5).expect("parameter event"),
    ];
    let payload = parameter_event_payload(&events);
    let mut destination = Vec::new();

    decode_parameter_events_into(header, &payload, 8, &mut destination).expect("parameter events");

    assert_eq!(
        destination,
        vec![
            Vst3ParameterChange {
                sample_offset: 1,
                parameter_id: 10,
                value_normalized: 0.5,
            },
            Vst3ParameterChange {
                sample_offset: 1,
                parameter_id: 20,
                value_normalized: 0.25,
            },
            Vst3ParameterChange {
                sample_offset: 7,
                parameter_id: 12,
                value_normalized: 0.75,
            },
        ]
    );
}

#[test]
fn maps_midi_controller_events_to_parameter_changes() {
    let header = test_header_with_events(8, 2);
    let events = [
        MidiEvent::new(3, MidiEventKind::ControlChange, 2, 7, 64).expect("cc"),
        MidiEvent::new(4, MidiEventKind::PitchBend, 2, 0, 64).expect("pitch bend"),
    ];
    let payload = midi_event_payload(&events);
    let mut destination = Vec::new();
    let mut parameter_changes = Vec::new();

    decode_midi_events_into(
        header,
        &payload,
        8,
        &mut destination,
        &mut parameter_changes,
        |channel, controller| match (channel, controller) {
            (2, 7) => Some(100),
            (2, VST3_MIDI_CONTROLLER_PITCH_BEND) => Some(101),
            _ => None,
        },
    )
    .expect("events");

    assert!(destination.is_empty());
    assert_eq!(parameter_changes.len(), 2);
    assert_eq!(parameter_changes[0].sample_offset, 3);
    assert_eq!(parameter_changes[0].parameter_id, 100);
    assert!((parameter_changes[0].value_normalized - (64.0 / 127.0)).abs() < f64::EPSILON);
    assert_eq!(parameter_changes[1].sample_offset, 4);
    assert_eq!(parameter_changes[1].parameter_id, 101);
    assert!((parameter_changes[1].value_normalized - (8192.0 / 16_383.0)).abs() < f64::EPSILON);
}

#[test]
fn sorts_midi_mapped_parameter_changes_with_existing_automation() {
    let header = test_header_with_events(8, 2);
    let events = [
        MidiEvent::new(6, MidiEventKind::PitchBend, 2, 0, 64).expect("pitch bend"),
        MidiEvent::new(2, MidiEventKind::ControlChange, 2, 7, 64).expect("cc"),
    ];
    let payload = midi_event_payload(&events);
    let mut destination = Vec::new();
    let mut parameter_changes = vec![Vst3ParameterChange {
        sample_offset: 4,
        parameter_id: 50,
        value_normalized: 0.5,
    }];

    decode_midi_events_into(
        header,
        &payload,
        8,
        &mut destination,
        &mut parameter_changes,
        |channel, controller| match (channel, controller) {
            (2, 7) => Some(100),
            (2, VST3_MIDI_CONTROLLER_PITCH_BEND) => Some(101),
            _ => None,
        },
    )
    .expect("events");

    assert!(destination.is_empty());
    assert_eq!(parameter_changes[0].sample_offset, 2);
    assert_eq!(parameter_changes[0].parameter_id, 100);
    assert_eq!(parameter_changes[1].sample_offset, 4);
    assert_eq!(parameter_changes[1].parameter_id, 50);
    assert_eq!(parameter_changes[2].sample_offset, 6);
    assert_eq!(parameter_changes[2].parameter_id, 101);
}

#[test]
fn encodes_output_audio_midi_and_parameter_events() {
    let header = test_header_with_event_counts(2, 1, 1);
    let output = [0.1_f32, 0.2, 0.3, 0.4];
    let mut events = [
        Vst3OutputEvent::PolyPressure(Vst3PolyPressureEvent {
            sample_offset: 1,
            channel: 1,
            pitch: 60,
            pressure: 64.0 / 127.0,
            note_id: 12,
        }),
        Vst3OutputEvent::NoteOn(Vst3NoteEvent {
            sample_offset: 0,
            channel: 1,
            pitch: 60,
            velocity: 100.0 / 127.0,
            note_id: 12,
        }),
    ];
    let mut parameter_changes = [
        Vst3ParameterChange {
            sample_offset: 1,
            parameter_id: 42,
            value_normalized: 0.75,
        },
        Vst3ParameterChange {
            sample_offset: 0,
            parameter_id: 99,
            value_normalized: 0.25,
        },
    ];
    let mut frame = Vec::new();

    encode_output_frame_into(
        header,
        2,
        &output,
        &mut events,
        &mut [],
        &mut parameter_changes,
        &mut frame,
    )
    .expect("encoded");

    let response_header = AudioFrameHeader::decode(&frame).expect("response header");
    assert_eq!(response_header.event_count, 2);
    assert_eq!(response_header.parameter_event_count, 2);
    let audio_end = AUDIO_FRAME_HEADER_LEN + response_header.audio_payload_len().unwrap() as usize;
    let midi_end = audio_end + response_header.midi_event_payload_len().unwrap() as usize;
    let parameter_end = midi_end + response_header.parameter_event_payload_len().unwrap() as usize;
    assert_eq!(parameter_end, frame.len());
    assert_eq!(
        read_f32_payload(&frame[AUDIO_FRAME_HEADER_LEN..audio_end]).expect("payload"),
        output.to_vec()
    );
    assert_eq!(
        MidiEvent::decode(&frame[audio_end..audio_end + MIDI_EVENT_LEN]).expect("note on"),
        {
            let mut event = MidiEvent::new(0, MidiEventKind::NoteOn, 1, 60, 100).expect("event");
            event.note_id = 12;
            event
        }
    );
    assert_eq!(
        MidiEvent::decode(&frame[audio_end + MIDI_EVENT_LEN..midi_end]).expect("poly pressure"),
        {
            let mut event =
                MidiEvent::new(1, MidiEventKind::PolyAftertouch, 1, 60, 64).expect("event");
            event.note_id = 12;
            event
        }
    );
    assert_eq!(
        ParameterAutomationEvent::decode(
            &frame[midi_end..midi_end + PARAMETER_AUTOMATION_EVENT_LEN]
        )
        .expect("parameter"),
        ParameterAutomationEvent::new(0, 99, 0.25).expect("parameter")
    );
    assert_eq!(
        ParameterAutomationEvent::decode(
            &frame[midi_end + PARAMETER_AUTOMATION_EVENT_LEN..parameter_end]
        )
        .expect("parameter"),
        ParameterAutomationEvent::new(1, 42, 0.75).expect("parameter")
    );
}

fn create_instance(state: &mut WorkerIpcState, input_channels: usize, output_channels: usize) {
    super::super::insert_test_audio_instance(state, input_channels, output_channels);
}

fn start_processing(state: &mut WorkerIpcState) {
    let response = super::super::handle_ipc_line(
        r#"{"id":2,"method":"instance.startProcessing","params":{"instanceId":7}}"#,
        state,
    );

    assert!(response.contains(r#""result""#), "{response}");
}

fn test_header_with_events(frames: u16, event_count: u16) -> AudioFrameHeader {
    test_header_with_event_counts(frames, event_count, 0)
}

fn test_header_with_event_counts(
    frames: u16,
    event_count: u16,
    parameter_event_count: u16,
) -> AudioFrameHeader {
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
    .with_event_counts(event_count, parameter_event_count)
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

fn parameter_event_payload(events: &[ParameterAutomationEvent]) -> Vec<u8> {
    let mut payload = vec![0; events.len() * PARAMETER_AUTOMATION_EVENT_LEN];
    for (index, event) in events.iter().enumerate() {
        event
            .encode(&mut payload[index * PARAMETER_AUTOMATION_EVENT_LEN..])
            .expect("encode parameter event");
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
