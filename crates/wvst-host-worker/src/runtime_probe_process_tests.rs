use super::*;
use wvst_vst3_host::{
    Vst3AdvancedOutputEvent, Vst3AdvancedOutputEventKind, Vst3InputEvent, Vst3NoteEvent,
    Vst3OutputEventStats, Vst3ProcessOutput,
};

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
fn merges_advanced_output_event_diagnostics() {
    let mut target = Vst3ProcessOutputDiagnostics {
        output_events: Vst3OutputEventStats {
            advanced_events: 1,
            advanced_data_events: 1,
            advanced_payload_events: 1,
            advanced_payload_bytes: 4,
            advanced_raw_payload_events: 1,
            ..Vst3OutputEventStats::default()
        },
        ..Vst3ProcessOutputDiagnostics::default()
    };
    let source = Vst3ProcessOutputDiagnostics {
        output_events: Vst3OutputEventStats {
            advanced_events: 4,
            advanced_note_expression_events: 2,
            advanced_chord_events: 1,
            advanced_scale_events: 1,
            advanced_payload_events: 2,
            advanced_payload_bytes: 9,
            advanced_text_payload_events: 2,
            advanced_truncated_payload_events: 1,
            advanced_unavailable_payload_events: 1,
            advanced_invalid_text_payload_events: 1,
            unknown_type_events: 3,
            ..Vst3OutputEventStats::default()
        },
        ..Vst3ProcessOutputDiagnostics::default()
    };

    merge_process_diagnostics(&mut target, source);

    assert_eq!(target.output_events.advanced_events, 5);
    assert_eq!(target.output_events.advanced_data_events, 1);
    assert_eq!(target.output_events.advanced_note_expression_events, 2);
    assert_eq!(target.output_events.advanced_chord_events, 1);
    assert_eq!(target.output_events.advanced_scale_events, 1);
    assert_eq!(target.output_events.advanced_payload_events, 3);
    assert_eq!(target.output_events.advanced_payload_bytes, 13);
    assert_eq!(target.output_events.advanced_raw_payload_events, 1);
    assert_eq!(target.output_events.advanced_text_payload_events, 2);
    assert_eq!(target.output_events.advanced_truncated_payload_events, 1);
    assert_eq!(target.output_events.advanced_unavailable_payload_events, 1);
    assert_eq!(target.output_events.advanced_invalid_text_payload_events, 1);
    assert_eq!(target.output_events.unknown_type_events, 3);
}

#[test]
fn records_bounded_advanced_output_event_examples() {
    let mut summary = RuntimeProbeProcessSummary::new(2, 128, 0, 0, 48_000, false);
    let process_output = Vst3ProcessOutput {
        advanced_events: (0..17)
            .map(|index| {
                let mut event = Vst3AdvancedOutputEvent {
                    sample_offset: index,
                    kind: Vst3AdvancedOutputEventKind::NoteExpressionIntValue,
                    vst3_event_type: 8,
                    bus_index: 1,
                    data1: 10,
                    data2: i32::from(index),
                    value: 0.0,
                    data_size: 0,
                    data_type: 44,
                    ..Vst3AdvancedOutputEvent::default()
                };
                if index == 0 {
                    event.payload_size = 3;
                    event.payload_encoding = VST3_OUTPUT_PAYLOAD_ENCODING_UTF8;
                    event.payload[..3].copy_from_slice(b"abc");
                }
                event
            })
            .collect(),
        ..Vst3ProcessOutput::default()
    };

    summary.observe_process_output(1, 128, &process_output);

    let value = serde_json::to_value(summary).expect("summary json");
    let examples = value["advancedOutputEventExamples"]
        .as_array()
        .expect("examples");
    assert_eq!(value["outputEvents"], 17);
    assert_eq!(examples.len(), MAX_ADVANCED_OUTPUT_EVENT_EXAMPLES);
    assert_eq!(examples[0]["blockIndex"], 1);
    assert_eq!(examples[0]["sampleOffset"], 0);
    assert_eq!(examples[0]["absoluteFrame"], 128);
    assert_eq!(examples[0]["kind"], "note-expression-int-value");
    assert_eq!(examples[0]["data1"], 10);
    assert_eq!(examples[0]["data2"], 0);
    assert_eq!(examples[0]["dataType"], 44);
    assert_eq!(examples[0]["payloadSize"], 3);
    assert_eq!(
        examples[0]["payloadEncoding"],
        u64::from(VST3_OUTPUT_PAYLOAD_ENCODING_UTF8)
    );
    assert_eq!(examples[0]["payloadFlags"], 0);
    assert_eq!(examples[0]["payloadHex"], "616263");
    assert_eq!(examples[0]["payloadText"], "abc");
    assert_eq!(examples[15]["sampleOffset"], 15);
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
