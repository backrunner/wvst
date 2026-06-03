use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameFlags, AudioFrameHeader, MIDI_EVENT_LEN, MidiEvent,
    MidiEventKind,
};
use wvst_vst3_host::{
    VST3_MIDI_CONTROLLER_PITCH_BEND, VST3_MIDI_CONTROLLER_PROGRAM_CHANGE,
    VST3_MIDI_CONTROLLER_SONG_POINTER, Vst3LegacyMidiCcOutEvent, Vst3OutputEvent,
};

use super::encode_output_frame_into;

#[test]
fn encodes_vst3_legacy_midi_cc_output_events() {
    let header = test_header(2);
    let output = [0.0_f32, 0.0, 0.0, 0.0];
    let mut events = [
        Vst3OutputEvent::LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent {
            sample_offset: 1,
            control_number: VST3_MIDI_CONTROLLER_PROGRAM_CHANGE as u8,
            channel: 3,
            value: 11,
            value2: 0,
        }),
        Vst3OutputEvent::LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent {
            sample_offset: 0,
            control_number: 7,
            channel: 2,
            value: 100,
            value2: 0,
        }),
        Vst3OutputEvent::LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent {
            sample_offset: 1,
            control_number: VST3_MIDI_CONTROLLER_PITCH_BEND as u8,
            channel: 2,
            value: 0,
            value2: 64,
        }),
        Vst3OutputEvent::LegacyMidiCcOut(Vst3LegacyMidiCcOutEvent {
            sample_offset: 1,
            control_number: VST3_MIDI_CONTROLLER_SONG_POINTER as u8,
            channel: 0,
            value: 1,
            value2: 2,
        }),
    ];
    let mut frame = Vec::new();

    encode_output_frame_into(
        header,
        2,
        &output,
        &mut events,
        &mut [],
        &mut [],
        &mut frame,
    )
    .expect("encoded");

    let response_header = AudioFrameHeader::decode(&frame).expect("response header");
    assert_eq!(response_header.event_count, 4);
    let audio_end = AUDIO_FRAME_HEADER_LEN + response_header.audio_payload_len().unwrap() as usize;
    let midi_end = audio_end + response_header.midi_event_payload_len().unwrap() as usize;
    let decoded: Vec<_> = frame[audio_end..midi_end]
        .chunks_exact(MIDI_EVENT_LEN)
        .map(|chunk| MidiEvent::decode(chunk).expect("midi event"))
        .collect();

    assert_eq!(
        decoded,
        vec![
            MidiEvent::new(0, MidiEventKind::ControlChange, 2, 7, 100).expect("cc"),
            MidiEvent::raw_midi(1, 0xc3, 11, 0, 2).expect("program change"),
            MidiEvent::new(1, MidiEventKind::PitchBend, 2, 0, 64).expect("pitch bend"),
            MidiEvent::raw_midi(1, 0xf2, 1, 2, 3).expect("song pointer"),
        ]
    );
}

fn test_header(frames: u16) -> AudioFrameHeader {
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
}
