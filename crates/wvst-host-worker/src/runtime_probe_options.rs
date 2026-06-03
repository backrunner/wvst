use std::path::PathBuf;

use wvst_vst3_host::{Vst3InputEvent, Vst3NoteEvent, Vst3ParameterChange};

const DEFAULT_PROBE_BLOCKS: u32 = 1;
const MAX_PROBE_BLOCKS: u32 = 60_000;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RuntimeProbeOptions {
    pub(super) bundle_path: PathBuf,
    pub(super) class_id: String,
    pub(super) sample_rate: u32,
    pub(super) max_block_frames: u16,
    pub(super) input_channels: u16,
    pub(super) output_channels: u16,
    pub(super) frames: u16,
    pub(super) blocks: u32,
    pub(super) controller_edit_probe: bool,
    pub(super) controller_edit_probe_parameter_id: Option<u32>,
    pub(super) connection_notify_probe: bool,
    pub(super) state_roundtrip_probe: bool,
    pub(super) note: Option<ProbeNote>,
    pub(super) parameter_changes: Vec<ProbeParameterChange>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ProbeNote {
    pub(super) pitch: u8,
    pub(super) velocity: f32,
    pub(super) channel: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ProbeParameterChange {
    pub(super) parameter_id: u32,
    pub(super) value_normalized: f64,
    pub(super) sample_offset: u16,
}

impl RuntimeProbeOptions {
    pub(super) fn parse(args: &[String]) -> Result<Self, String> {
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
            blocks: DEFAULT_PROBE_BLOCKS,
            controller_edit_probe: true,
            controller_edit_probe_parameter_id: None,
            connection_notify_probe: true,
            state_roundtrip_probe: true,
            note: None,
            parameter_changes: Vec::new(),
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
                "--blocks" => {
                    options.blocks = parse_option(args, index, "--blocks")?;
                    index += 2;
                }
                "--skip-controller-edit-probe" => {
                    options.controller_edit_probe = false;
                    index += 1;
                }
                "--skip-connection-notify-probe" => {
                    options.connection_notify_probe = false;
                    index += 1;
                }
                "--skip-state-roundtrip-probe" => {
                    options.state_roundtrip_probe = false;
                    index += 1;
                }
                "--controller-edit-probe-parameter" => {
                    options.controller_edit_probe_parameter_id = Some(parse_option(
                        args,
                        index,
                        "--controller-edit-probe-parameter",
                    )?);
                    index += 2;
                }
                "--note" => {
                    let value = option_value(args, index, "--note")?;
                    options.note = Some(parse_note(value)?);
                    index += 2;
                }
                "--parameter-change" => {
                    let value = option_value(args, index, "--parameter-change")?;
                    options
                        .parameter_changes
                        .push(parse_parameter_change(value)?);
                    index += 2;
                }
                other => return Err(format!("unknown runtime-probe option: {other}")),
            }
        }
        options.validate()?;

        Ok(options)
    }

    fn validate(&self) -> Result<(), String> {
        if self.frames == 0 || self.frames > self.max_block_frames {
            return Err(format!(
                "--frames must be in 1..={} for this probe",
                self.max_block_frames
            ));
        }
        if self.blocks == 0 || self.blocks > MAX_PROBE_BLOCKS {
            return Err(format!("--blocks must be in 1..={MAX_PROBE_BLOCKS}"));
        }
        for change in &self.parameter_changes {
            if change.sample_offset >= self.frames {
                return Err(format!(
                    "--parameter-change sample offset must be in 0..{}",
                    self.frames
                ));
            }
        }
        Ok(())
    }
}

pub(super) fn probe_events(options: &RuntimeProbeOptions, block_index: u32) -> Vec<Vst3InputEvent> {
    let Some(note) = options.note else {
        return Vec::new();
    };
    let mut events = Vec::new();
    if block_index == 0 {
        events.push(Vst3InputEvent::NoteOn(Vst3NoteEvent {
            sample_offset: 0,
            channel: note.channel,
            pitch: note.pitch,
            velocity: note.velocity,
            note_id: 1,
        }));
    }
    if block_index + 1 == options.blocks {
        events.push(Vst3InputEvent::NoteOff(Vst3NoteEvent {
            sample_offset: options.frames - 1,
            channel: note.channel,
            pitch: note.pitch,
            velocity: 0.0,
            note_id: 1,
        }));
    }
    events
}

pub(super) fn probe_parameter_changes(
    options: &RuntimeProbeOptions,
    block_index: u32,
) -> Vec<Vst3ParameterChange> {
    if block_index != 0 {
        return Vec::new();
    }
    options
        .parameter_changes
        .iter()
        .map(|change| Vst3ParameterChange {
            sample_offset: change.sample_offset,
            parameter_id: change.parameter_id,
            value_normalized: change.value_normalized,
        })
        .collect()
}

fn parse_option<T>(args: &[String], index: usize, label: &'static str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = option_value(args, index, label)?;
    value
        .parse::<T>()
        .map_err(|error| format!("invalid {label}: {error}"))
}

fn option_value<'a>(
    args: &'a [String],
    index: usize,
    label: &'static str,
) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{label} requires a value"))
}

fn parse_note(value: &str) -> Result<ProbeNote, String> {
    let parts = split_fields(value, "--note", 3)?;
    let pitch = parse_bounded_u8(parts[0], "--note pitch", 0, 127)?;
    let velocity = if let Some(value) = parts.get(1) {
        parse_normalized_f32(value, "--note velocity")?
    } else {
        0.8
    };
    let channel = if let Some(value) = parts.get(2) {
        parse_bounded_u8(value, "--note channel", 0, 15)?
    } else {
        0
    };

    Ok(ProbeNote {
        pitch,
        velocity,
        channel,
    })
}

fn parse_parameter_change(value: &str) -> Result<ProbeParameterChange, String> {
    let parts = split_fields(value, "--parameter-change", 3)?;
    let id_and_value = parts[0];
    let Some((parameter_id, normalized_value)) = id_and_value.split_once('=') else {
        return Err("--parameter-change must use id=value[:sample-offset]".to_string());
    };
    let parameter_id = parameter_id
        .parse::<u32>()
        .map_err(|error| format!("invalid --parameter-change id: {error}"))?;
    let value_normalized =
        parse_normalized_f64(normalized_value, "--parameter-change normalized value")?;
    let sample_offset = if let Some(value) = parts.get(1) {
        value
            .parse::<u16>()
            .map_err(|error| format!("invalid --parameter-change sample offset: {error}"))?
    } else {
        0
    };

    Ok(ProbeParameterChange {
        parameter_id,
        value_normalized,
        sample_offset,
    })
}

fn split_fields<'a>(
    value: &'a str,
    label: &'static str,
    max_fields: usize,
) -> Result<Vec<&'a str>, String> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.is_empty() || fields.len() > max_fields || fields.iter().any(|field| field.is_empty())
    {
        return Err(format!("{label} has invalid field count"));
    }
    Ok(fields)
}

fn parse_bounded_u8(value: &str, label: &'static str, min: u8, max: u8) -> Result<u8, String> {
    let parsed = value
        .parse::<u8>()
        .map_err(|error| format!("invalid {label}: {error}"))?;
    if parsed < min || parsed > max {
        return Err(format!("{label} must be in {min}..={max}"));
    }
    Ok(parsed)
}

fn parse_normalized_f32(value: &str, label: &'static str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|error| format!("invalid {label}: {error}"))?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(format!("{label} must be finite and in 0.0..=1.0"));
    }
    Ok(parsed)
}

fn parse_normalized_f64(value: &str, label: &'static str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|error| format!("invalid {label}: {error}"))?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(format!("{label} must be finite and in 0.0..=1.0"));
    }
    Ok(parsed)
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
        assert_eq!(options.blocks, 1);
        assert!(options.controller_edit_probe);
        assert_eq!(options.controller_edit_probe_parameter_id, None);
        assert!(options.connection_notify_probe);
        assert!(options.state_roundtrip_probe);
        assert_eq!(options.note, None);
        assert!(options.parameter_changes.is_empty());
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
            "--blocks".to_string(),
            "4".to_string(),
            "--skip-controller-edit-probe".to_string(),
            "--skip-connection-notify-probe".to_string(),
            "--skip-state-roundtrip-probe".to_string(),
            "--controller-edit-probe-parameter".to_string(),
            "99".to_string(),
            "--note".to_string(),
            "64:0.5:2".to_string(),
            "--parameter-change".to_string(),
            "42=0.75:32".to_string(),
        ])
        .expect("options");

        assert_eq!(options.sample_rate, 96_000);
        assert_eq!(options.max_block_frames, 512);
        assert_eq!(options.input_channels, 0);
        assert_eq!(options.output_channels, 2);
        assert_eq!(options.frames, 256);
        assert_eq!(options.blocks, 4);
        assert!(!options.controller_edit_probe);
        assert_eq!(options.controller_edit_probe_parameter_id, Some(99));
        assert!(!options.connection_notify_probe);
        assert!(!options.state_roundtrip_probe);
        assert_eq!(
            options.note,
            Some(ProbeNote {
                pitch: 64,
                velocity: 0.5,
                channel: 2
            })
        );
        assert_eq!(
            options.parameter_changes,
            vec![ProbeParameterChange {
                parameter_id: 42,
                value_normalized: 0.75,
                sample_offset: 32
            }]
        );
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
    fn rejects_parameter_changes_outside_probe_block() {
        let error = RuntimeProbeOptions::parse(&[
            "/tmp/Test.vst3".to_string(),
            "class-a".to_string(),
            "--frames".to_string(),
            "128".to_string(),
            "--parameter-change".to_string(),
            "42=0.5:128".to_string(),
        ])
        .expect_err("invalid parameter change");

        assert!(error.contains("sample offset"));
    }

    #[test]
    fn creates_note_on_and_off_across_probe_window() {
        let mut options = RuntimeProbeOptions::parse(&[
            "/tmp/Test.vst3".to_string(),
            "class-a".to_string(),
            "--frames".to_string(),
            "128".to_string(),
            "--blocks".to_string(),
            "2".to_string(),
            "--note".to_string(),
            "60".to_string(),
        ])
        .expect("options");

        assert!(matches!(
            probe_events(&options, 0).as_slice(),
            [Vst3InputEvent::NoteOn(_)]
        ));
        assert!(matches!(
            probe_events(&options, 1).as_slice(),
            [Vst3InputEvent::NoteOff(_)]
        ));

        options.blocks = 1;
        assert!(matches!(
            probe_events(&options, 0).as_slice(),
            [Vst3InputEvent::NoteOn(_), Vst3InputEvent::NoteOff(_)]
        ));
    }
}
