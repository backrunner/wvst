use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;
use wvst_scanner::{
    MetadataSource, PluginClass, PluginDescriptor, PluginFormat, parse_vst3_bundle,
};
use wvst_vst3_host::{
    HeadlessPluginInstance, create_vst3_component_probe, load_vst3_factory_info, probe_vst3_module,
};

mod ipc;
mod runtime_probe;
mod runtime_probe_options;

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
        Some("runtime-probe") => runtime_probe::runtime_probe(&args[1..]),
        Some("passthrough-probe") => passthrough_probe(),
        Some("serve") => ipc::serve_stdio(parse_audio_connect(&args[1..])?),
        Some("serve-framed") => ipc::serve_framed_stdio(parse_audio_connect(&args[1..])?),
        _ => Err(
            "usage: wvst-host-worker describe <plugin.vst3> | factory-info <plugin.vst3> | factory-probe <plugin.vst3> | component-probe <plugin.vst3> <class-id> | runtime-probe <plugin.vst3> <class-id> [--sample-rate n] [--max-block-frames n] [--input-channels n] [--output-channels n] [--frames n] [--blocks n] [--note pitch[:velocity[:channel]]] [--parameter-change id=value[:sample-offset]] | passthrough-probe | serve | serve-framed"
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
