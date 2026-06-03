use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION, RuntimeProbeCaseEvidence, RuntimeProbePluginKind,
    audio_bus_summary::RuntimeProbeAudioBusSummary,
    note_timing::RuntimeProbeNoteTimingHealthSummary,
    process_output_summary::RuntimeProbeProcessOutputSummary,
    process_timing_summary::RuntimeProbeProcessTimingSummary,
    runtime_characteristics_summary::RuntimeProbeRuntimeCharacteristicsSummary,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProbeStatus {
    #[default]
    Passed,
    Failed,
    LaunchFailed,
    TimedOut,
    ExpectationFailed,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeResult {
    pub case_name: String,
    pub plugin_path: PathBuf,
    pub class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<RuntimeProbeCaseEvidence>,
    pub status: RuntimeProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_status: Option<RuntimeProbeStatus>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe_report: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expectation_failures: Vec<String>,
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
    pub timed_out: usize,
    pub expectation_failed: usize,
    pub audio_health: RuntimeProbeAudioHealthSummary,
    pub audio_buses: RuntimeProbeAudioBusSummary,
    pub note_timing: RuntimeProbeNoteTimingHealthSummary,
    pub process_output: RuntimeProbeProcessOutputSummary,
    pub process_timing: RuntimeProbeProcessTimingSummary,
    pub runtime_characteristics: RuntimeProbeRuntimeCharacteristicsSummary,
    pub controller_health: RuntimeProbeControllerHealthSummary,
    pub diagnostics: RuntimeProbeDiagnosticsSummary,
    pub evidence: RuntimeProbeEvidenceSummary,
}

impl RuntimeProbeMatrixReport {
    pub(crate) fn new(results: Vec<RuntimeProbeResult>) -> Self {
        let passed = results.iter().filter(|result| result.passed()).count();
        let failed = count_status(&results, RuntimeProbeStatus::Failed);
        let launch_failed = count_status(&results, RuntimeProbeStatus::LaunchFailed);
        let timed_out = count_status(&results, RuntimeProbeStatus::TimedOut);
        let expectation_failed = count_status(&results, RuntimeProbeStatus::ExpectationFailed);
        let audio_health = RuntimeProbeAudioHealthSummary::from_results(&results);
        let audio_buses = RuntimeProbeAudioBusSummary::from_results(&results);
        let note_timing = RuntimeProbeNoteTimingHealthSummary::from_results(&results);
        let process_output = RuntimeProbeProcessOutputSummary::from_results(&results);
        let process_timing = RuntimeProbeProcessTimingSummary::from_results(&results);
        let runtime_characteristics =
            RuntimeProbeRuntimeCharacteristicsSummary::from_results(&results);
        let controller_health = RuntimeProbeControllerHealthSummary::from_results(&results);
        let diagnostics = RuntimeProbeDiagnosticsSummary::from_results(&results);
        let evidence = RuntimeProbeEvidenceSummary::from_results(&results);

        Self {
            schema_version: RUNTIME_PROBE_MATRIX_REPORT_SCHEMA_VERSION,
            results,
            passed,
            failed,
            launch_failed,
            timed_out,
            expectation_failed,
            audio_health,
            audio_buses,
            note_timing,
            process_output,
            process_timing,
            runtime_characteristics,
            controller_health,
            diagnostics,
            evidence,
        }
    }

    pub const fn all_passed(&self) -> bool {
        self.failed == 0
            && self.launch_failed == 0
            && self.timed_out == 0
            && self.expectation_failed == 0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeEvidenceSummary {
    pub reported_cases: usize,
    pub missing_evidence_cases: usize,
    pub third_party_cases: usize,
    pub effect_cases: usize,
    pub instrument_cases: usize,
    pub hybrid_cases: usize,
    pub unknown_kind_cases: usize,
    pub missing_vendor_cases: usize,
    pub missing_plugin_name_cases: usize,
    pub vendors: BTreeMap<String, usize>,
    pub tags: BTreeMap<String, usize>,
}

impl RuntimeProbeEvidenceSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(evidence) = &result.evidence else {
                summary.missing_evidence_cases += 1;
                continue;
            };

            summary.reported_cases += 1;
            if evidence.third_party {
                summary.third_party_cases += 1;
            }
            match evidence.plugin_kind {
                RuntimeProbePluginKind::Effect => summary.effect_cases += 1,
                RuntimeProbePluginKind::Instrument => summary.instrument_cases += 1,
                RuntimeProbePluginKind::Hybrid => summary.hybrid_cases += 1,
                RuntimeProbePluginKind::Unknown => summary.unknown_kind_cases += 1,
            }

            if let Some(vendor) = non_empty(&evidence.vendor) {
                increment(&mut summary.vendors, vendor.to_string());
            } else {
                summary.missing_vendor_cases += 1;
            }
            if non_empty(&evidence.plugin_name).is_none() {
                summary.missing_plugin_name_cases += 1;
            }
            for tag in &evidence.tags {
                let tag = tag.trim();
                if !tag.is_empty() {
                    increment(&mut summary.tags, tag.to_string());
                }
            }
        }

        summary
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeControllerHealthSummary {
    pub reported_cases: usize,
    pub parameter_cases: usize,
    pub total_parameters: u64,
    pub total_automatable_parameters: u64,
    pub max_parameter_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_parameter_count_case: Option<String>,
    pub max_automatable_parameters: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_automatable_parameters_case: Option<String>,
    pub component_state_cases: usize,
    pub component_state_roundtrip_cases: usize,
    pub total_component_state_bytes: u64,
    pub max_component_state_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_component_state_bytes_case: Option<String>,
    pub controller_state_cases: usize,
    pub controller_state_roundtrip_cases: usize,
    pub controller_component_state_sync_attempted_cases: usize,
    pub controller_component_state_sync_cases: usize,
    pub controller_component_state_sync_failed_cases: usize,
    pub total_controller_component_state_sync_bytes: u64,
    pub max_controller_component_state_sync_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_controller_component_state_sync_bytes_case: Option<String>,
    pub total_controller_state_bytes: u64,
    pub max_controller_state_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_controller_state_bytes_case: Option<String>,
    pub unit_info_cases: usize,
    pub total_units: u64,
    pub total_program_lists: u64,
    pub total_programs: u64,
    pub max_total_programs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_total_programs_case: Option<String>,
    pub program_list_data_cases: usize,
    pub total_program_list_data_checked: u64,
    pub total_program_list_data_supported: u64,
    pub total_program_list_data_unsupported: u64,
    pub unit_data_cases: usize,
    pub total_unit_data_checked: u64,
    pub total_unit_data_supported: u64,
    pub total_unit_data_unsupported: u64,
    pub component_handler_cases: usize,
    pub component_handler_edit_probe_cases: usize,
    pub connection_point_cases: usize,
    pub connection_notify_probe_cases: usize,
    pub component_connection_notify_probe_cases: usize,
    pub controller_connection_notify_probe_cases: usize,
    pub total_component_handler_events: u64,
    pub total_component_handler_edit_probe_event_delta: u64,
}

impl RuntimeProbeControllerHealthSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(controller) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("controller"))
            else {
                continue;
            };

            summary.reported_cases += 1;

            let parameter_count = controller
                .get("parameters")
                .map(|parameters| u64_field(parameters, "count"))
                .unwrap_or(0);
            let automatable = controller
                .get("parameters")
                .map(|parameters| u64_field(parameters, "automatable"))
                .unwrap_or(0);

            if parameter_count > 0 {
                summary.parameter_cases += 1;
            }
            summary.total_parameters = summary.total_parameters.saturating_add(parameter_count);
            summary.total_automatable_parameters = summary
                .total_automatable_parameters
                .saturating_add(automatable);
            if parameter_count > summary.max_parameter_count {
                summary.max_parameter_count = parameter_count;
                summary.max_parameter_count_case = Some(result.case_name.clone());
            }
            if automatable > summary.max_automatable_parameters {
                summary.max_automatable_parameters = automatable;
                summary.max_automatable_parameters_case = Some(result.case_name.clone());
            }

            if availability_field(controller, "componentState") {
                summary.component_state_cases += 1;
                if let Some(state) = controller.get("componentState") {
                    if state
                        .get("roundtrip")
                        .and_then(|roundtrip| roundtrip.get("success"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        summary.component_state_roundtrip_cases += 1;
                    }
                    let bytes = u64_field(state, "bytes");
                    summary.total_component_state_bytes =
                        summary.total_component_state_bytes.saturating_add(bytes);
                    if bytes > summary.max_component_state_bytes {
                        summary.max_component_state_bytes = bytes;
                        summary.max_component_state_bytes_case = Some(result.case_name.clone());
                    }
                }
            }
            if availability_field(controller, "controllerState") {
                summary.controller_state_cases += 1;
                if let Some(state) = controller.get("controllerState") {
                    if state
                        .get("roundtrip")
                        .and_then(|roundtrip| roundtrip.get("success"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        summary.controller_state_roundtrip_cases += 1;
                    }
                    let bytes = u64_field(state, "bytes");
                    summary.total_controller_state_bytes =
                        summary.total_controller_state_bytes.saturating_add(bytes);
                    if bytes > summary.max_controller_state_bytes {
                        summary.max_controller_state_bytes = bytes;
                        summary.max_controller_state_bytes_case = Some(result.case_name.clone());
                    }
                }
            }
            if let Some(sync) = controller.get("controllerComponentStateSync") {
                if sync
                    .get("attempted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    summary.controller_component_state_sync_attempted_cases += 1;
                }
                if sync
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    summary.controller_component_state_sync_cases += 1;
                } else if sync.get("error").is_some() {
                    summary.controller_component_state_sync_failed_cases += 1;
                }
                let bytes = u64_field(sync, "componentStateBytes");
                summary.total_controller_component_state_sync_bytes = summary
                    .total_controller_component_state_sync_bytes
                    .saturating_add(bytes);
                if bytes > summary.max_controller_component_state_sync_bytes {
                    summary.max_controller_component_state_sync_bytes = bytes;
                    summary.max_controller_component_state_sync_bytes_case =
                        Some(result.case_name.clone());
                }
            }
            if availability_field(controller, "units") {
                summary.unit_info_cases += 1;
                if let Some(units) = controller.get("units") {
                    let unit_count = u64_field(units, "unitCount");
                    let program_list_count = u64_field(units, "programListCount");
                    let total_programs = u64_field(units, "totalPrograms");
                    summary.total_units = summary.total_units.saturating_add(unit_count);
                    summary.total_program_lists = summary
                        .total_program_lists
                        .saturating_add(program_list_count);
                    summary.total_programs = summary.total_programs.saturating_add(total_programs);
                    if total_programs > summary.max_total_programs {
                        summary.max_total_programs = total_programs;
                        summary.max_total_programs_case = Some(result.case_name.clone());
                    }
                }
            }
            if availability_field(controller, "programListData") {
                summary.program_list_data_cases += 1;
                if let Some(data) = controller.get("programListData") {
                    summary.total_program_list_data_checked = summary
                        .total_program_list_data_checked
                        .saturating_add(u64_field(data, "checked"));
                    summary.total_program_list_data_supported = summary
                        .total_program_list_data_supported
                        .saturating_add(u64_field(data, "supported"));
                    summary.total_program_list_data_unsupported = summary
                        .total_program_list_data_unsupported
                        .saturating_add(u64_field(data, "unsupported"));
                }
            }
            if availability_field(controller, "unitData") {
                summary.unit_data_cases += 1;
                if let Some(data) = controller.get("unitData") {
                    summary.total_unit_data_checked = summary
                        .total_unit_data_checked
                        .saturating_add(u64_field(data, "checked"));
                    summary.total_unit_data_supported = summary
                        .total_unit_data_supported
                        .saturating_add(u64_field(data, "supported"));
                    summary.total_unit_data_unsupported = summary
                        .total_unit_data_unsupported
                        .saturating_add(u64_field(data, "unsupported"));
                }
            }
            if availability_field(controller, "componentHandler") {
                summary.component_handler_cases += 1;
                if let Some(handler) = controller.get("componentHandler") {
                    summary.total_component_handler_events = summary
                        .total_component_handler_events
                        .saturating_add(u64_field(handler, "totalEvents"));
                    if let Some(edit_probe) = handler.get("editProbe")
                        && edit_probe
                            .get("success")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                    {
                        summary.component_handler_edit_probe_cases += 1;
                        summary.total_component_handler_edit_probe_event_delta = summary
                            .total_component_handler_edit_probe_event_delta
                            .saturating_add(u64_field(edit_probe, "eventDelta"));
                    }
                }
            }
            if controller
                .get("connectionPoints")
                .and_then(|connection_points| connection_points.get("connected"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                summary.connection_point_cases += 1;
            }
            if let Some(notify_probe) = controller
                .get("connectionPoints")
                .and_then(|connection_points| connection_points.get("notifyProbe"))
            {
                if notify_probe
                    .get("component")
                    .and_then(|component| component.get("success"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    summary.component_connection_notify_probe_cases += 1;
                }
                if notify_probe
                    .get("controller")
                    .and_then(|controller| controller.get("success"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    summary.controller_connection_notify_probe_cases += 1;
                }
                if notify_probe
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    summary.connection_notify_probe_cases += 1;
                }
            }
        }

        summary
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeDiagnosticsSummary {
    pub compatibility_categories: BTreeMap<String, usize>,
    pub classification_categories: BTreeMap<String, usize>,
    pub failure_kinds: BTreeMap<String, usize>,
}

impl RuntimeProbeDiagnosticsSummary {
    fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(report) = &result.probe_report else {
                continue;
            };
            for category in collect_diagnostic_categories(report, "compatibility") {
                increment(&mut summary.compatibility_categories, category);
            }
            for category in collect_diagnostic_categories(report, "classification") {
                increment(&mut summary.classification_categories, category);
            }
            if let Some(kind) = report
                .get("data")
                .and_then(|data| data.get("kind"))
                .and_then(Value::as_str)
            {
                increment(&mut summary.failure_kinds, kind.to_string());
            }
        }

        summary
    }
}

pub(crate) fn collect_diagnostic_categories(
    value: &Value,
    diagnostic_key: &'static str,
) -> Vec<String> {
    let mut categories = Vec::new();
    collect_diagnostic_categories_into(value, diagnostic_key, &mut categories);
    categories
}

fn collect_diagnostic_categories_into(
    value: &Value,
    diagnostic_key: &'static str,
    categories: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            if let Some(category) = object
                .get(diagnostic_key)
                .and_then(|diagnostic| diagnostic.get("category"))
                .and_then(Value::as_str)
            {
                categories.push(category.to_string());
            }
            for value in object.values() {
                collect_diagnostic_categories_into(value, diagnostic_key, categories);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_diagnostic_categories_into(value, diagnostic_key, categories);
            }
        }
        _ => {}
    }
}

fn count_status(results: &[RuntimeProbeResult], status: RuntimeProbeStatus) -> usize {
    results
        .iter()
        .filter(|result| result.status == status)
        .count()
}

fn increment(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

pub(crate) fn f64_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

pub(crate) fn availability_field(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(|field| field.get("available"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
