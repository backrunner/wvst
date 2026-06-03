use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::runtime_matrix::RuntimeProbePluginKind;

use super::{RuntimeProbeCaseManifest, RuntimeProbeMatrixManifestError};

#[path = "evidence_quality.rs"]
mod evidence_quality;

use evidence_quality::validate_third_party_expectation_quality;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeEvidenceRequirements {
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_all_cases_evidence: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_reported_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_effect_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_instrument_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_hybrid_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_non_silent_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_note_response_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_controller_rich_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_output_event_cases: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_blocks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_process_frames: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_third_party_timeout_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_audio_health_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_process_timing_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_runtime_health_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_note_response_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_controller_rich_expectations: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_third_party_output_event_expectations: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub required_tag_counts: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub required_third_party_tag_counts: BTreeMap<String, usize>,
}

pub(super) fn validate_evidence_requirements(
    cases: &[RuntimeProbeCaseManifest],
    requirements: &RuntimeProbeEvidenceRequirements,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    validate_required_tags(&requirements.required_tags)?;
    validate_required_tag_counts("requiredTagCounts", &requirements.required_tag_counts)?;
    validate_required_tag_counts(
        "requiredThirdPartyTagCounts",
        &requirements.required_third_party_tag_counts,
    )?;
    validate_optional_min_u32("minThirdPartyBlocks", requirements.min_third_party_blocks)?;
    validate_optional_min_u64(
        "minThirdPartyProcessFrames",
        requirements.min_third_party_process_frames,
    )?;
    validate_optional_min_u64(
        "minThirdPartyTimeoutMillis",
        requirements.min_third_party_timeout_millis,
    )?;
    let counts = RuntimeProbeManifestEvidenceCounts::from_cases(cases);

    if requirements.require_all_cases_evidence && counts.reported_cases != cases.len() {
        return Err(invalid_evidence_requirements(format!(
            "requireAllCasesEvidence expected {} cases with evidence, got {}",
            cases.len(),
            counts.reported_cases
        )));
    }
    require_min_count(
        "minReportedCases",
        requirements.min_reported_cases,
        counts.reported_cases,
    )?;
    require_min_count(
        "minThirdPartyCases",
        requirements.min_third_party_cases,
        counts.third_party_cases,
    )?;
    require_min_count(
        "minThirdPartyEffectCases",
        requirements.min_third_party_effect_cases,
        counts.third_party_effect_cases,
    )?;
    require_min_count(
        "minThirdPartyInstrumentCases",
        requirements.min_third_party_instrument_cases,
        counts.third_party_instrument_cases,
    )?;
    require_min_count(
        "minThirdPartyHybridCases",
        requirements.min_third_party_hybrid_cases,
        counts.third_party_hybrid_cases,
    )?;
    validate_runtime_coverage_target(
        "minThirdPartyNonSilentCases",
        requirements.min_third_party_non_silent_cases,
        counts.third_party_cases,
    )?;
    validate_runtime_coverage_target(
        "minThirdPartyNoteResponseCases",
        requirements.min_third_party_note_response_cases,
        counts.third_party_cases,
    )?;
    validate_runtime_coverage_target(
        "minThirdPartyControllerRichCases",
        requirements.min_third_party_controller_rich_cases,
        counts.third_party_cases,
    )?;
    validate_runtime_coverage_target(
        "minThirdPartyOutputEventCases",
        requirements.min_third_party_output_event_cases,
        counts.third_party_cases,
    )?;

    for tag in &requirements.required_tags {
        let trimmed = tag.trim();
        if !counts.tags.contains_key(trimmed) {
            return Err(invalid_evidence_requirements(format!(
                "requiredTags missing tag {trimmed:?}"
            )));
        }
    }
    for (tag, expected) in &requirements.required_tag_counts {
        let trimmed = tag.trim();
        require_min_count(
            "requiredTagCounts",
            Some(*expected),
            counts.tags.get(trimmed).copied().unwrap_or(0),
        )
        .map_err(|_| {
            invalid_evidence_requirements(format!(
                "requiredTagCounts tag {trimmed:?} expected at least {expected}, got {}",
                counts.tags.get(trimmed).copied().unwrap_or(0)
            ))
        })?;
    }
    for (tag, expected) in &requirements.required_third_party_tag_counts {
        let trimmed = tag.trim();
        require_min_count(
            "requiredThirdPartyTagCounts",
            Some(*expected),
            counts.third_party_tags.get(trimmed).copied().unwrap_or(0),
        )
        .map_err(|_| {
            invalid_evidence_requirements(format!(
                "requiredThirdPartyTagCounts tag {trimmed:?} expected at least {expected}, got {}",
                counts.third_party_tags.get(trimmed).copied().unwrap_or(0)
            ))
        })?;
    }
    validate_third_party_case_quality(cases, requirements)?;

    Ok(())
}

fn validate_third_party_case_quality(
    cases: &[RuntimeProbeCaseManifest],
    requirements: &RuntimeProbeEvidenceRequirements,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    for case in cases {
        let Some(evidence) = &case.evidence else {
            continue;
        };
        if !evidence.third_party {
            continue;
        }
        if requirements.require_third_party_expectations && case.expectations.is_none() {
            return Err(invalid_evidence_requirements(format!(
                "requireThirdPartyExpectations expected case {:?} to declare expectations",
                case.name
            )));
        }
        if let Some(min_blocks) = requirements.min_third_party_blocks
            && case.blocks < min_blocks
        {
            return Err(invalid_evidence_requirements(format!(
                "minThirdPartyBlocks expected each third-party case to run at least {min_blocks} blocks; case {:?} runs {}",
                case.name, case.blocks
            )));
        }
        if let Some(min_frames) = requirements.min_third_party_process_frames {
            let process_frames = u64::from(case.frames).saturating_mul(u64::from(case.blocks));
            if process_frames < min_frames {
                return Err(invalid_evidence_requirements(format!(
                    "minThirdPartyProcessFrames expected each third-party case to process at least {min_frames} frames; case {:?} processes {process_frames}",
                    case.name
                )));
            }
        }
        if let Some(min_timeout) = requirements.min_third_party_timeout_millis
            && case.timeout_millis < min_timeout
        {
            return Err(invalid_evidence_requirements(format!(
                "minThirdPartyTimeoutMillis expected each third-party case timeout at least {min_timeout} ms; case {:?} uses {} ms",
                case.name, case.timeout_millis
            )));
        }
        validate_third_party_expectation_quality(case, requirements)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct RuntimeProbeManifestEvidenceCounts {
    reported_cases: usize,
    third_party_cases: usize,
    third_party_effect_cases: usize,
    third_party_instrument_cases: usize,
    third_party_hybrid_cases: usize,
    tags: BTreeMap<String, usize>,
    third_party_tags: BTreeMap<String, usize>,
}

impl RuntimeProbeManifestEvidenceCounts {
    fn from_cases(cases: &[RuntimeProbeCaseManifest]) -> Self {
        let mut counts = Self::default();
        for case in cases {
            let Some(evidence) = &case.evidence else {
                continue;
            };
            counts.reported_cases += 1;
            for tag in &evidence.tags {
                increment_tag(&mut counts.tags, tag);
            }
            if !evidence.third_party {
                continue;
            }
            counts.third_party_cases += 1;
            for tag in &evidence.tags {
                increment_tag(&mut counts.third_party_tags, tag);
            }
            match evidence.plugin_kind {
                RuntimeProbePluginKind::Effect => counts.third_party_effect_cases += 1,
                RuntimeProbePluginKind::Instrument => counts.third_party_instrument_cases += 1,
                RuntimeProbePluginKind::Hybrid => counts.third_party_hybrid_cases += 1,
                RuntimeProbePluginKind::Unknown => {}
            }
        }
        counts
    }
}

fn increment_tag(map: &mut BTreeMap<String, usize>, tag: &str) {
    let trimmed = tag.trim();
    if !trimmed.is_empty() {
        *map.entry(trimmed.to_string()).or_default() += 1;
    }
}

fn validate_required_tags(tags: &[String]) -> Result<(), RuntimeProbeMatrixManifestError> {
    let mut seen = BTreeSet::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(invalid_evidence_requirements(
                "requiredTags must not contain empty tags",
            ));
        }
        if !seen.insert(trimmed) {
            return Err(invalid_evidence_requirements(format!(
                "requiredTags contains duplicate tag {trimmed:?}"
            )));
        }
    }
    Ok(())
}

fn validate_required_tag_counts(
    field_name: &'static str,
    tags: &BTreeMap<String, usize>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let mut seen = BTreeSet::new();
    for (tag, count) in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return Err(invalid_evidence_requirements(format!(
                "{field_name} must not contain empty tags"
            )));
        }
        if !seen.insert(trimmed) {
            return Err(invalid_evidence_requirements(format!(
                "{field_name} contains duplicate tag {trimmed:?} after trimming"
            )));
        }
        if *count == 0 {
            return Err(invalid_evidence_requirements(format!(
                "{field_name} tag {trimmed:?} must be greater than 0"
            )));
        }
    }
    Ok(())
}

fn validate_optional_min_u32(
    field_name: &'static str,
    value: Option<u32>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if matches!(value, Some(0)) {
        return Err(invalid_evidence_requirements(format!(
            "{field_name} must be greater than 0"
        )));
    }
    Ok(())
}

fn validate_optional_min_u64(
    field_name: &'static str,
    value: Option<u64>,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if matches!(value, Some(0)) {
        return Err(invalid_evidence_requirements(format!(
            "{field_name} must be greater than 0"
        )));
    }
    Ok(())
}

fn require_min_count(
    field_name: &'static str,
    expected: Option<usize>,
    actual: usize,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    if let Some(expected) = expected
        && actual < expected
    {
        return Err(invalid_evidence_requirements(format!(
            "{field_name} expected at least {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn validate_runtime_coverage_target(
    field_name: &'static str,
    expected: Option<usize>,
    third_party_cases: usize,
) -> Result<(), RuntimeProbeMatrixManifestError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected == 0 {
        return Err(invalid_evidence_requirements(format!(
            "{field_name} must be greater than 0"
        )));
    }
    if expected > third_party_cases {
        return Err(invalid_evidence_requirements(format!(
            "{field_name} expected at most {third_party_cases} third-party cases declared in the manifest, got {expected}"
        )));
    }
    Ok(())
}

fn invalid_evidence_requirements(message: impl Into<String>) -> RuntimeProbeMatrixManifestError {
    RuntimeProbeMatrixManifestError::InvalidEvidenceRequirements {
        message: message.into(),
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
}
