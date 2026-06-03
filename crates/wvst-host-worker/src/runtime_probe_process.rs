use std::fmt::Write as _;

use serde::Serialize;
use wvst_vst3_host::{
    VST3_OUTPUT_PAYLOAD_ENCODING_UTF8, Vst3AdvancedOutputEvent, Vst3AdvancedOutputEventKind,
    Vst3InputEvent, Vst3OutputEventStats, Vst3OutputParameterChangeStats, Vst3ProcessOutput,
    Vst3ProcessOutputDiagnostics,
};

const MAX_ADVANCED_OUTPUT_EVENT_EXAMPLES: usize = 16;

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeProbeProcessSummary {
    total_blocks: u32,
    total_frames: u64,
    input_samples: u64,
    output_samples: u64,
    finite_output_samples: u64,
    non_finite_output_samples: u64,
    clipped_output_samples: u64,
    output_energy: f64,
    output_rms: f64,
    max_output_peak: f32,
    max_output_peak_block: Option<u32>,
    non_zero_output_blocks: u32,
    silent_output_blocks: u32,
    first_non_zero_output_block: Option<u32>,
    first_non_zero_output_frame: Option<u16>,
    first_non_zero_output_absolute_frame: Option<u64>,
    output_events: u64,
    output_parameter_changes: u64,
    diagnostics: Vst3ProcessOutputDiagnostics,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    advanced_output_event_examples: Vec<RuntimeProbeAdvancedOutputEventExample>,
    process_time_micros: ProbeTimingSummary,
    note_timing: RuntimeProbeNoteTimingSummary,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeNoteTimingSummary {
    sample_rate_hz: u32,
    note_present: bool,
    note_on_block: Option<u32>,
    note_on_sample_offset: Option<u16>,
    note_on_absolute_frame: Option<u64>,
    note_off_block: Option<u32>,
    note_off_sample_offset: Option<u16>,
    note_off_absolute_frame: Option<u64>,
    first_non_zero_output_block: Option<u32>,
    first_non_zero_output_frame: Option<u16>,
    first_non_zero_output_absolute_frame: Option<u64>,
    frames_from_note_on_to_first_non_zero_output: Option<i64>,
    micros_from_note_on_to_first_non_zero_output: Option<i64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeTimingSummary {
    min: u64,
    p50: u64,
    p95: u64,
    p99: u64,
    max: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeAdvancedOutputEventExample {
    block_index: u32,
    sample_offset: u16,
    absolute_frame: u64,
    kind: &'static str,
    vst3_event_type: u16,
    bus_index: i32,
    data1: i32,
    data2: i32,
    value: f64,
    data_size: u32,
    data_type: u32,
    payload_size: u16,
    payload_encoding: u8,
    payload_flags: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_text: Option<String>,
}

impl RuntimeProbeProcessSummary {
    pub(crate) fn new(
        total_blocks: u32,
        frames: u16,
        input_len: usize,
        output_len: usize,
        sample_rate_hz: u32,
        note_present: bool,
    ) -> Self {
        Self {
            total_blocks,
            total_frames: u64::from(total_blocks) * u64::from(frames),
            input_samples: input_len as u64 * u64::from(total_blocks),
            output_samples: output_len as u64 * u64::from(total_blocks),
            note_timing: RuntimeProbeNoteTimingSummary::new(sample_rate_hz, note_present),
            ..Self::default()
        }
    }

    pub(crate) fn observe_input_events(
        &mut self,
        block_index: u32,
        block_frames: u16,
        events: &[Vst3InputEvent],
    ) {
        self.note_timing
            .observe_input_events(block_index, block_frames, events);
    }

    pub(crate) fn observe_output_block(
        &mut self,
        block_index: u32,
        block_frames: u16,
        output_channels: u16,
        output: &[f32],
    ) {
        let mut block_peak = 0.0_f32;
        let mut block_non_finite = 0_u64;
        let mut block_finite = 0_u64;
        let mut block_energy = 0.0_f64;
        let mut block_clipped = 0_u64;
        let first_non_zero_frame = first_non_zero_frame(output, usize::from(output_channels));

        for sample in output {
            if sample.is_finite() {
                let abs = sample.abs();
                block_peak = block_peak.max(abs);
                block_energy += f64::from(*sample) * f64::from(*sample);
                block_finite = block_finite.saturating_add(1);
                if abs > 1.0 {
                    block_clipped = block_clipped.saturating_add(1);
                }
            } else {
                block_non_finite = block_non_finite.saturating_add(1);
            }
        }

        self.finite_output_samples = self.finite_output_samples.saturating_add(block_finite);
        self.non_finite_output_samples = self
            .non_finite_output_samples
            .saturating_add(block_non_finite);
        self.clipped_output_samples = self.clipped_output_samples.saturating_add(block_clipped);
        self.output_energy += block_energy;

        if !output.is_empty() && block_peak == 0.0 && block_non_finite == 0 {
            self.silent_output_blocks = self.silent_output_blocks.saturating_add(1);
        }

        if block_peak > 0.0 {
            self.non_zero_output_blocks = self.non_zero_output_blocks.saturating_add(1);
            if self.first_non_zero_output_block.is_none() {
                self.first_non_zero_output_block = Some(block_index);
                self.first_non_zero_output_frame = first_non_zero_frame;
                self.first_non_zero_output_absolute_frame = first_non_zero_frame
                    .map(|frame| absolute_frame(block_index, block_frames, frame));
            }
            if block_peak > self.max_output_peak {
                self.max_output_peak = block_peak;
                self.max_output_peak_block = Some(block_index);
            }
        }

        if let Some(frame) = first_non_zero_frame {
            self.note_timing
                .observe_first_non_zero_output(block_index, block_frames, frame);
        }
    }

    pub(crate) fn observe_process_output(
        &mut self,
        block_index: u32,
        block_frames: u16,
        process_output: &Vst3ProcessOutput,
    ) {
        self.output_events = self
            .output_events
            .saturating_add(process_output.events.len() as u64)
            .saturating_add(process_output.advanced_events.len() as u64);
        self.output_parameter_changes = self
            .output_parameter_changes
            .saturating_add(process_output.parameter_changes.len() as u64);
        self.observe_advanced_output_event_examples(
            block_index,
            block_frames,
            &process_output.advanced_events,
        );
        merge_process_diagnostics(&mut self.diagnostics, process_output.diagnostics);
    }

    fn observe_advanced_output_event_examples(
        &mut self,
        block_index: u32,
        block_frames: u16,
        events: &[Vst3AdvancedOutputEvent],
    ) {
        if self.advanced_output_event_examples.len() >= MAX_ADVANCED_OUTPUT_EVENT_EXAMPLES {
            return;
        }
        let available =
            MAX_ADVANCED_OUTPUT_EVENT_EXAMPLES - self.advanced_output_event_examples.len();
        self.advanced_output_event_examples.extend(
            events
                .iter()
                .take(available)
                .map(|event| advanced_output_event_example(block_index, block_frames, *event)),
        );
    }

    pub(crate) fn finish(&mut self, process_micros: Vec<u64>) {
        self.process_time_micros = timing_summary(process_micros);
        self.output_rms = output_rms(self.output_energy, self.finite_output_samples);
    }
}

impl RuntimeProbeNoteTimingSummary {
    const fn new(sample_rate_hz: u32, note_present: bool) -> Self {
        Self {
            sample_rate_hz,
            note_present,
            note_on_block: None,
            note_on_sample_offset: None,
            note_on_absolute_frame: None,
            note_off_block: None,
            note_off_sample_offset: None,
            note_off_absolute_frame: None,
            first_non_zero_output_block: None,
            first_non_zero_output_frame: None,
            first_non_zero_output_absolute_frame: None,
            frames_from_note_on_to_first_non_zero_output: None,
            micros_from_note_on_to_first_non_zero_output: None,
        }
    }

    fn observe_input_events(
        &mut self,
        block_index: u32,
        block_frames: u16,
        events: &[Vst3InputEvent],
    ) {
        for event in events {
            match event {
                Vst3InputEvent::NoteOn(note) if self.note_on_block.is_none() => {
                    self.note_on_block = Some(block_index);
                    self.note_on_sample_offset = Some(note.sample_offset);
                    self.note_on_absolute_frame = Some(absolute_frame(
                        block_index,
                        block_frames,
                        note.sample_offset,
                    ));
                }
                Vst3InputEvent::NoteOff(note) if self.note_off_block.is_none() => {
                    self.note_off_block = Some(block_index);
                    self.note_off_sample_offset = Some(note.sample_offset);
                    self.note_off_absolute_frame = Some(absolute_frame(
                        block_index,
                        block_frames,
                        note.sample_offset,
                    ));
                }
                _ => {}
            }
        }
        self.refresh_latency();
    }

    fn observe_first_non_zero_output(&mut self, block_index: u32, block_frames: u16, frame: u16) {
        if self.first_non_zero_output_block.is_some() {
            return;
        }
        self.first_non_zero_output_block = Some(block_index);
        self.first_non_zero_output_frame = Some(frame);
        self.first_non_zero_output_absolute_frame =
            Some(absolute_frame(block_index, block_frames, frame));
        self.refresh_latency();
    }

    fn refresh_latency(&mut self) {
        let (Some(note_frame), Some(output_frame)) = (
            self.note_on_absolute_frame,
            self.first_non_zero_output_absolute_frame,
        ) else {
            return;
        };
        let frames = output_frame as i128 - note_frame as i128;
        let frames = frames.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
        self.frames_from_note_on_to_first_non_zero_output = Some(frames);
        self.micros_from_note_on_to_first_non_zero_output = (self.sample_rate_hz > 0)
            .then(|| frames.saturating_mul(1_000_000) / i64::from(self.sample_rate_hz));
    }
}

fn advanced_output_event_example(
    block_index: u32,
    block_frames: u16,
    event: Vst3AdvancedOutputEvent,
) -> RuntimeProbeAdvancedOutputEventExample {
    RuntimeProbeAdvancedOutputEventExample {
        block_index,
        sample_offset: event.sample_offset,
        absolute_frame: absolute_frame(block_index, block_frames, event.sample_offset),
        kind: advanced_output_event_kind_name(event.kind),
        vst3_event_type: event.vst3_event_type,
        bus_index: event.bus_index,
        data1: event.data1,
        data2: event.data2,
        value: event.value,
        data_size: event.data_size,
        data_type: event.data_type,
        payload_size: event.payload_size,
        payload_encoding: event.payload_encoding,
        payload_flags: event.payload_flags,
        payload_hex: advanced_payload_hex(event),
        payload_text: advanced_payload_text(event),
    }
}

fn advanced_payload_hex(event: Vst3AdvancedOutputEvent) -> Option<String> {
    let bytes = event.payload_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }
    Some(output)
}

fn advanced_payload_text(event: Vst3AdvancedOutputEvent) -> Option<String> {
    if event.payload_encoding != VST3_OUTPUT_PAYLOAD_ENCODING_UTF8 {
        return None;
    }
    std::str::from_utf8(event.payload_bytes())
        .ok()
        .map(ToOwned::to_owned)
}

fn advanced_output_event_kind_name(kind: Vst3AdvancedOutputEventKind) -> &'static str {
    match kind {
        Vst3AdvancedOutputEventKind::Data => "data",
        Vst3AdvancedOutputEventKind::NoteExpressionValue => "note-expression-value",
        Vst3AdvancedOutputEventKind::NoteExpressionText => "note-expression-text",
        Vst3AdvancedOutputEventKind::NoteExpressionIntValue => "note-expression-int-value",
        Vst3AdvancedOutputEventKind::Chord => "chord",
        Vst3AdvancedOutputEventKind::Scale => "scale",
    }
}

fn first_non_zero_frame(output: &[f32], output_channels: usize) -> Option<u16> {
    if output_channels == 0 {
        return None;
    }
    output
        .iter()
        .position(|sample| sample.is_finite() && sample.abs() > 0.0)
        .and_then(|index| u16::try_from(index / output_channels).ok())
}

fn absolute_frame(block_index: u32, block_frames: u16, sample_offset: u16) -> u64 {
    u64::from(block_index) * u64::from(block_frames) + u64::from(sample_offset)
}

fn output_rms(output_energy: f64, finite_output_samples: u64) -> f64 {
    if finite_output_samples == 0 {
        0.0
    } else {
        (output_energy / finite_output_samples as f64).sqrt()
    }
}

fn merge_process_diagnostics(
    target: &mut Vst3ProcessOutputDiagnostics,
    source: Vst3ProcessOutputDiagnostics,
) {
    merge_output_event_stats(&mut target.output_events, source.output_events);
    merge_output_parameter_change_stats(
        &mut target.output_parameter_changes,
        source.output_parameter_changes,
    );
}

fn merge_output_event_stats(target: &mut Vst3OutputEventStats, source: Vst3OutputEventStats) {
    target.raw_events = target.raw_events.saturating_add(source.raw_events);
    target.normalized_events = target
        .normalized_events
        .saturating_add(source.normalized_events);
    target.filtered_events = target
        .filtered_events
        .saturating_add(source.filtered_events);
    target.invalid_sample_offset_events = target
        .invalid_sample_offset_events
        .saturating_add(source.invalid_sample_offset_events);
    target.invalid_payload_events = target
        .invalid_payload_events
        .saturating_add(source.invalid_payload_events);
    target.advanced_events = target
        .advanced_events
        .saturating_add(source.advanced_events);
    target.advanced_data_events = target
        .advanced_data_events
        .saturating_add(source.advanced_data_events);
    target.advanced_note_expression_events = target
        .advanced_note_expression_events
        .saturating_add(source.advanced_note_expression_events);
    target.advanced_chord_events = target
        .advanced_chord_events
        .saturating_add(source.advanced_chord_events);
    target.advanced_scale_events = target
        .advanced_scale_events
        .saturating_add(source.advanced_scale_events);
    target.advanced_payload_events = target
        .advanced_payload_events
        .saturating_add(source.advanced_payload_events);
    target.advanced_payload_bytes = target
        .advanced_payload_bytes
        .saturating_add(source.advanced_payload_bytes);
    target.advanced_raw_payload_events = target
        .advanced_raw_payload_events
        .saturating_add(source.advanced_raw_payload_events);
    target.advanced_text_payload_events = target
        .advanced_text_payload_events
        .saturating_add(source.advanced_text_payload_events);
    target.advanced_truncated_payload_events = target
        .advanced_truncated_payload_events
        .saturating_add(source.advanced_truncated_payload_events);
    target.advanced_unavailable_payload_events = target
        .advanced_unavailable_payload_events
        .saturating_add(source.advanced_unavailable_payload_events);
    target.advanced_invalid_text_payload_events = target
        .advanced_invalid_text_payload_events
        .saturating_add(source.advanced_invalid_text_payload_events);
    target.unknown_type_events = target
        .unknown_type_events
        .saturating_add(source.unknown_type_events);
}

fn merge_output_parameter_change_stats(
    target: &mut Vst3OutputParameterChangeStats,
    source: Vst3OutputParameterChangeStats,
) {
    target.raw_points = target.raw_points.saturating_add(source.raw_points);
    target.normalized_points = target
        .normalized_points
        .saturating_add(source.normalized_points);
    target.filtered_points = target
        .filtered_points
        .saturating_add(source.filtered_points);
}

fn timing_summary(mut values: Vec<u64>) -> ProbeTimingSummary {
    if values.is_empty() {
        return ProbeTimingSummary::default();
    }
    values.sort_unstable();
    ProbeTimingSummary {
        min: values[0],
        p50: percentile(&values, 50),
        p95: percentile(&values, 95),
        p99: percentile(&values, 99),
        max: values[values.len() - 1],
    }
}

fn percentile(values: &[u64], percentile: u32) -> u64 {
    debug_assert!(!values.is_empty());
    let index = ((values.len() - 1) as u64 * u64::from(percentile) / 100) as usize;
    values[index]
}

#[cfg(test)]
#[path = "runtime_probe_process_tests.rs"]
mod tests;
