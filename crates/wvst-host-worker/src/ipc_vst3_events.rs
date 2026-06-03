use wvst_protocol::{
    VST3_OUTPUT_EVENT_LEN, Vst3OutputEvent as ProtocolVst3OutputEvent, Vst3OutputEventKind,
    Vst3OutputEventPayloadEncoding,
};
use wvst_vst3_host::{Vst3AdvancedOutputEvent, Vst3AdvancedOutputEventKind};

pub(super) fn encode_vst3_output_events_into(
    events: &[Vst3AdvancedOutputEvent],
    destination: &mut [u8],
) -> Result<(), String> {
    let expected_event_bytes = events
        .len()
        .checked_mul(VST3_OUTPUT_EVENT_LEN)
        .ok_or_else(|| "VST3 output event payload length overflow".to_string())?;
    if destination.len() != expected_event_bytes {
        return Err(format!(
            "VST3 output event payload length mismatch: expected {expected_event_bytes}, got {}",
            destination.len()
        ));
    }

    for (index, event) in events.iter().enumerate() {
        protocol_event(*event)?
            .encode(&mut destination[index * VST3_OUTPUT_EVENT_LEN..])
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn protocol_event(event: Vst3AdvancedOutputEvent) -> Result<ProtocolVst3OutputEvent, String> {
    Ok(ProtocolVst3OutputEvent {
        sample_offset: event.sample_offset,
        kind: protocol_event_kind(event.kind),
        vst3_event_type: event.vst3_event_type,
        bus_index: event.bus_index,
        data1: event.data1,
        data2: event.data2,
        value: event.value,
        data_size: event.data_size,
        data_type: event.data_type,
        payload_size: event.payload_size,
        payload_encoding: Vst3OutputEventPayloadEncoding::try_from(event.payload_encoding)
            .map_err(|error| error.to_string())?,
        payload_flags: event.payload_flags,
        payload: event.payload,
    })
}

fn protocol_event_kind(kind: Vst3AdvancedOutputEventKind) -> Vst3OutputEventKind {
    match kind {
        Vst3AdvancedOutputEventKind::Data => Vst3OutputEventKind::Data,
        Vst3AdvancedOutputEventKind::NoteExpressionValue => {
            Vst3OutputEventKind::NoteExpressionValue
        }
        Vst3AdvancedOutputEventKind::NoteExpressionText => Vst3OutputEventKind::NoteExpressionText,
        Vst3AdvancedOutputEventKind::NoteExpressionIntValue => {
            Vst3OutputEventKind::NoteExpressionIntValue
        }
        Vst3AdvancedOutputEventKind::Chord => Vst3OutputEventKind::Chord,
        Vst3AdvancedOutputEventKind::Scale => Vst3OutputEventKind::Scale,
    }
}
