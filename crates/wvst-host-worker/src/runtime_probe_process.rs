use serde::Serialize;
use wvst_vst3_host::{
    Vst3InputEvent, Vst3OutputEventStats, Vst3OutputParameterChangeStats, Vst3ProcessOutput,
    Vst3ProcessOutputDiagnostics,
};

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

    pub(crate) fn observe_process_output(&mut self, process_output: &Vst3ProcessOutput) {
        self.output_events = self
            .output_events
            .saturating_add(process_output.events.len() as u64);
        self.output_parameter_changes = self
            .output_parameter_changes
            .saturating_add(process_output.parameter_changes.len() as u64);
        merge_process_diagnostics(&mut self.diagnostics, process_output.diagnostics);
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
mod tests {
    use super::*;
    use wvst_vst3_host::{Vst3InputEvent, Vst3NoteEvent};

    #[test]
    fn summarizes_process_timings() {
        assert_eq!(
            timing_summary(vec![10, 50, 30, 20, 40]),
            ProbeTimingSummary {
                min: 10,
                p50: 30,
                p95: 40,
                p99: 40,
                max: 50
            }
        );
    }

    #[test]
    fn observes_output_block_audio_diagnostics() {
        let mut summary = RuntimeProbeProcessSummary::new(2, 2, 4, 4, 48_000, false);

        summary.observe_output_block(0, 2, 2, &[0.0, 0.0, 0.0, 0.0]);
        summary.observe_output_block(1, 2, 2, &[0.0, 0.5, -1.25, f32::NAN]);
        summary.finish(vec![10, 20]);

        assert_eq!(summary.finite_output_samples, 7);
        assert_eq!(summary.non_finite_output_samples, 1);
        assert_eq!(summary.clipped_output_samples, 1);
        assert_eq!(summary.silent_output_blocks, 1);
        assert_eq!(summary.non_zero_output_blocks, 1);
        assert_eq!(summary.first_non_zero_output_block, Some(1));
        assert_eq!(summary.first_non_zero_output_frame, Some(0));
        assert_eq!(summary.first_non_zero_output_absolute_frame, Some(2));
        assert_eq!(summary.max_output_peak_block, Some(1));
        assert_eq!(summary.max_output_peak, 1.25);
        assert!((summary.output_energy - 1.8125).abs() < f64::EPSILON);
        assert!((summary.output_rms - (1.8125_f64 / 7.0).sqrt()).abs() < f64::EPSILON);

        let value = serde_json::to_value(summary).expect("summary json");
        assert_eq!(value["finiteOutputSamples"], 7);
        assert_eq!(value["nonFiniteOutputSamples"], 1);
        assert_eq!(value["firstNonZeroOutputBlock"], 1);
        assert_eq!(value["firstNonZeroOutputFrame"], 0);
        assert_eq!(value["firstNonZeroOutputAbsoluteFrame"], 2);
    }

    #[test]
    fn reports_note_to_first_output_timing() {
        let mut summary = RuntimeProbeProcessSummary::new(4, 128, 0, 256, 48_000, true);
        let note_on = Vst3InputEvent::NoteOn(Vst3NoteEvent {
            sample_offset: 32,
            channel: 0,
            pitch: 60,
            velocity: 0.75,
            note_id: 1,
        });
        summary.observe_input_events(0, 128, &[note_on]);

        let mut output = vec![0.0; 256];
        output[64 * 2 + 1] = 0.25;
        summary.observe_output_block(1, 128, 2, &output);

        let value = serde_json::to_value(summary).expect("summary json");
        let note_timing = &value["noteTiming"];
        assert_eq!(note_timing["notePresent"], true);
        assert_eq!(note_timing["noteOnBlock"], 0);
        assert_eq!(note_timing["noteOnSampleOffset"], 32);
        assert_eq!(note_timing["noteOnAbsoluteFrame"], 32);
        assert_eq!(note_timing["firstNonZeroOutputBlock"], 1);
        assert_eq!(note_timing["firstNonZeroOutputFrame"], 64);
        assert_eq!(note_timing["firstNonZeroOutputAbsoluteFrame"], 192);
        assert_eq!(note_timing["framesFromNoteOnToFirstNonZeroOutput"], 160);
        assert_eq!(note_timing["microsFromNoteOnToFirstNonZeroOutput"], 3333);
    }

    #[test]
    fn leaves_note_latency_empty_for_silent_output() {
        let mut summary = RuntimeProbeProcessSummary::new(1, 128, 0, 256, 48_000, true);
        let note_on = Vst3InputEvent::NoteOn(Vst3NoteEvent {
            sample_offset: 0,
            channel: 0,
            pitch: 60,
            velocity: 0.75,
            note_id: 1,
        });
        summary.observe_input_events(0, 128, &[note_on]);
        summary.observe_output_block(0, 128, 2, &[0.0; 256]);

        let value = serde_json::to_value(summary).expect("summary json");
        let note_timing = &value["noteTiming"];
        assert_eq!(note_timing["notePresent"], true);
        assert_eq!(note_timing["noteOnAbsoluteFrame"], 0);
        assert!(note_timing["firstNonZeroOutputAbsoluteFrame"].is_null());
        assert!(note_timing["framesFromNoteOnToFirstNonZeroOutput"].is_null());
    }
}
