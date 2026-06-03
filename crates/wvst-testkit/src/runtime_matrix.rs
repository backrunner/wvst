use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod audio_bus_expectations;
mod audio_bus_summary;
mod controller_expectations;
mod expectations;
mod note_timing;
mod output_expectations;
mod process_output_summary;
mod process_timing_expectations;
mod process_timing_summary;
mod report;
mod runtime_characteristics_expectations;
mod runtime_characteristics_summary;

pub use audio_bus_summary::RuntimeProbeAudioBusSummary;
pub use expectations::RuntimeProbeExpectations;
pub use note_timing::RuntimeProbeNoteTimingHealthSummary;
pub use process_output_summary::RuntimeProbeProcessOutputSummary;
pub use process_timing_summary::RuntimeProbeProcessTimingSummary;
pub use report::{
    RuntimeProbeAudioHealthSummary, RuntimeProbeControllerHealthSummary,
    RuntimeProbeDiagnosticsSummary, RuntimeProbeEvidenceSummary, RuntimeProbeMatrixReport,
    RuntimeProbeResult, RuntimeProbeStatus,
};
pub use runtime_characteristics_summary::RuntimeProbeRuntimeCharacteristicsSummary;

pub const RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeCase {
    pub name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    pub evidence: Option<RuntimeProbeCaseEvidence>,
    pub sample_rate_hz: u32,
    pub max_block_frames: u16,
    pub input_channels: u16,
    pub output_channels: u16,
    pub frames: u16,
    pub blocks: u32,
    pub controller_edit_probe: bool,
    pub controller_edit_probe_parameter_id: Option<u32>,
    pub connection_notify_probe: bool,
    pub state_roundtrip_probe: bool,
    pub note: Option<RuntimeProbeNote>,
    pub parameter_changes: Vec<RuntimeProbeParameterChange>,
    pub expectations: Option<RuntimeProbeExpectations>,
    pub timeout_millis: u64,
}

impl RuntimeProbeCase {
    pub fn new(
        name: impl Into<String>,
        plugin_path: impl Into<PathBuf>,
        class_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            plugin_path: plugin_path.into(),
            class_id: class_id.into(),
            evidence: None,
            sample_rate_hz: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            frames: 128,
            blocks: 1,
            controller_edit_probe: true,
            controller_edit_probe_parameter_id: None,
            connection_notify_probe: true,
            state_roundtrip_probe: true,
            note: None,
            parameter_changes: Vec::new(),
            expectations: None,
            timeout_millis: DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS,
        }
    }

    pub const fn with_processing(
        mut self,
        sample_rate_hz: u32,
        max_block_frames: u16,
        input_channels: u16,
        output_channels: u16,
        frames: u16,
        blocks: u32,
    ) -> Self {
        self.sample_rate_hz = sample_rate_hz;
        self.max_block_frames = max_block_frames;
        self.input_channels = input_channels;
        self.output_channels = output_channels;
        self.frames = frames;
        self.blocks = blocks;
        self
    }

    pub const fn with_note(mut self, note: RuntimeProbeNote) -> Self {
        self.note = Some(note);
        self
    }

    pub const fn with_controller_edit_probe_parameter_id(mut self, parameter_id: u32) -> Self {
        self.controller_edit_probe_parameter_id = Some(parameter_id);
        self
    }

    pub const fn with_controller_edit_probe(mut self, enabled: bool) -> Self {
        self.controller_edit_probe = enabled;
        self
    }

    pub const fn with_connection_notify_probe(mut self, enabled: bool) -> Self {
        self.connection_notify_probe = enabled;
        self
    }

    pub const fn with_state_roundtrip_probe(mut self, enabled: bool) -> Self {
        self.state_roundtrip_probe = enabled;
        self
    }

    pub fn with_evidence(mut self, evidence: RuntimeProbeCaseEvidence) -> Self {
        self.evidence = Some(evidence);
        self
    }

    pub fn with_parameter_change(mut self, change: RuntimeProbeParameterChange) -> Self {
        self.parameter_changes.push(change);
        self
    }

    pub fn with_expectations(mut self, expectations: RuntimeProbeExpectations) -> Self {
        self.expectations = Some(expectations);
        self
    }

    pub const fn with_timeout_millis(mut self, timeout_millis: u64) -> Self {
        self.timeout_millis = timeout_millis;
        self
    }

    fn args(&self) -> Vec<String> {
        let mut args = vec![
            "runtime-probe".to_string(),
            self.plugin_path.display().to_string(),
            self.class_id.clone(),
            "--sample-rate".to_string(),
            self.sample_rate_hz.to_string(),
            "--max-block-frames".to_string(),
            self.max_block_frames.to_string(),
            "--input-channels".to_string(),
            self.input_channels.to_string(),
            "--output-channels".to_string(),
            self.output_channels.to_string(),
            "--frames".to_string(),
            self.frames.to_string(),
            "--blocks".to_string(),
            self.blocks.to_string(),
        ];
        if let Some(note) = self.note {
            args.push("--note".to_string());
            args.push(note.to_arg());
        }
        if !self.controller_edit_probe {
            args.push("--skip-controller-edit-probe".to_string());
        }
        if !self.connection_notify_probe {
            args.push("--skip-connection-notify-probe".to_string());
        }
        if !self.state_roundtrip_probe {
            args.push("--skip-state-roundtrip-probe".to_string());
        }
        if let Some(parameter_id) = self.controller_edit_probe_parameter_id {
            args.push("--controller-edit-probe-parameter".to_string());
            args.push(parameter_id.to_string());
        }
        for change in &self.parameter_changes {
            args.push("--parameter-change".to_string());
            args.push(change.to_arg());
        }
        args
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbePluginKind {
    #[default]
    Unknown,
    Effect,
    Instrument,
    Hybrid,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeCaseEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_version: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_plugin_kind")]
    pub plugin_kind: RuntimeProbePluginKind,
    #[serde(default, skip_serializing_if = "is_false")]
    pub third_party: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_notes: Option<String>,
}

impl RuntimeProbeCaseEvidence {
    pub fn third_party_plugin(
        vendor: impl Into<String>,
        plugin_name: impl Into<String>,
        plugin_kind: RuntimeProbePluginKind,
    ) -> Self {
        Self {
            vendor: Some(vendor.into()),
            plugin_name: Some(plugin_name.into()),
            plugin_kind,
            third_party: true,
            ..Self::default()
        }
    }

    pub fn with_plugin_version(mut self, plugin_version: impl Into<String>) -> Self {
        self.plugin_version = Some(plugin_version.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_validation_notes(mut self, notes: impl Into<String>) -> Self {
        self.validation_notes = Some(notes.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProbeNote {
    pub pitch: u8,
    pub velocity_milli: u16,
    pub channel: u8,
}

impl RuntimeProbeNote {
    pub const fn new(pitch: u8, velocity_milli: u16, channel: u8) -> Self {
        Self {
            pitch,
            velocity_milli,
            channel,
        }
    }

    fn to_arg(self) -> String {
        format!(
            "{}:{:.3}:{}",
            self.pitch,
            f32::from(self.velocity_milli) / 1000.0,
            self.channel
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProbeParameterChange {
    pub parameter_id: u32,
    pub value_milli: u16,
    pub sample_offset: u16,
}

impl RuntimeProbeParameterChange {
    pub const fn new(parameter_id: u32, value_milli: u16, sample_offset: u16) -> Self {
        Self {
            parameter_id,
            value_milli,
            sample_offset,
        }
    }

    fn to_arg(self) -> String {
        format!(
            "{}={:.3}:{}",
            self.parameter_id,
            f64::from(self.value_milli) / 1000.0,
            self.sample_offset
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeMatrix {
    worker_executable: PathBuf,
    cases: Vec<RuntimeProbeCase>,
}

impl RuntimeProbeMatrix {
    pub fn new(worker_executable: impl Into<PathBuf>) -> Self {
        Self {
            worker_executable: worker_executable.into(),
            cases: Vec::new(),
        }
    }

    pub fn with_case(mut self, case: RuntimeProbeCase) -> Self {
        self.cases.push(case);
        self
    }

    pub fn push_case(&mut self, case: RuntimeProbeCase) {
        self.cases.push(case);
    }

    pub fn cases(&self) -> &[RuntimeProbeCase] {
        &self.cases
    }

    pub fn run(&self) -> RuntimeProbeMatrixReport {
        let mut executor = ProcessRuntimeProbeExecutor;
        self.run_with(&mut executor)
    }

    pub fn run_with(&self, executor: &mut impl RuntimeProbeExecutor) -> RuntimeProbeMatrixReport {
        let results = self
            .cases
            .iter()
            .map(|case| {
                let invocation = RuntimeProbeInvocation {
                    worker_executable: self.worker_executable.clone(),
                    args: case.args(),
                    timeout_millis: case.timeout_millis,
                };
                let started = Instant::now();
                let mut result = executor.run(invocation);
                result.case_name = case.name.clone();
                result.plugin_path = case.plugin_path.clone();
                result.class_id = case.class_id.clone();
                result.evidence = case.evidence.clone();
                result.duration_millis = started.elapsed().as_millis() as u64;
                if let Some(expectations) = &case.expectations {
                    expectations.apply_to(&mut result);
                }
                result
            })
            .collect::<Vec<_>>();

        RuntimeProbeMatrixReport::new(results)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeInvocation {
    pub worker_executable: PathBuf,
    pub args: Vec<String>,
    pub timeout_millis: u64,
}

pub trait RuntimeProbeExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProcessRuntimeProbeExecutor;

impl RuntimeProbeExecutor for ProcessRuntimeProbeExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
        let mut child = match Command::new(&invocation.worker_executable)
            .args(&invocation.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return RuntimeProbeResult {
                    status: RuntimeProbeStatus::LaunchFailed,
                    stderr: error.to_string(),
                    ..RuntimeProbeResult::default()
                };
            }
        };

        let timeout = Duration::from_millis(invocation.timeout_millis);
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_status)) => match child.wait_with_output() {
                    Ok(output) => return output_to_result(output, RuntimeProbeStatus::Failed),
                    Err(error) => {
                        return RuntimeProbeResult {
                            status: RuntimeProbeStatus::Failed,
                            stderr: error.to_string(),
                            ..RuntimeProbeResult::default()
                        };
                    }
                },
                Ok(None) if started.elapsed() >= timeout => {
                    let _ = child.kill();
                    return match child.wait_with_output() {
                        Ok(output) => {
                            let mut result = output_to_result(output, RuntimeProbeStatus::TimedOut);
                            append_timeout_stderr(&mut result.stderr, invocation.timeout_millis);
                            result
                        }
                        Err(error) => RuntimeProbeResult {
                            status: RuntimeProbeStatus::TimedOut,
                            stderr: format!(
                                "runtime probe timed out after {} ms; failed to collect output: {error}",
                                invocation.timeout_millis
                            ),
                            ..RuntimeProbeResult::default()
                        },
                    };
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    return RuntimeProbeResult {
                        status: RuntimeProbeStatus::Failed,
                        stderr: error.to_string(),
                        ..RuntimeProbeResult::default()
                    };
                }
            }
        }
    }
}

fn output_to_result(
    output: std::process::Output,
    timeout_status: RuntimeProbeStatus,
) -> RuntimeProbeResult {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = if matches!(timeout_status, RuntimeProbeStatus::TimedOut) {
        RuntimeProbeStatus::TimedOut
    } else if output.status.success() {
        RuntimeProbeStatus::Passed
    } else {
        RuntimeProbeStatus::Failed
    };

    RuntimeProbeResult {
        status,
        exit_code: output.status.code(),
        probe_report: parse_probe_report(&stdout).or_else(|| parse_probe_report(&stderr)),
        stdout,
        stderr,
        ..RuntimeProbeResult::default()
    }
}

fn append_timeout_stderr(stderr: &mut String, timeout_millis: u64) {
    if !stderr.is_empty() && !stderr.ends_with('\n') {
        stderr.push('\n');
    }
    stderr.push_str(&format!(
        "runtime probe timed out after {timeout_millis} ms"
    ));
}

fn parse_probe_report(text: &str) -> Option<Value> {
    text.lines().find_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        let schema_version = value.get("schemaVersion")?.as_u64()?;
        value.get("ok")?.as_bool()?;
        (schema_version == 1).then_some(value)
    })
}

fn is_default_plugin_kind(kind: &RuntimeProbePluginKind) -> bool {
    matches!(kind, RuntimeProbePluginKind::Unknown)
}

const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests;
