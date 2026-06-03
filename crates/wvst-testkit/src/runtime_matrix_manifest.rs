use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::runtime_matrix::{
    RuntimeProbeCase, RuntimeProbeMatrix, RuntimeProbeNote, RuntimeProbeParameterChange,
};

const RUNTIME_PROBE_MATRIX_SCHEMA_VERSION: u16 = 1;
const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;
const DEFAULT_MAX_BLOCK_FRAMES: u16 = 128;
const DEFAULT_CHANNEL_COUNT: u16 = 2;
const DEFAULT_FRAMES: u16 = 128;
const DEFAULT_BLOCKS: u32 = 1;
const DEFAULT_NOTE_VELOCITY_MILLI: u16 = 1_000;
const DEFAULT_NOTE_CHANNEL: u8 = 0;
const DEFAULT_PARAMETER_SAMPLE_OFFSET: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeMatrixManifest {
    pub schema_version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub cases: Vec<RuntimeProbeCaseManifest>,
}

impl RuntimeProbeMatrixManifest {
    pub fn from_json_str(text: &str) -> Result<Self, RuntimeProbeMatrixManifestError> {
        let manifest = serde_json::from_str::<Self>(text)
            .map_err(|error| RuntimeProbeMatrixManifestError::Json(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, RuntimeProbeMatrixManifestError> {
        self.validate()?;
        serde_json::to_string_pretty(self)
            .map_err(|error| RuntimeProbeMatrixManifestError::Json(error.to_string()))
    }

    pub fn to_matrix(
        &self,
        worker_executable: impl Into<PathBuf>,
    ) -> Result<RuntimeProbeMatrix, RuntimeProbeMatrixManifestError> {
        self.validate()?;
        let mut matrix = RuntimeProbeMatrix::new(worker_executable);
        for case in &self.cases {
            matrix.push_case(case.to_probe_case());
        }
        Ok(matrix)
    }

    pub fn validate(&self) -> Result<(), RuntimeProbeMatrixManifestError> {
        if self.schema_version != RUNTIME_PROBE_MATRIX_SCHEMA_VERSION {
            return Err(RuntimeProbeMatrixManifestError::UnsupportedSchemaVersion {
                expected: RUNTIME_PROBE_MATRIX_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.cases.is_empty() {
            return Err(RuntimeProbeMatrixManifestError::EmptyCases);
        }
        for case in &self.cases {
            case.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeCaseManifest {
    pub name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    #[serde(default = "default_sample_rate_hz")]
    pub sample_rate_hz: u32,
    #[serde(default = "default_max_block_frames")]
    pub max_block_frames: u16,
    #[serde(default = "default_channel_count")]
    pub input_channels: u16,
    #[serde(default = "default_channel_count")]
    pub output_channels: u16,
    #[serde(default = "default_frames")]
    pub frames: u16,
    #[serde(default = "default_blocks")]
    pub blocks: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<RuntimeProbeNoteManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_changes: Vec<RuntimeProbeParameterChangeManifest>,
}

impl RuntimeProbeCaseManifest {
    fn to_probe_case(&self) -> RuntimeProbeCase {
        let mut case = RuntimeProbeCase::new(
            self.name.clone(),
            self.plugin_path.clone(),
            self.class_id.clone(),
        )
        .with_processing(
            self.sample_rate_hz,
            self.max_block_frames,
            self.input_channels,
            self.output_channels,
            self.frames,
            self.blocks,
        );
        if let Some(note) = self.note {
            case = case.with_note(RuntimeProbeNote::new(
                note.pitch,
                note.velocity_milli,
                note.channel,
            ));
        }
        for change in &self.parameter_changes {
            case = case.with_parameter_change(RuntimeProbeParameterChange::new(
                change.parameter_id,
                change.value_milli,
                change.sample_offset,
            ));
        }
        case
    }

    fn validate(&self) -> Result<(), RuntimeProbeMatrixManifestError> {
        let case_name = self.name.clone();
        if self.name.trim().is_empty() {
            return Err(invalid_case(case_name, "name must not be empty"));
        }
        if self.plugin_path.as_os_str().is_empty() {
            return Err(invalid_case(case_name, "pluginPath must not be empty"));
        }
        if self.class_id.trim().is_empty() {
            return Err(invalid_case(case_name, "classId must not be empty"));
        }
        if self.sample_rate_hz == 0 {
            return Err(invalid_case(
                case_name,
                "sampleRateHz must be greater than 0",
            ));
        }
        if self.max_block_frames == 0 {
            return Err(invalid_case(
                case_name,
                "maxBlockFrames must be greater than 0",
            ));
        }
        if self.frames == 0 {
            return Err(invalid_case(case_name, "frames must be greater than 0"));
        }
        if self.frames > self.max_block_frames {
            return Err(invalid_case(
                case_name,
                "frames must be less than or equal to maxBlockFrames",
            ));
        }
        if self.blocks == 0 {
            return Err(invalid_case(case_name, "blocks must be greater than 0"));
        }
        if let Some(note) = self.note {
            note.validate(&case_name)?;
        }
        for change in &self.parameter_changes {
            change.validate(&case_name, self.frames)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeNoteManifest {
    pub pitch: u8,
    #[serde(default = "default_note_velocity_milli")]
    pub velocity_milli: u16,
    #[serde(default = "default_note_channel")]
    pub channel: u8,
}

impl RuntimeProbeNoteManifest {
    fn validate(&self, case_name: &str) -> Result<(), RuntimeProbeMatrixManifestError> {
        if self.pitch > 127 {
            return Err(invalid_case(case_name, "note pitch must be in 0..=127"));
        }
        if self.velocity_milli > 1_000 {
            return Err(invalid_case(
                case_name,
                "note velocityMilli must be in 0..=1000",
            ));
        }
        if self.channel > 15 {
            return Err(invalid_case(case_name, "note channel must be in 0..=15"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeParameterChangeManifest {
    pub parameter_id: u32,
    pub value_milli: u16,
    #[serde(default = "default_parameter_sample_offset")]
    pub sample_offset: u16,
}

impl RuntimeProbeParameterChangeManifest {
    fn validate(
        &self,
        case_name: &str,
        frames: u16,
    ) -> Result<(), RuntimeProbeMatrixManifestError> {
        if self.value_milli > 1_000 {
            return Err(invalid_case(
                case_name,
                "parameter valueMilli must be in 0..=1000",
            ));
        }
        if self.sample_offset >= frames {
            return Err(invalid_case(
                case_name,
                "parameter sampleOffset must be inside the processed block",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeProbeMatrixManifestError {
    Json(String),
    UnsupportedSchemaVersion { expected: u16, actual: u16 },
    EmptyCases,
    InvalidCase { case_name: String, message: String },
}

impl std::fmt::Display for RuntimeProbeMatrixManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(message) => write!(formatter, "invalid runtime matrix json: {message}"),
            Self::UnsupportedSchemaVersion { expected, actual } => write!(
                formatter,
                "unsupported runtime matrix schema version {actual}; expected {expected}"
            ),
            Self::EmptyCases => write!(formatter, "runtime matrix manifest must contain cases"),
            Self::InvalidCase { case_name, message } => {
                write!(
                    formatter,
                    "invalid runtime matrix case {case_name:?}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for RuntimeProbeMatrixManifestError {}

fn invalid_case(
    case_name: impl Into<String>,
    message: impl Into<String>,
) -> RuntimeProbeMatrixManifestError {
    RuntimeProbeMatrixManifestError::InvalidCase {
        case_name: case_name.into(),
        message: message.into(),
    }
}

const fn default_sample_rate_hz() -> u32 {
    DEFAULT_SAMPLE_RATE_HZ
}

const fn default_max_block_frames() -> u16 {
    DEFAULT_MAX_BLOCK_FRAMES
}

const fn default_channel_count() -> u16 {
    DEFAULT_CHANNEL_COUNT
}

const fn default_frames() -> u16 {
    DEFAULT_FRAMES
}

const fn default_blocks() -> u32 {
    DEFAULT_BLOCKS
}

const fn default_note_velocity_milli() -> u16 {
    DEFAULT_NOTE_VELOCITY_MILLI
}

const fn default_note_channel() -> u8 {
    DEFAULT_NOTE_CHANNEL
}

const fn default_parameter_sample_offset() -> u16 {
    DEFAULT_PARAMETER_SAMPLE_OFFSET
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_matrix::{RuntimeProbeExecutor, RuntimeProbeInvocation, RuntimeProbeResult};

    #[derive(Default)]
    struct RecordingExecutor {
        invocations: Vec<RuntimeProbeInvocation>,
    }

    impl RuntimeProbeExecutor for RecordingExecutor {
        fn run(&mut self, invocation: RuntimeProbeInvocation) -> RuntimeProbeResult {
            self.invocations.push(invocation);
            RuntimeProbeResult::default()
        }
    }

    #[test]
    fn parses_manifest_and_builds_matrix() {
        let manifest = RuntimeProbeMatrixManifest::from_json_str(
            r#"{"schemaVersion":1,"description":"local third-party VST3 smoke matrix","cases":[{"name":"instrument-note","pluginPath":"/Library/Audio/Plug-Ins/VST3/Synth.vst3","classId":"class-a","inputChannels":0,"outputChannels":2,"frames":128,"blocks":8,"note":{"pitch":60,"velocityMilli":750,"channel":1},"parameterChanges":[{"parameterId":42,"valueMilli":500,"sampleOffset":64}]}]}"#,
        )
        .expect("manifest");

        let matrix = manifest.to_matrix("/tmp/wvst-host-worker").expect("matrix");

        assert_eq!(manifest.cases[0].sample_rate_hz, DEFAULT_SAMPLE_RATE_HZ);
        assert_eq!(matrix.cases().len(), 1);
        assert_eq!(matrix.cases()[0].name, "instrument-note");
        assert_eq!(matrix.cases()[0].input_channels, 0);
        let mut executor = RecordingExecutor::default();
        let report = matrix.run_with(&mut executor);
        assert_eq!(
            report.schema_version,
            crate::runtime_matrix::RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION
        );
        assert!(report.all_passed());
        assert!(
            executor.invocations[0]
                .args
                .contains(&"60:0.750:1".to_string())
        );
        assert!(
            executor.invocations[0]
                .args
                .contains(&"42=0.500:64".to_string())
        );
        assert!(
            manifest
                .to_json_string_pretty()
                .expect("pretty json")
                .contains("\"schemaVersion\": 1")
        );
    }

    #[test]
    fn rejects_invalid_manifest() {
        let version_error = RuntimeProbeMatrixManifest::from_json_str(
            r#"{"schemaVersion":2,"cases":[{"name":"x","pluginPath":"/tmp/X.vst3","classId":"a"}]}"#,
        )
        .expect_err("unsupported schema");
        assert!(matches!(
            version_error,
            RuntimeProbeMatrixManifestError::UnsupportedSchemaVersion {
                expected: RUNTIME_PROBE_MATRIX_SCHEMA_VERSION,
                actual: 2,
            }
        ));

        let case_error = RuntimeProbeMatrixManifest::from_json_str(
            r#"{"schemaVersion":1,"cases":[{"name":"bad-offset","pluginPath":"/tmp/X.vst3","classId":"a","frames":32,"parameterChanges":[{"parameterId":1,"valueMilli":1001,"sampleOffset":0}]}]}"#,
        )
        .expect_err("invalid case");
        assert!(matches!(
            case_error,
            RuntimeProbeMatrixManifestError::InvalidCase { case_name, message }
                if case_name == "bad-offset" && message.contains("valueMilli")
        ));
    }
}
