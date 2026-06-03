use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::runtime_matrix_manifest::RuntimeProbeEvidenceRequirements;

use super::{RuntimeProbePluginKind, RuntimeProbeResult};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeCoverageAudit {
    pub checked: bool,
    pub passed: bool,
    pub violations: Vec<String>,
    pub observed: RuntimeProbeCoverageObserved,
}

impl RuntimeProbeCoverageAudit {
    pub(crate) fn from_results(
        results: &[RuntimeProbeResult],
        requirements: Option<&RuntimeProbeEvidenceRequirements>,
    ) -> Self {
        let observed = RuntimeProbeCoverageObserved::from_results(results);
        let Some(requirements) = requirements else {
            return Self {
                checked: false,
                passed: true,
                violations: Vec::new(),
                observed,
            };
        };

        let mut audit = Self {
            checked: true,
            passed: true,
            violations: Vec::new(),
            observed,
        };
        audit.evaluate(results, requirements);
        audit.passed = audit.violations.is_empty();
        audit
    }

    fn evaluate(
        &mut self,
        results: &[RuntimeProbeResult],
        requirements: &RuntimeProbeEvidenceRequirements,
    ) {
        if requirements.require_all_cases_evidence
            && self.observed.reported_cases != self.observed.total_cases
        {
            self.violations.push(format!(
                "requireAllCasesEvidence expected {} cases with evidence, got {}",
                self.observed.total_cases, self.observed.reported_cases
            ));
        }
        self.require_min(
            "minReportedCases",
            requirements.min_reported_cases,
            self.observed.reported_cases,
        );
        self.require_min(
            "minThirdPartyCases",
            requirements.min_third_party_cases,
            self.observed.third_party_cases,
        );
        self.require_min(
            "minThirdPartyEffectCases",
            requirements.min_third_party_effect_cases,
            self.observed.third_party_effect_cases,
        );
        self.require_min(
            "minThirdPartyInstrumentCases",
            requirements.min_third_party_instrument_cases,
            self.observed.third_party_instrument_cases,
        );
        self.require_min(
            "minThirdPartyHybridCases",
            requirements.min_third_party_hybrid_cases,
            self.observed.third_party_hybrid_cases,
        );
        self.require_min(
            "minThirdPartyNonSilentCases",
            requirements.min_third_party_non_silent_cases,
            self.observed.third_party_non_silent_cases,
        );
        self.require_min(
            "minThirdPartyNoteResponseCases",
            requirements.min_third_party_note_response_cases,
            self.observed.third_party_note_response_cases,
        );
        self.require_min(
            "minThirdPartyControllerRichCases",
            requirements.min_third_party_controller_rich_cases,
            self.observed.third_party_controller_rich_cases,
        );
        self.require_min(
            "minThirdPartyOutputEventCases",
            requirements.min_third_party_output_event_cases,
            self.observed.third_party_output_event_cases,
        );
        self.require_tags(&requirements.required_tags);
        self.require_tag_counts(
            "requiredTagCounts",
            &requirements.required_tag_counts,
            false,
        );
        self.require_tag_counts(
            "requiredThirdPartyTagCounts",
            &requirements.required_third_party_tag_counts,
            true,
        );
        self.require_third_party_runtime_minimums(results, requirements);
    }

    fn require_min(&mut self, field_name: &'static str, expected: Option<usize>, actual: usize) {
        if let Some(expected) = expected
            && actual < expected
        {
            self.violations.push(format!(
                "{field_name} expected at least {expected}, got {actual}"
            ));
        }
    }

    fn require_tags(&mut self, tags: &[String]) {
        for tag in tags {
            let trimmed = tag.trim();
            if !self.observed.tags.contains_key(trimmed) {
                self.violations
                    .push(format!("requiredTags missing tag {trimmed:?}"));
            }
        }
    }

    fn require_tag_counts(
        &mut self,
        field_name: &'static str,
        tags: &BTreeMap<String, usize>,
        third_party_only: bool,
    ) {
        for (tag, expected) in tags {
            let trimmed = tag.trim();
            let actual = if third_party_only {
                self.observed
                    .third_party_tags
                    .get(trimmed)
                    .copied()
                    .unwrap_or(0)
            } else {
                self.observed.tags.get(trimmed).copied().unwrap_or(0)
            };
            if actual < *expected {
                self.violations.push(format!(
                    "{field_name} tag {trimmed:?} expected at least {expected}, got {actual}"
                ));
            }
        }
    }

    fn require_third_party_runtime_minimums(
        &mut self,
        results: &[RuntimeProbeResult],
        requirements: &RuntimeProbeEvidenceRequirements,
    ) {
        for result in results
            .iter()
            .filter(|result| result_is_third_party(result))
        {
            if let Some(min_blocks) = requirements.min_third_party_blocks {
                let blocks = process_u64(result, "totalBlocks");
                if blocks < u64::from(min_blocks) {
                    self.violations.push(format!(
                        "minThirdPartyBlocks expected case {:?} to report at least {min_blocks} processed blocks, got {blocks}",
                        result.case_name
                    ));
                }
            }
            if let Some(min_frames) = requirements.min_third_party_process_frames {
                let frames = process_u64(result, "totalFrames");
                if frames < min_frames {
                    self.violations.push(format!(
                        "minThirdPartyProcessFrames expected case {:?} to report at least {min_frames} processed frames, got {frames}",
                        result.case_name
                    ));
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeCoverageObserved {
    pub total_cases: usize,
    pub reported_cases: usize,
    pub missing_evidence_cases: usize,
    pub third_party_cases: usize,
    pub third_party_effect_cases: usize,
    pub third_party_instrument_cases: usize,
    pub third_party_hybrid_cases: usize,
    pub third_party_passed_cases: usize,
    pub third_party_non_silent_cases: usize,
    pub third_party_note_response_cases: usize,
    pub third_party_controller_rich_cases: usize,
    pub third_party_output_event_cases: usize,
    pub tags: BTreeMap<String, usize>,
    pub third_party_tags: BTreeMap<String, usize>,
}

impl RuntimeProbeCoverageObserved {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut observed = Self {
            total_cases: results.len(),
            ..Self::default()
        };
        for result in results {
            let Some(evidence) = &result.evidence else {
                observed.missing_evidence_cases += 1;
                continue;
            };
            observed.reported_cases += 1;
            for tag in &evidence.tags {
                increment_tag(&mut observed.tags, tag);
            }
            if !evidence.third_party {
                continue;
            }
            observed.third_party_cases += 1;
            if result.passed() {
                observed.third_party_passed_cases += 1;
            }
            for tag in &evidence.tags {
                increment_tag(&mut observed.third_party_tags, tag);
            }
            match evidence.plugin_kind {
                RuntimeProbePluginKind::Effect => observed.third_party_effect_cases += 1,
                RuntimeProbePluginKind::Instrument => observed.third_party_instrument_cases += 1,
                RuntimeProbePluginKind::Hybrid => observed.third_party_hybrid_cases += 1,
                RuntimeProbePluginKind::Unknown => {}
            }
            observed.record_runtime_capabilities(result);
        }
        observed
    }

    fn record_runtime_capabilities(&mut self, result: &RuntimeProbeResult) {
        if !result.passed() {
            return;
        }
        if has_non_silent_output(result) {
            self.third_party_non_silent_cases += 1;
        }
        if has_note_response(result) {
            self.third_party_note_response_cases += 1;
        }
        if has_controller_rich_runtime(result) {
            self.third_party_controller_rich_cases += 1;
        }
        if has_output_events(result) {
            self.third_party_output_event_cases += 1;
        }
    }
}

fn result_is_third_party(result: &RuntimeProbeResult) -> bool {
    result
        .evidence
        .as_ref()
        .is_some_and(|evidence| evidence.third_party)
}

fn has_non_silent_output(result: &RuntimeProbeResult) -> bool {
    process_u64(result, "nonZeroOutputBlocks") > 0
}

fn has_note_response(result: &RuntimeProbeResult) -> bool {
    result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("process"))
        .and_then(|process| process.get("noteTiming"))
        .is_some_and(|timing| {
            timing
                .get("notePresent")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && timing
                    .get("framesFromNoteOnToFirstNonZeroOutput")
                    .and_then(Value::as_i64)
                    .is_some_and(|frames| frames >= 0)
        })
}

fn has_controller_rich_runtime(result: &RuntimeProbeResult) -> bool {
    let Some(controller) = result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("controller"))
    else {
        return false;
    };
    object_u64(controller, "parameters", "count") > 0
        && object_u64(controller, "parameters", "automatable") > 0
        && nested_bool(controller, "componentState", "roundtrip", "success")
        && nested_bool(controller, "controllerState", "roundtrip", "success")
        && nested_bool(controller, "componentHandler", "editProbe", "success")
        && nested_bool(controller, "connectionPoints", "notifyProbe", "success")
}

fn has_output_events(result: &RuntimeProbeResult) -> bool {
    let Some(process) = result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("process"))
    else {
        return false;
    };
    u64_field(process, "outputEvents") > 0
        || u64_field(process, "outputParameterChanges") > 0
        || process
            .get("diagnostics")
            .and_then(|diagnostics| diagnostics.get("outputEvents"))
            .is_some_and(|events| {
                u64_field(events, "normalizedEvents") > 0 || u64_field(events, "advancedEvents") > 0
            })
        || process
            .get("diagnostics")
            .and_then(|diagnostics| diagnostics.get("outputParameterChanges"))
            .is_some_and(|changes| u64_field(changes, "normalizedPoints") > 0)
}

fn process_u64(result: &RuntimeProbeResult, field: &str) -> u64 {
    result
        .probe_report
        .as_ref()
        .and_then(|report| report.get("process"))
        .map(|process| u64_field(process, field))
        .unwrap_or(0)
}

fn object_u64(value: &Value, object: &str, field: &str) -> u64 {
    value
        .get(object)
        .map(|object| u64_field(object, field))
        .unwrap_or(0)
}

fn nested_bool(value: &Value, object: &str, nested: &str, field: &str) -> bool {
    value
        .get(object)
        .and_then(|object| object.get(nested))
        .and_then(|nested| nested.get(field))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn increment_tag(map: &mut BTreeMap<String, usize>, tag: &str) {
    let trimmed = tag.trim();
    if !trimmed.is_empty() {
        *map.entry(trimmed.to_string()).or_default() += 1;
    }
}
