use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};
use wvst_scanner::{
    MetadataSource, PluginClass, PluginDescriptor, PluginFormat, parse_vst3_bundle,
};
use wvst_vst3_host::{
    DEFAULT_MAX_VST3_EVENTS_PER_BLOCK, DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    HeadlessPluginInstance, Vst3AudioBusInfo, Vst3BusDirection, Vst3BusType, Vst3ProcessOutput,
    Vst3ProcessingConfig, create_vst3_component_instance, create_vst3_component_probe,
    load_vst3_factory_info, probe_vst3_module,
};

mod ipc;

fn main() {
    let exit_code = match run(std::env::args().skip(1).collect()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            2
        }
    };

    std::process::exit(exit_code);
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("describe") => describe(args.get(1)),
        Some("factory-info") => factory_info(args.get(1)),
        Some("factory-probe") => factory_probe(args.get(1)),
        Some("component-probe") => component_probe(args.get(1), args.get(2)),
        Some("runtime-probe") => runtime_probe(&args[1..]),
        Some("passthrough-probe") => passthrough_probe(),
        Some("serve") => ipc::serve_stdio(parse_audio_connect(&args[1..])?),
        Some("serve-framed") => ipc::serve_framed_stdio(parse_audio_connect(&args[1..])?),
        _ => Err(
            "usage: wvst-host-worker describe <plugin.vst3> | factory-info <plugin.vst3> | factory-probe <plugin.vst3> | component-probe <plugin.vst3> <class-id> | runtime-probe <plugin.vst3> <class-id> [--sample-rate n] [--max-block-frames n] [--input-channels n] [--output-channels n] [--frames n] | passthrough-probe | serve | serve-framed"
                .to_string(),
        ),
    }
}

fn parse_audio_connect(args: &[String]) -> Result<Option<String>, String> {
    let mut index = 0;
    let mut audio_connect = None;

    while index < args.len() {
        match args[index].as_str() {
            "--audio-connect" => {
                let Some(address) = args.get(index + 1) else {
                    return Err("--audio-connect requires an address".to_string());
                };
                audio_connect = Some(address.clone());
                index += 2;
            }
            other => return Err(format!("unknown serve option: {other}")),
        }
    }

    Ok(audio_connect)
}

fn describe(path: Option<&String>) -> Result<(), String> {
    let Some(path) = path else {
        return Err("describe requires a VST3 bundle path".to_string());
    };

    let descriptor = parse_vst3_bundle(PathBuf::from(path)).map_err(|error| error.to_string())?;
    print_json(&descriptor)
}

fn factory_info(path: Option<&String>) -> Result<(), String> {
    let Some(path) = path else {
        return Err("factory-info requires a VST3 bundle path".to_string());
    };

    let info = load_vst3_factory_info(PathBuf::from(path)).map_err(|error| error.to_string())?;
    print_json(&info)
}

fn factory_probe(path: Option<&String>) -> Result<(), String> {
    let Some(path) = path else {
        return Err("factory-probe requires a VST3 bundle path".to_string());
    };

    let probe = probe_vst3_module(PathBuf::from(path)).map_err(|error| error.to_string())?;
    print_json(&probe)
}

fn component_probe(path: Option<&String>, class_id: Option<&String>) -> Result<(), String> {
    let Some(path) = path else {
        return Err("component-probe requires a VST3 bundle path".to_string());
    };
    let Some(class_id) = class_id else {
        return Err("component-probe requires a VST3 class id".to_string());
    };

    let probe = create_vst3_component_probe(PathBuf::from(path), class_id)
        .map_err(|error| error.to_string())?;
    print_json(&probe)
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RuntimeProbeOptions {
    bundle_path: PathBuf,
    class_id: String,
    sample_rate: u32,
    max_block_frames: u16,
    input_channels: u16,
    output_channels: u16,
    frames: u16,
}

impl RuntimeProbeOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let Some(bundle_path) = args.first() else {
            return Err("runtime-probe requires a VST3 bundle path".to_string());
        };
        let Some(class_id) = args.get(1) else {
            return Err("runtime-probe requires a VST3 class id".to_string());
        };

        let mut options = Self {
            bundle_path: PathBuf::from(bundle_path),
            class_id: class_id.clone(),
            sample_rate: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            frames: 128,
        };
        let mut index = 2;
        while index < args.len() {
            match args[index].as_str() {
                "--sample-rate" => {
                    options.sample_rate = parse_option(args, index, "--sample-rate")?;
                    index += 2;
                }
                "--max-block-frames" => {
                    options.max_block_frames = parse_option(args, index, "--max-block-frames")?;
                    index += 2;
                }
                "--input-channels" => {
                    options.input_channels = parse_option(args, index, "--input-channels")?;
                    index += 2;
                }
                "--output-channels" => {
                    options.output_channels = parse_option(args, index, "--output-channels")?;
                    index += 2;
                }
                "--frames" => {
                    options.frames = parse_option(args, index, "--frames")?;
                    index += 2;
                }
                other => return Err(format!("unknown runtime-probe option: {other}")),
            }
        }
        if options.frames == 0 || options.frames > options.max_block_frames {
            return Err(format!(
                "--frames must be in 1..={} for this probe",
                options.max_block_frames
            ));
        }

        Ok(options)
    }
}

fn parse_option<T>(args: &[String], index: usize, label: &'static str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let Some(value) = args.get(index + 1) else {
        return Err(format!("{label} requires a value"));
    };
    value
        .parse::<T>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn runtime_probe(args: &[String]) -> Result<(), String> {
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

    let frames = usize::from(options.frames);
    let mut input = vec![0.0; frames * usize::from(options.input_channels)];
    seed_probe_signal(&mut input, usize::from(options.input_channels));
    let mut output = vec![0.0; frames * usize::from(options.output_channels)];
    let mut process_output = Vst3ProcessOutput::with_capacities(
        DEFAULT_MAX_VST3_EVENTS_PER_BLOCK,
        DEFAULT_MAX_VST3_PARAMETER_CHANGES_PER_BLOCK,
    );
    loaded
        .instance_mut()
        .process_interleaved_f32_with_io_events_and_parameters(
            frames,
            &input,
            &[],
            &[],
            &mut output,
            &mut process_output,
        )
        .map_err(|error| error.to_string())?;
    let output_peak = output
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let latency_samples = loaded.instance().latency_samples();
    let tail_samples = loaded.instance().tail_samples();
    let process_context_requirements = loaded.instance().process_context_requirements();

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
        .map_err(|error| error.to_string())?;

    print_json(&json!({
        "bundlePath": options.bundle_path,
        "classId": options.class_id,
        "processing": config,
        "frames": options.frames,
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
        "process": {
            "inputSamples": input.len(),
            "outputSamples": output.len(),
            "outputPeak": output_peak,
            "outputEvents": process_output.events.len(),
            "outputParameterChanges": process_output.parameter_changes.len(),
            "diagnostics": process_output.diagnostics
        },
        "terminated": true
    }))
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

fn passthrough_probe() -> Result<(), String> {
    let descriptor = probe_descriptor();
    let instance =
        HeadlessPluginInstance::new(&descriptor, 2, 2).map_err(|error| error.to_string())?;
    let input = [0.1, 0.2, 0.3, 0.4];
    let mut output = [0.0; 4];
    let stats = instance
        .process_interleaved_f32(2, &input, &mut output)
        .map_err(|error| error.to_string())?;

    print_json(&json!({
        "metadata": instance.metadata(),
        "stats": stats,
        "output": output,
    }))
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}

fn probe_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        plugin_id: "vst3:probe".to_string(),
        format: PluginFormat::Vst3,
        name: "WVST Passthrough Probe".to_string(),
        vendor: Some("WVST".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        path: "wvst://probe".to_string(),
        classes: vec![PluginClass {
            class_id: None,
            name: "WVST Passthrough Probe".to_string(),
            category: Some("Fx".to_string()),
            subcategories: vec!["Stereo".to_string()],
        }],
        metadata_source: MetadataSource::BundleName,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_runtime_probe_defaults() {
        let options = RuntimeProbeOptions::parse(&[
            "/tmp/Test.vst3".to_string(),
            "0123456789abcdef0123456789abcdef".to_string(),
        ])
        .expect("options");

        assert_eq!(options.bundle_path, PathBuf::from("/tmp/Test.vst3"));
        assert_eq!(options.sample_rate, 48_000);
        assert_eq!(options.max_block_frames, 128);
        assert_eq!(options.input_channels, 2);
        assert_eq!(options.output_channels, 2);
        assert_eq!(options.frames, 128);
    }

    #[test]
    fn parses_runtime_probe_overrides() {
        let options = RuntimeProbeOptions::parse(&[
            "/tmp/Test.vst3".to_string(),
            "class-a".to_string(),
            "--sample-rate".to_string(),
            "96000".to_string(),
            "--max-block-frames".to_string(),
            "512".to_string(),
            "--input-channels".to_string(),
            "0".to_string(),
            "--output-channels".to_string(),
            "2".to_string(),
            "--frames".to_string(),
            "256".to_string(),
        ])
        .expect("options");

        assert_eq!(options.sample_rate, 96_000);
        assert_eq!(options.max_block_frames, 512);
        assert_eq!(options.input_channels, 0);
        assert_eq!(options.output_channels, 2);
        assert_eq!(options.frames, 256);
    }

    #[test]
    fn rejects_runtime_probe_frames_above_block_size() {
        let error = RuntimeProbeOptions::parse(&[
            "/tmp/Test.vst3".to_string(),
            "class-a".to_string(),
            "--max-block-frames".to_string(),
            "128".to_string(),
            "--frames".to_string(),
            "256".to_string(),
        ])
        .expect_err("invalid frames");

        assert!(error.contains("--frames"));
    }

    #[test]
    fn seeds_probe_signal_on_first_frame() {
        let mut input = vec![0.0; 8];

        seed_probe_signal(&mut input, 2);

        assert_eq!(&input[..4], &[0.25, 0.25, 0.0, 0.0]);
        assert_eq!(&input[4..], &[0.0, 0.0, 0.0, 0.0]);
    }
}
