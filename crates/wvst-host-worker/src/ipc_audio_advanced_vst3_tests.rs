use wvst_core::{ChannelCount, FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AudioFrameFlags, AudioFrameHeader, VST3_OUTPUT_EVENT_LEN,
    Vst3OutputEvent as ProtocolVst3OutputEvent, Vst3OutputEventKind,
};
use wvst_vst3_host::{Vst3AdvancedOutputEvent, Vst3AdvancedOutputEventKind};

use super::encode_output_frame_into;

#[test]
fn encodes_advanced_vst3_output_events_after_audio_midi_and_parameters() {
    let header = test_header(2);
    let output = [0.0_f32, 0.0, 0.0, 0.0];
    let mut advanced_events = [
        Vst3AdvancedOutputEvent {
            sample_offset: 1,
            kind: Vst3AdvancedOutputEventKind::Scale,
            vst3_event_type: 7,
            bus_index: 2,
            data1: 0,
            data2: 0,
            value: 0.0,
            data_size: 0,
            data_type: 0,
            ..Vst3AdvancedOutputEvent::default()
        },
        Vst3AdvancedOutputEvent {
            sample_offset: 1,
            kind: Vst3AdvancedOutputEventKind::NoteExpressionIntValue,
            vst3_event_type: 8,
            bus_index: 3,
            data1: 12,
            data2: 123,
            value: 0.0,
            data_size: 0,
            data_type: 44,
            ..Vst3AdvancedOutputEvent::default()
        },
        Vst3AdvancedOutputEvent {
            sample_offset: 0,
            kind: Vst3AdvancedOutputEventKind::Data,
            vst3_event_type: 2,
            bus_index: 1,
            data1: 0,
            data2: 0,
            value: 0.0,
            data_size: 128,
            data_type: 9,
            ..Vst3AdvancedOutputEvent::default()
        },
    ];
    let mut frame = Vec::new();

    encode_output_frame_into(
        header,
        2,
        &output,
        &mut [],
        &mut advanced_events,
        &mut [],
        &mut frame,
    )
    .expect("encoded");

    let response_header = AudioFrameHeader::decode(&frame).expect("response header");
    assert_eq!(response_header.event_count, 0);
    assert_eq!(response_header.parameter_event_count, 0);
    assert_eq!(response_header.vst3_output_event_count, 3);
    let audio_end = AUDIO_FRAME_HEADER_LEN + response_header.audio_payload_len().unwrap() as usize;
    let vst3_end = audio_end
        + response_header
            .vst3_output_event_payload_len()
            .expect("vst3 payload") as usize;

    assert_eq!(vst3_end, frame.len());
    assert_eq!(
        ProtocolVst3OutputEvent::decode(&frame[audio_end..audio_end + VST3_OUTPUT_EVENT_LEN])
            .expect("data event"),
        ProtocolVst3OutputEvent {
            sample_offset: 0,
            kind: Vst3OutputEventKind::Data,
            vst3_event_type: 2,
            bus_index: 1,
            data1: 0,
            data2: 0,
            value: 0.0,
            data_size: 128,
            data_type: 9,
            ..ProtocolVst3OutputEvent::default()
        }
    );
    assert_eq!(
        ProtocolVst3OutputEvent::decode(&frame[audio_end + VST3_OUTPUT_EVENT_LEN..vst3_end])
            .expect("scale event")
            .kind,
        Vst3OutputEventKind::Scale
    );
    assert_eq!(
        ProtocolVst3OutputEvent::decode(&frame[audio_end + (VST3_OUTPUT_EVENT_LEN * 2)..vst3_end])
            .expect("note expression int event"),
        ProtocolVst3OutputEvent {
            sample_offset: 1,
            kind: Vst3OutputEventKind::NoteExpressionIntValue,
            vst3_event_type: 8,
            bus_index: 3,
            data1: 12,
            data2: 123,
            value: 0.0,
            data_size: 0,
            data_type: 44,
            ..ProtocolVst3OutputEvent::default()
        }
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
