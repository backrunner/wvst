use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::runtime_matrix::{
    DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS, RuntimeProbeCase, RuntimeProbeCaseEvidence,
    RuntimeProbeExpectations, RuntimeProbeMatrix, RuntimeProbeNote, RuntimeProbeParameterChange,
    RuntimeProbePluginKind,
};

#[path = "runtime_matrix_manifest/coverage_tags.rs"]
mod coverage_tags;
#[path = "runtime_matrix_manifest/evidence_requirements.rs"]
mod evidence_requirements;
#[path = "runtime_matrix_manifest/expectations.rs"]
mod expectations;

use coverage_tags::validate_coverage_tags;
pub use evidence_requirements::RuntimeProbeEvidenceRequirements;
use evidence_requirements::validate_evidence_requirements;
use expectations::validate_expectations;

const RUNTIME_PROBE_MATRIX_SCHEMA_VERSION: u16 = 1;
const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;
const DEFAULT_MAX_BLOCK_FRAMES: u16 = 128;
const DEFAULT_CHANNEL_COUNT: u16 = 2;
const DEFAULT_FRAMES: u16 = 128;
const DEFAULT_BLOCKS: u32 = 1;
const DEFAULT_CONTROLLER_EDIT_PROBE: bool = true;
const DEFAULT_CONNECTION_NOTIFY_PROBE: bool = true;
const DEFAULT_STATE_ROUNDTRIP_PROBE: bool = true;
const DEFAULT_NOTE_VELOCITY_MILLI: u16 = 1_000;
const DEFAULT_NOTE_CHANNEL: u8 = 0;
const DEFAULT_PARAMETER_SAMPLE_OFFSET: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeMatrixManifest {
    pub schema_version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_requirements: Option<RuntimeProbeEvidenceRequirements>,
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
        let fixture_root = self
            .fixture_root
            .as_ref()
            .map(|path| expand_path(path, "<matrix>"))
            .transpose()?;
        let mut matrix = RuntimeProbeMatrix::new(worker_executable);
        if let Some(requirements) = &self.evidence_requirements {
            matrix = matrix.with_coverage_requirements(requirements.clone());
        }
        for case in &self.cases {
            matrix.push_case(case.to_probe_case(fixture_root.as_deref())?);
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
        if let Some(requirements) = &self.evidence_requirements {
            validate_evidence_requirements(&self.cases, requirements)?;
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
    #[serde(
        default = "default_controller_edit_probe",
        skip_serializing_if = "is_true"
    )]
    pub controller_edit_probe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_edit_probe_parameter_id: Option<u32>,
    #[serde(
        default = "default_connection_notify_probe",
        skip_serializing_if = "is_true"
    )]
    pub connection_notify_probe: bool,
    #[serde(
        default = "default_state_roundtrip_probe",
        skip_serializing_if = "is_true"
    )]
    pub state_roundtrip_probe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<RuntimeProbeCaseEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<RuntimeProbeNoteManifest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_changes: Vec<RuntimeProbeParameterChangeManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expectations: Option<RuntimeProbeExpectations>,
    #[serde(
        default = "default_runtime_probe_timeout_millis",
        skip_serializing_if = "is_default_runtime_probe_timeout_millis"
    )]
    pub timeout_millis: u64,
}

impl RuntimeProbeCaseManifest {
    fn to_probe_case(
        &self,
        fixture_root: Option<&Path>,
    ) -> Result<RuntimeProbeCase, RuntimeProbeMatrixManifestError> {
        let plugin_path = resolve_plugin_path(&self.plugin_path, fixture_root, &self.name)?;
        let mut case = RuntimeProbeCase::new(self.name.clone(), plugin_path, self.class_id.clone())
            .with_processing(
                self.sample_rate_hz,
                self.max_block_frames,
                self.input_channels,
                self.output_channels,
                self.frames,
                self.blocks,
            )
            .with_timeout_millis(self.timeout_millis)
            .with_controller_edit_probe(self.controller_edit_probe)
            .with_connection_notify_probe(self.connection_notify_probe)
            .with_state_roundtrip_probe(self.state_roundtrip_probe);
        if let Some(parameter_id) = self.controller_edit_probe_parameter_id {
            case = case.with_controller_edit_probe_parameter_id(parameter_id);
        }
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
        if let Some(evidence) = &self.evidence {
            case = case.with_evidence(evidence.clone());
        }
        if let Some(expectations) = &self.expectations {
            case = case.with_expectations(expectations.clone());
        }
        Ok(case)
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
        if self.timeout_millis == 0 {
            return Err(invalid_case(
                case_name,
                "timeoutMillis must be greater than 0",
            ));
        }
        if let Some(note) = self.note {
            note.validate(&case_name)?;
        }
        for change in &self.parameter_changes {
            change.validate(&case_name, self.frames)?;
        }
        if let Some(evidence) = &self.evidence {
            validate_evidence(&case_name, evidence)?;
        }
        if let Some(expectations) = &self.expectations {
            validate_expectations(&case_name, expectations)?;
        }
        validate_coverage_tags(self)?;
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
    InvalidEvidenceRequirements { message: String },
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
            Self::InvalidEvidenceRequirements { message } => {
                write!(
                    formatter,
                    "invalid runtime matrix evidence requirements: {message}"
                )
            }
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

fn validate_evidence(
    case_name: &str,
    evidence: &RuntimeProbeCaseEvidence,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    validate_optional_text(case_name, "evidence.pluginName", &evidence.plugin_name)?;
    validate_optional_text(case_name, "evidence.vendor", &evidence.vendor)?;
    validate_optional_text(
        case_name,
        "evidence.pluginVersion",
        &evidence.plugin_version,
    )?;
    validate_optional_text(
        case_name,
        "evidence.validationNotes",
        &evidence.validation_notes,
    )?;

    if evidence.third_party {
        require_evidence_text(case_name, "evidence.vendor", &evidence.vendor)?;
        require_evidence_text(case_name, "evidence.pluginName", &evidence.plugin_name)?;
        if evidence.plugin_kind == RuntimeProbePluginKind::Unknown {
            return Err(invalid_case(
                case_name,
                "third-party evidence must set evidence.pluginKind to effect, instrument, or hybrid",
            ));
        }
    }

    let mut tags = BTreeSet::new();
    for tag in &evidence.tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(invalid_case(
                case_name,
                "evidence.tags must not contain empty tags",
            ));
        }
        if !tags.insert(trimmed) {
            return Err(invalid_case(
                case_name,
                format!("evidence.tags contains duplicate tag {trimmed:?}"),
            ));
        }
    }

    Ok(())
}

fn validate_optional_text(
    case_name: &str,
    field_name: &str,
    value: &Option<String>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if let Some(value) = value
        && value.trim().is_empty()
    {
        return Err(invalid_case(
            case_name,
            format!("{field_name} must not be empty when set"),
        ));
    }
    Ok(())
}

fn require_evidence_text(
    case_name: &str,
    field_name: &str,
    value: &Option<String>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if value
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return Ok(());
    }
    Err(invalid_case(
        case_name,
        format!("{field_name} is required for third-party evidence"),
    ))
}

fn resolve_plugin_path(
    plugin_path: &Path,
    fixture_root: Option<&Path>,
    case_name: &str,
) -> Result<PathBuf, RuntimeProbeMatrixManifestError> {
    let expanded = expand_path(plugin_path, case_name)?;
    if expanded.is_absolute() {
        return Ok(expanded);
    }

    Ok(fixture_root
        .map(|root| root.join(&expanded))
        .unwrap_or(expanded))
}

fn expand_path(path: &Path, case_name: &str) -> Result<PathBuf, RuntimeProbeMatrixManifestError> {
    let Some(text) = path.to_str() else {
        return Ok(path.to_path_buf());
    };
    let expanded_home = expand_home(text, case_name)?;
    expand_env_vars(&expanded_home, case_name).map(PathBuf::from)
}

fn expand_home(text: &str, case_name: &str) -> Result<String, RuntimeProbeMatrixManifestError> {
    if text == "~" || text.starts_with("~/") {
        let home = std::env::var("HOME").map_err(|_| {
            invalid_case(
                case_name,
                "path uses ~/ but HOME environment variable is not set",
            )
        })?;
        return Ok(format!("{home}{}", &text[1..]));
    }
    Ok(text.to_string())
}

fn expand_env_vars(text: &str, case_name: &str) -> Result<String, RuntimeProbeMatrixManifestError> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            return Err(invalid_case(
                case_name,
                "path contains an unterminated ${VAR}",
            ));
        };
        let variable = &after_start[..end];
        if variable.is_empty() {
            return Err(invalid_case(
                case_name,
                "path contains an empty ${} variable",
            ));
        }
        let value = std::env::var(variable).map_err(|_| {
            invalid_case(
                case_name,
                format!("path references unset environment variable {variable:?}"),
            )
        })?;
        output.push_str(&value);
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

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

const fn default_controller_edit_probe() -> bool {
    DEFAULT_CONTROLLER_EDIT_PROBE
}

const fn default_connection_notify_probe() -> bool {
    DEFAULT_CONNECTION_NOTIFY_PROBE
}

const fn default_state_roundtrip_probe() -> bool {
    DEFAULT_STATE_ROUNDTRIP_PROBE
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

const fn default_runtime_probe_timeout_millis() -> u64 {
    DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS
}

const fn is_true(value: &bool) -> bool {
    *value
}

fn is_default_runtime_probe_timeout_millis(value: &u64) -> bool {
    *value == DEFAULT_RUNTIME_PROBE_TIMEOUT_MILLIS
}

#[cfg(test)]
#[path = "runtime_matrix_manifest_tests.rs"]
mod tests;
