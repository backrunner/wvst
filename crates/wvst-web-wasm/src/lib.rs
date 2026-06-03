use wasm_bindgen::prelude::*;
use wvst_core::{FrameCount, SampleRate, StreamId};
use wvst_protocol::{
    AUDIO_FRAME_HEADER_LEN, AUDIO_FRAME_VERSION, AudioFrameChannelCount, AudioFrameFlags,
    AudioFrameHeader, MIDI_EVENT_LEN, PARAMETER_AUTOMATION_EVENT_LEN, VST3_OUTPUT_EVENT_LEN,
};

#[wasm_bindgen(js_name = WVSTAudioFrameHeader)]
pub struct WasmAudioFrameHeader {
    header: AudioFrameHeader,
}

#[wasm_bindgen(js_class = WVSTAudioFrameHeader)]
impl WasmAudioFrameHeader {
    #[wasm_bindgen(getter, js_name = streamIdLow)]
    pub fn stream_id_low(&self) -> u32 {
        low_u32(self.header.stream_id.get())
    }

    #[wasm_bindgen(getter, js_name = streamIdHigh)]
    pub fn stream_id_high(&self) -> u32 {
        high_u32(self.header.stream_id.get())
    }

    #[wasm_bindgen(getter, js_name = sequenceLow)]
    pub fn sequence_low(&self) -> u32 {
        low_u32(self.header.sequence)
    }

    #[wasm_bindgen(getter, js_name = sequenceHigh)]
    pub fn sequence_high(&self) -> u32 {
        high_u32(self.header.sequence)
    }

    #[wasm_bindgen(getter, js_name = sentFrameTimeLow)]
    pub fn sent_frame_time_low(&self) -> u32 {
        low_u32(self.header.sent_frame_time)
    }

    #[wasm_bindgen(getter, js_name = sentFrameTimeHigh)]
    pub fn sent_frame_time_high(&self) -> u32 {
        high_u32(self.header.sent_frame_time)
    }

    #[wasm_bindgen(getter, js_name = sampleRate)]
    pub fn sample_rate(&self) -> u32 {
        self.header.sample_rate.get()
    }

    #[wasm_bindgen(getter, js_name = payloadBytes)]
    pub fn payload_bytes(&self) -> u32 {
        self.header.payload_len
    }

    #[wasm_bindgen(getter)]
    pub fn frames(&self) -> u16 {
        self.header.frames.get()
    }

    #[wasm_bindgen(getter)]
    pub fn channels(&self) -> u16 {
        self.header.channels.get()
    }

    #[wasm_bindgen(getter)]
    pub fn format(&self) -> u8 {
        self.header.format.into()
    }

    #[wasm_bindgen(getter)]
    pub fn flags(&self) -> u16 {
        self.header.flags.bits()
    }

    #[wasm_bindgen(getter, js_name = eventCount)]
    pub fn event_count(&self) -> u16 {
        self.header.event_count
    }

    #[wasm_bindgen(getter, js_name = parameterEventCount)]
    pub fn parameter_event_count(&self) -> u16 {
        self.header.parameter_event_count
    }

    #[wasm_bindgen(getter, js_name = vst3OutputEventCount)]
    pub fn vst3_output_event_count(&self) -> u16 {
        self.header.vst3_output_event_count
    }
}

#[wasm_bindgen(js_name = audioFrameHeaderBytes)]
pub fn audio_frame_header_bytes() -> u32 {
    AUDIO_FRAME_HEADER_LEN as u32
}

#[wasm_bindgen(js_name = audioFrameVersion)]
pub fn audio_frame_version() -> u32 {
    u32::from(AUDIO_FRAME_VERSION)
}

#[wasm_bindgen(js_name = midiEventBytes)]
pub fn midi_event_bytes() -> u32 {
    MIDI_EVENT_LEN as u32
}

#[wasm_bindgen(js_name = parameterAutomationEventBytes)]
pub fn parameter_automation_event_bytes() -> u32 {
    PARAMETER_AUTOMATION_EVENT_LEN as u32
}

#[wasm_bindgen(js_name = vst3OutputEventBytes)]
pub fn vst3_output_event_bytes() -> u32 {
    VST3_OUTPUT_EVENT_LEN as u32
}

#[wasm_bindgen(js_name = expectedAudioFramePayloadBytes)]
pub fn expected_audio_frame_payload_bytes(
    frames: u16,
    channels: u16,
    midi_event_count: u16,
    parameter_event_count: u16,
    vst3_output_event_count: u16,
) -> Result<u32, JsValue> {
    let header = build_header(
        0,
        0,
        0,
        48_000,
        frames,
        channels,
        0,
        midi_event_count,
        parameter_event_count,
        vst3_output_event_count,
    )
    .map_err(js_error)?;
    Ok(header.payload_len)
}

#[wasm_bindgen(js_name = encodeAudioFrameHeader)]
#[allow(clippy::too_many_arguments)]
pub fn encode_audio_frame_header(
    stream_id_low: u32,
    stream_id_high: u32,
    sequence_low: u32,
    sequence_high: u32,
    sent_frame_time_low: u32,
    sent_frame_time_high: u32,
    sample_rate: u32,
    frames: u16,
    channels: u16,
    flags: u16,
    midi_event_count: u16,
    parameter_event_count: u16,
    vst3_output_event_count: u16,
) -> Result<Vec<u8>, JsValue> {
    let header = build_header(
        join_u64(stream_id_low, stream_id_high),
        join_u64(sequence_low, sequence_high),
        join_u64(sent_frame_time_low, sent_frame_time_high),
        sample_rate,
        frames,
        channels,
        flags,
        midi_event_count,
        parameter_event_count,
        vst3_output_event_count,
    )
    .map_err(js_error)?;
    let mut bytes = vec![0; AUDIO_FRAME_HEADER_LEN];
    header.encode(&mut bytes).map_err(js_error)?;
    Ok(bytes)
}

#[wasm_bindgen(js_name = decodeAudioFrameHeader)]
pub fn decode_audio_frame_header(bytes: &[u8]) -> Result<WasmAudioFrameHeader, JsValue> {
    let header = AudioFrameHeader::decode(bytes).map_err(js_error)?;
    Ok(WasmAudioFrameHeader { header })
}

#[allow(clippy::too_many_arguments)]
fn build_header(
    stream_id: u64,
    sequence: u64,
    sent_frame_time: u64,
    sample_rate: u32,
    frames: u16,
    channels: u16,
    flags: u16,
    midi_event_count: u16,
    parameter_event_count: u16,
    vst3_output_event_count: u16,
) -> Result<AudioFrameHeader, String> {
    AudioFrameHeader::new_f32_with_audio_channels(
        StreamId::new(stream_id),
        sequence,
        sent_frame_time,
        SampleRate::new(sample_rate).map_err(|error| error.to_string())?,
        FrameCount::new(frames).map_err(|error| error.to_string())?,
        AudioFrameChannelCount::new(channels).map_err(|error| error.to_string())?,
        AudioFrameFlags::from_bits(flags),
    )
    .and_then(|header| {
        header.with_all_event_counts(
            midi_event_count,
            parameter_event_count,
            vst3_output_event_count,
        )
    })
    .map_err(|error| error.to_string())
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

const fn join_u64(low: u32, high: u32) -> u64 {
    (low as u64) | ((high as u64) << 32)
}

const fn low_u32(value: u64) -> u32 {
    value as u32
}

const fn high_u32(value: u64) -> u32 {
    (value >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_payload_sizes_with_protocol_rules() {
        assert_eq!(
            expected_audio_frame_payload_bytes(128, 2, 1, 1, 1).expect("payload size"),
            1_024 + 16 + 16 + 104
        );
        assert!(build_header(0, 0, 0, 48_000, 0, 2, 0, 0, 0, 0).is_err());
    }

    #[test]
    fn encodes_and_decodes_header_bytes() {
        let bytes = encode_audio_frame_header(7, 0, 9, 0, 128, 0, 48_000, 2, 2, 16, 1, 2, 3)
            .expect("header encodes");
        let header = decode_audio_frame_header(&bytes).expect("header decodes");

        assert_eq!(header.stream_id_low(), 7);
        assert_eq!(header.sequence_low(), 9);
        assert_eq!(header.sent_frame_time_low(), 128);
        assert_eq!(header.sample_rate(), 48_000);
        assert_eq!(header.payload_bytes(), 4 * 4 + 16 + 32 + 312);
        assert_eq!(header.flags(), 16);
        assert_eq!(header.event_count(), 1);
        assert_eq!(header.parameter_event_count(), 2);
        assert_eq!(header.vst3_output_event_count(), 3);
    }
}
