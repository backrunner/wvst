use std::time::Instant;

use serde::Serialize;
use serde_json::{Value, json};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    Vst3AudioBusInfo, Vst3BusDirection, Vst3BusType, Vst3ProcessOutput, Vst3ProcessingConfig,
    create_vst3_component_instance,
};

use crate::runtime_probe_controller::controller_summary as runtime_probe_controller_summary;
use crate::runtime_probe_error::{
    RUNTIME_PROBE_REPORT_SCHEMA_VERSION, RuntimeProbeFailure, vst3_control, vst3_init, vst3_process,
};
use crate::runtime_probe_options::{
    ProbeNote, ProbeParameterChange, RuntimeProbeOptions, probe_events, probe_parameter_changes,
};
use crate::runtime_probe_process::RuntimeProbeProcessSummary;

pub fn runtime_probe(args: &[String]) -> Result<(), String> {
    match runtime_probe_report(args) {
        Ok(report) => print_json(&report),
        Err(error) => {
            print_json(&error.to_report())?;
            Err(error.message)
        }
    }
}

fn runtime_probe_report(args: &[String]) -> Result<Value, RuntimeProbeFailure> {
    let options = RuntimeProbeOptions::parse(args).map_err(|error| {
        RuntimeProbeFailure::plain("runtime-probe-options", "options.parse", error)
    })?;
    let config = vst3_init(
        "processing.config",
        Vst3ProcessingConfig::new(
            options.sample_rate,
            options.max_block_frames,
            options.input_channels,
            options.output_channels,
        ),
    )?;
    let mut loaded = vst3_init(
        "component.create",
        create_vst3_component_instance(&options.bundle_path, &options.class_id, config),
    )?;
    let controller_class_id = vst3_init(
        "component.controller-class-id",
        loaded.instance().controller_class_id(),
    )?;

    vst3_init("component.initialize", loaded.instance_mut().initialize())?;
    vst3_init("controller.initialize", loaded.initialize_controller())?;
    let parameters = vst3_init("controller.parameters", loaded.parameters())?;
    let parameter_count = parameters.len();
    let controller_summary = runtime_probe_controller_summary(&loaded, &parameters);
    let input_buses = vst3_init(
        "component.audio-buses.input",
        loaded.instance_mut().audio_buses(Vst3BusDirection::Input),
    )?;
    let output_buses = vst3_init(
        "component.audio-buses.output",
        loaded.instance_mut().audio_buses(Vst3BusDirection::Output),
    )?;
    vst3_init(
        "component.setup-processing",
        loaded.instance_mut().setup_processing(),
    )?;
    vst3_init("component.activate", loaded.instance_mut().activate())?;
    vst3_init(
        "component.start-processing",
        loaded.instance_mut().start_processing(),
    )?;

    let process_result = run_process_blocks(&mut loaded, &options);
    let latency_samples = loaded.instance().latency_samples();
    let tail_samples = loaded.instance().tail_samples();
    let process_context_requirements = loaded.instance().process_context_requirements();
    let cleanup_result = stop_and_terminate(&mut loaded);
    let process_summary = match process_result {
        Ok(summary) => summary,
        Err(error) => {
            if let Err(cleanup_error) = cleanup_result {
                return Err(error.with_cleanup(cleanup_error));
            }
            return Err(error);
        }
    };
    cleanup_result?;

    Ok(json!({
        "schemaVersion": RUNTIME_PROBE_REPORT_SCHEMA_VERSION,
        "ok": true,
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
        "controller": controller_summary,
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

fn stop_and_terminate(
    loaded: &mut wvst_vst3_host::Vst3LoadedComponent,
) -> Result<(), RuntimeProbeFailure> {
    vst3_control(
        "component.stop-processing",
        loaded.instance_mut().stop_processing(),
    )?;
    vst3_control("controller.terminate", loaded.terminate_controller())?;
    vst3_control("component.terminate", loaded.instance_mut().terminate())
}

fn run_process_blocks(
    loaded: &mut wvst_vst3_host::Vst3LoadedComponent,
    options: &RuntimeProbeOptions,
) -> Result<RuntimeProbeProcessSummary, RuntimeProbeFailure> {
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
    let mut summary = RuntimeProbeProcessSummary::new(
        options.blocks,
        options.frames,
        input_len,
        output_len,
        options.sample_rate,
        options.note.is_some(),
    );

    for block_index in 0..options.blocks {
        input.fill(0.0);
        output.fill(0.0);
        if block_index == 0 {
            seed_probe_signal(&mut input, usize::from(options.input_channels));
        }
        let events = probe_events(options, block_index);
        let parameter_changes = probe_parameter_changes(options, block_index);
        summary.observe_input_events(block_index, options.frames, &events);
        let started = Instant::now();
        vst3_process(
            "component.process",
            loaded
                .instance_mut()
                .process_interleaved_f32_with_io_events_and_parameters(
                    frames,
                    &input,
                    &events,
                    &parameter_changes,
                    &mut output,
                    &mut process_output,
                ),
        )?;
        process_micros.push(started.elapsed().as_micros() as u64);

        summary.observe_output_block(
            block_index,
            options.frames,
            options.output_channels,
            &output,
        );
        summary.observe_process_output(&process_output);
    }
    summary.finish(process_micros);

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
    fn reports_invalid_config_as_structured_failure() {
        let args = vec![
            "/tmp/Test.vst3".to_string(),
            "class-a".to_string(),
            "--sample-rate".to_string(),
            "0".to_string(),
        ];
        let failure = runtime_probe_report(&args).expect_err("invalid config");
        let report = failure.to_report();

        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["ok"], false);
        assert_eq!(report["data"]["kind"], "vst3-runtime-init");
        assert_eq!(report["data"]["stage"], "processing.config");
        assert_eq!(report["data"]["hostError"], "invalid-sample-rate");
        assert_eq!(
            report["data"]["compatibility"]["category"],
            "processing-configuration"
        );
    }
}
