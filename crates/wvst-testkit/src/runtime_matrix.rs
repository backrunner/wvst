use std::{path::PathBuf, process::Command, time::Instant};

use serde::Serialize;
use serde_json::Value;

pub const RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeCase {
    pub name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    pub sample_rate_hz: u32,
    pub max_block_frames: u16,
    pub input_channels: u16,
    pub output_channels: u16,
    pub frames: u16,
    pub blocks: u32,
    pub note: Option<RuntimeProbeNote>,
    pub parameter_changes: Vec<RuntimeProbeParameterChange>,
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
            sample_rate_hz: 48_000,
            max_block_frames: 128,
            input_channels: 2,
            output_channels: 2,
            frames: 128,
            blocks: 1,
            note: None,
            parameter_changes: Vec::new(),
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

    pub fn with_parameter_change(mut self, change: RuntimeProbeParameterChange) -> Self {
        self.parameter_changes.push(change);
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
        for change in &self.parameter_changes {
            args.push("--parameter-change".to_string());
            args.push(change.to_arg());
        }
        args
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
                };
                let started = Instant::now();
                let mut result = executor.run(invocation);
                result.case_name = case.name.clone();
                result.plugin_path = case.plugin_path.clone();
                result.class_id = case.class_id.clone();
                result.duration_millis = started.elapsed().as_millis() as u64;
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
}

pub trait RuntimeProbeExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProcessRuntimeProbeExecutor;

impl RuntimeProbeExecutor for ProcessRuntimeProbeExecutor {
    fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
        match Command::new(&invocation.worker_executable)
            .args(&invocation.args)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                RuntimeProbeResult {
                    status: if output.status.success() {
                        RuntimeProbeStatus::Passed
                    } else {
                        RuntimeProbeStatus::Failed
                    },
                    exit_code: output.status.code(),
                    probe_report: parse_probe_report(&stdout)
                        .or_else(|| parse_probe_report(&stderr)),
                    stdout,
                    stderr,
                    ..RuntimeProbeResult::default()
                }
            }
            Err(error) => RuntimeProbeResult {
                status: RuntimeProbeStatus::LaunchFailed,
                stderr: error.to_string(),
                ..RuntimeProbeResult::default()
            },
        }
    }
}

fn parse_probe_report(text: &str) -> Option<Value> {
    text.lines().find_map(|line| {
        let value = serde_json::from_str::<Value>(line).ok()?;
        let schema_version = value.get("schemaVersion")?.as_u64()?;
        value.get("ok")?.as_bool()?;
        (schema_version == 1).then_some(value)
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbeStatus {
    #[default]
    Passed,
    Failed,
    LaunchFailed,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeResult {
    pub case_name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    pub status: RuntimeProbeStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_report: Option<Value>,
    pub duration_millis: u64,
}

impl RuntimeProbeResult {
    pub const fn passed(&self) -> bool {
        matches!(self.status, RuntimeProbeStatus::Passed)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeMatrixReport {
    pub schema_version: u16,
    pub results: Vec<RuntimeProbeResult>,
    pub passed: usize,
    pub failed: usize,
    pub launch_failed: usize,
    pub audio_health: RuntimeProbeAudioHealthSummary,
}

impl RuntimeProbeMatrixReport {
    fn new(results: Vec<RuntimeProbeResult>) -> Self {
        let passed = results.iter().filter(|result| result.passed()).count();
        let failed = count_status(&results, RuntimeProbeStatus::Failed);
        let launch_failed = count_status(&results, RuntimeProbeStatus::LaunchFailed);
        let audio_health = RuntimeProbeAudioHealthSummary::from_results(&results);

        Self {
            schema_version: RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION,
            results,
            passed,
            failed,
            launch_failed,
            audio_health,
        }
    }

    pub const fn all_passed(&self) -> bool {
        self.failed == 0 && self.launch_failed == 0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeAudioHealthSummary {
    pub reported_cases: usize,
    pub fully_silent_cases: usize,
    pub non_zero_cases: usize,
    pub non_finite_cases: usize,
    pub clipped_cases: usize,
    pub total_non_finite_output_samples: u64,
    pub total_clipped_output_samples: u64,
    pub total_silent_output_blocks: u64,
    pub max_output_peak: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_peak_case: Option<String>,
    pub max_output_rms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_rms_case: Option<String>,
}

impl RuntimeProbeAudioHealthSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(process) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("process"))
            else {
                continue;
            };

            summary.reported_cases += 1;
            let total_blocks = u64_field(process, "totalBlocks");
            let silent_blocks = u64_field(process, "silentOutputBlocks");
            let non_zero_blocks = u64_field(process, "nonZeroOutputBlocks");
            let non_finite_samples = u64_field(process, "nonFiniteOutputSamples");
            let clipped_samples = u64_field(process, "clippedOutputSamples");
            let peak = f64_field(process, "maxOutputPeak");
            let rms = f64_field(process, "outputRms");

            if total_blocks > 0 && silent_blocks == total_blocks {
                summary.fully_silent_cases += 1;
            }
            if non_zero_blocks > 0 {
                summary.non_zero_cases += 1;
            }
            if non_finite_samples > 0 {
                summary.non_finite_cases += 1;
            }
            if clipped_samples > 0 {
                summary.clipped_cases += 1;
            }
            summary.total_non_finite_output_samples = summary
                .total_non_finite_output_samples
                .saturating_add(non_finite_samples);
            summary.total_clipped_output_samples = summary
                .total_clipped_output_samples
                .saturating_add(clipped_samples);
            summary.total_silent_output_blocks = summary
                .total_silent_output_blocks
                .saturating_add(silent_blocks);

            if peak > summary.max_output_peak {
                summary.max_output_peak = peak;
                summary.max_output_peak_case = Some(result.case_name.clone());
            }
            if rms > summary.max_output_rms {
                summary.max_output_rms = rms;
                summary.max_output_rms_case = Some(result.case_name.clone());
            }
        }

        summary
    }
}

fn count_status(results: &[RuntimeProbeResult], status: RuntimeProbeStatus) -> usize {
    results
        .iter()
        .filter(|result| result.status == status)
        .count()
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn f64_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

#[cfg(test)]
mod tests;
