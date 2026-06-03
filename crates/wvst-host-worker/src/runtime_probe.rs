use std::time::Instant;

use serde::Serialize;
use serde_json::{Value, json};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    Vst3AudioBusInfo, Vst3BusDirection, Vst3BusType, Vst3OutputEventStats,
    Vst3OutputParameterChangeStats, Vst3ProcessOutput, Vst3ProcessOutputDiagnostics,
    Vst3ProcessingConfig, create_vst3_component_instance,
};

use crate::runtime_probe_options::{
    ProbeNote, ProbeParameterChange, RuntimeProbeOptions, probe_events, probe_parameter_changes,
};

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProbeProcessSummary {
    total_blocks: u32,
    total_frames: u64,
    input_samples: u64,
    output_samples: u64,
    max_output_peak: f32,
    non_zero_output_blocks: u32,
    output_events: u64,
    output_parameter_changes: u64,
    diagnostics: Vst3ProcessOutputDiagnostics,
    process_time_micros: ProbeTimingSummary,
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

pub fn runtime_probe(args: &[String]) -> Result<(), String> {
    let options = RuntimeProbeOptions::parse(args)?;
    let config = Vst3ProcessingConfig::new(
        options.sample_rate,
        options.max_block_frames,
        options.input_channels,
        options.output_channels,
    )
    .map_err(|error| error.to_string())?;
    let mut loaded =
        create_vst3_component_instance(&options.bundle_path, &options.class_id, config)
            .map_err(|error| error.to_string())?;
    let controller_class_id = loaded
        .instance()
        .controller_class_id()
        .map_err(|error| error.to_string())?;

    loaded
        .instance_mut()
        .initialize()
        .map_err(|error| error.to_string())?;
    loaded
        .initialize_controller()
        .map_err(|error| error.to_string())?;
    let parameter_count = loaded
        .parameters()
        .map_err(|error| error.to_string())?
        .len();
    let input_buses = loaded
        .instance_mut()
        .audio_buses(Vst3BusDirection::Input)
        .map_err(|error| error.to_string())?;
    let output_buses = loaded
        .instance_mut()
        .audio_buses(Vst3BusDirection::Output)
        .map_err(|error| error.to_string())?;
    loaded
        .instance_mut()
        .setup_processing()
        .map_err(|error| error.to_string())?;
    loaded
        .instance_mut()
        .activate()
        .map_err(|error| error.to_string())?;
    loaded
        .instance_mut()
        .start_processing()
        .map_err(|error| error.to_string())?;

    let process_result = run_process_blocks(&mut loaded, &options);
    let latency_samples = loaded.instance().latency_samples();
    let tail_samples = loaded.instance().tail_samples();
    let process_context_requirements = loaded.instance().process_context_requirements();
    let cleanup_result = stop_and_terminate(&mut loaded);
    let process_summary = match process_result {
        Ok(summary) => summary,
        Err(error) => {
            if let Err(cleanup_error) = cleanup_result {
                return Err(format!("{error}; cleanup failed: {cleanup_error}"));
            }
            return Err(error);
        }
    };
    cleanup_result?;

    print_json(&json!({
        "bundlePath": options.bundle_path,
        "classId": options.class_id,
        "processing": config,
        "frames": options.frames,
        "blocks": options.blocks,
        "probeInputs": {
            "note": options.note.map(note_json),
            "parameterChanges": options.parameter_changes.iter().map(parameter_change_json).collect::<Vec<_>>()
        },
        "controllerClassId": controller_class_id,
        "parameters": {
            "count": parameter_count
        },
        "audioBuses": {
            "inputs": input_buses.iter().map(audio_bus_json).collect::<Vec<_>>(),
            "outputs": output_buses.iter().map(audio_bus_json).collect::<Vec<_>>()
        },
        "latencySamples": latency_samples,
        "tailSamples": tail_samples,
        "processContextRequirements": process_context_requirements,
        "process": process_summary,
        "terminated": true
    }))
}

fn stop_and_terminate(loaded: &mut wvst_vst3_host::Vst3LoadedComponent) -> Result<(), String> {
    loaded
        .instance_mut()
        .stop_processing()
        .map_err(|error| error.to_string())?;
    loaded
        .terminate_controller()
        .map_err(|error| error.to_string())?;
    loaded
        .instance_mut()
        .terminate()
        .map_err(|error| error.to_string())
}

fn run_process_blocks(
    loaded: &mut wvst_vst3_host::Vst3LoadedComponent,
    options: &RuntimeProbeOptions,
) -> Result<RuntimeProbeProcessSummary, String> {
    let frames = usize::from(options.frames);
    let input_len = frames * usize::from(options.input_channels);
    let output_len = frames * usize::from(options.output_channels);
    let mut input = vec![0.0; input_len];
    let mut output = vec![0.0; output_len];
    let mut process_output = Vst3ProcessOutput::with_capacities(
        DEFAULT_MAX_VST3_EVENTS_PER_BLOCK,
        DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    );
    let mut process_micros = Vec::with_capacity(options.blocks as usize);
    let mut summary = RuntimeProbeProcessSummary {
        total_blocks: options.blocks,
        total_frames: u64::from(options.blocks) * u64::from(options.frames),
        input_samples: input_len as u64 * u64::from(options.blocks),
        output_samples: output_len as u64 * u64::from(options.blocks),
        ..RuntimeProbeProcessSummary::default()
    };

    for block_index in 0..options.blocks {
        input.fill(0.0);
        output.fill(0.0);
        if block_index == 0 {
            seed_probe_signal(&mut input, usize::from(options.input_channels));
        }
        let events = probe_events(options, block_index);
        let parameter_changes = probe_parameter_changes(options, block_index);
        let started = Instant::now();
        loaded
            .instance_mut()
            .process_interleaved_f32_with_io_events_and_parameters(
                frames,
                &input,
                &events,
                &parameter_changes,
                &mut output,
                &mut process_output,
            )
            .map_err(|error| error.to_string())?;
        process_micros.push(started.elapsed().as_micros() as u64);

        let output_peak = output
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        summary.max_output_peak = summary.max_output_peak.max(output_peak);
        if output_peak > 0.0 {
            summary.non_zero_output_blocks = summary.non_zero_output_blocks.saturating_add(1);
        }
        summary.output_events = summary
            .output_events
            .saturating_add(process_output.events.len() as u64);
        summary.output_parameter_changes = summary
            .output_parameter_changes
            .saturating_add(process_output.parameter_changes.len() as u64);
        merge_process_diagnostics(&mut summary.diagnostics, process_output.diagnostics);
    }
    summary.process_time_micros = timing_summary(process_micros);

    Ok(summary)
}

fn seed_probe_signal(input: &mut [f32], channels: usize) {
    if channels == 0 {
        return;
    }
    for (index, frame) in input.chunks_mut(channels).enumerate() {
        let sample = if index == 0 { 0.25 } else { 0.0 };
        for channel in frame {
            *channel = sample;
        }
    }
}

fn audio_bus_json(bus: &Vst3AudioBusInfo) -> Value {
    json!({
        "index": bus.index,
        "direction": match bus.direction {
            Vst3BusDirection::Input => "input",
            Vst3BusDirection::Output => "output",
        },
        "channelCount": bus.channel_count,
        "busType": match bus.bus_type {
            Vst3BusType::Main => json!("main"),
            Vst3BusType::Aux => json!("aux"),
            Vst3BusType::Unknown(value) => json!({ "unknown": value }),
        },
        "defaultActive": bus.default_active,
        "controlVoltage": bus.control_voltage,
        "name": bus.name,
    })
}

fn note_json(note: ProbeNote) -> Value {
    json!({
        "pitch": note.pitch,
        "velocity": note.velocity,
        "channel": note.channel,
    })
}

fn parameter_change_json(change: &ProbeParameterChange) -> Value {
    json!({
        "parameterId": change.parameter_id,
        "valueNormalized": change.value_normalized,
        "sampleOffset": change.sample_offset,
    })
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

fn print_json(value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_probe_signal_on_first_frame() {
        let mut input = vec![0.0; 8];

        seed_probe_signal(&mut input, 2);

        assert_eq!(&input[..4], &[0.25, 0.25, 0.0, 0.0]);
        assert_eq!(&input[4..], &[0.0, 0.0, 0.0, 0.0]);
    }

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
}
