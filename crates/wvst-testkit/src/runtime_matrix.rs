use std::{path::PathBuf, process::Command, time::Instant};

use serde::Serialize;

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
            Ok(output) => RuntimeProbeResult {
                status: if output.status.success() {
                    RuntimeProbeStatus::Passed
                } else {
                    RuntimeProbeStatus::Failed
                },
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                ..RuntimeProbeResult::default()
            },
            Err(error) => RuntimeProbeResult {
                status: RuntimeProbeStatus::LaunchFailed,
                stderr: error.to_string(),
                ..RuntimeProbeResult::default()
            },
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbeStatus {
    #[default]
    Passed,
    Failed,
    LaunchFailed,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeResult {
    pub case_name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    pub status: RuntimeProbeStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_millis: u64,
}

impl RuntimeProbeResult {
    pub const fn passed(&self) -> bool {
        matches!(self.status, RuntimeProbeStatus::Passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeMatrixReport {
    pub results: Vec<RuntimeProbeResult>,
    pub passed: usize,
    pub failed: usize,
    pub launch_failed: usize,
}

impl RuntimeProbeMatrixReport {
    fn new(results: Vec<RuntimeProbeResult>) -> Self {
        let passed = results.iter().filter(|result| result.passed()).count();
        let failed = count_status(&results, RuntimeProbeStatus::Failed);
        let launch_failed = count_status(&results, RuntimeProbeStatus::LaunchFailed);

        Self {
            results,
            passed,
            failed,
            launch_failed,
        }
    }

    pub const fn all_passed(&self) -> bool {
        self.failed == 0 && self.launch_failed == 0
    }
}

fn count_status(results: &[RuntimeProbeResult], status: RuntimeProbeStatus) -> usize {
    results
        .iter()
        .filter(|result| result.status == status)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingExecutor {
        invocations: Vec<RuntimeProbeInvocation>,
        results: Vec<RuntimeProbeResult>,
    }

    impl RuntimeProbeExecutor for RecordingExecutor {
        fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
            self.invocations.push(invocation);
            self.results.pop().unwrap_or_default()
        }
    }

    #[test]
    fn builds_runtime_probe_invocation_with_note_and_parameter_change() {
        let mut executor = RecordingExecutor::default();
        let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker").with_case(
            RuntimeProbeCase::new(
                "synth",
                "/Library/Audio/Plug-Ins/VST3/Synth.vst3",
                "class-a",
            )
            .with_processing(48_000, 256, 0, 2, 128, 8)
            .with_note(RuntimeProbeNote::new(60, 750, 1))
            .with_parameter_change(RuntimeProbeParameterChange::new(42, 500, 64)),
        );

        let report = matrix.run_with(&mut executor);

        assert!(report.all_passed());
        let invocation = &executor.invocations[0];
        assert_eq!(
            invocation.worker_executable,
            PathBuf::from("/tmp/wvst-host-worker")
        );
        assert_eq!(invocation.args[0], "runtime-probe");
        assert_eq!(
            invocation.args[1],
            "/Library/Audio/Plug-Ins/VST3/Synth.vst3"
        );
        assert_eq!(invocation.args[2], "class-a");
        assert!(invocation.args.contains(&"--input-channels".to_string()));
        assert!(invocation.args.contains(&"0".to_string()));
        assert!(invocation.args.contains(&"--note".to_string()));
        assert!(invocation.args.contains(&"60:0.750:1".to_string()));
        assert!(invocation.args.contains(&"--parameter-change".to_string()));
        assert!(invocation.args.contains(&"42=0.500:64".to_string()));
    }

    #[test]
    fn summarizes_failed_and_launch_failed_cases() {
        let mut executor = RecordingExecutor {
            results: vec![
                RuntimeProbeResult {
                    status: RuntimeProbeStatus::LaunchFailed,
                    ..RuntimeProbeResult::default()
                },
                RuntimeProbeResult {
                    status: RuntimeProbeStatus::Failed,
                    exit_code: Some(2),
                    ..RuntimeProbeResult::default()
                },
            ],
            ..RecordingExecutor::default()
        };
        let matrix = RuntimeProbeMatrix::new("/tmp/wvst-host-worker")
            .with_case(RuntimeProbeCase::new(
                "broken",
                "/tmp/Broken.vst3",
                "class-a",
            ))
            .with_case(RuntimeProbeCase::new(
                "missing",
                "/tmp/Missing.vst3",
                "class-b",
            ));

        let report = matrix.run_with(&mut executor);

        assert!(!report.all_passed());
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(report.launch_failed, 1);
        assert_eq!(report.results[0].case_name, "broken");
        assert_eq!(report.results[1].class_id, "class-b");
    }
}
