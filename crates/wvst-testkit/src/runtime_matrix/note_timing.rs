use serde::Serialize;
use serde_json::Value;

use super::report::RuntimeProbeResult;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeNoteTimingHealthSummary {
    pub reported_cases: usize,
    pub note_input_cases: usize,
    pub note_response_cases: usize,
    pub missing_note_response_cases: usize,
    pub negative_latency_cases: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_frames: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_frames_case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_micros: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_to_audio_micros_case: Option<String>,
}

impl RuntimeProbeNoteTimingHealthSummary {
    pub(crate) fn from_results(results: &[RuntimeProbeResult]) -> Self {
        let mut summary = Self::default();

        for result in results {
            let Some(note_timing) = result
                .probe_report
                .as_ref()
                .and_then(|report| report.get("process"))
                .and_then(|process| process.get("noteTiming"))
            else {
                continue;
            };

            summary.reported_cases += 1;
            if !bool_field(note_timing, "notePresent") {
                continue;
            }

            summary.note_input_cases += 1;
            let latency_frames =
                optional_i64_field(note_timing, "framesFromNoteOnToFirstNonZeroOutput");
            let latency_micros =
                optional_i64_field(note_timing, "microsFromNoteOnToFirstNonZeroOutput");

            match latency_frames {
                Some(frames) if frames >= 0 => {
                    summary.note_response_cases += 1;
                    summary.observe_max_frames(result, frames as u64);
                    if let Some(micros) = latency_micros
                        && micros >= 0
                    {
                        summary.observe_max_micros(result, micros as u64);
                    }
                }
                Some(_) => {
                    summary.negative_latency_cases += 1;
                    summary.missing_note_response_cases += 1;
                }
                None => {
                    summary.missing_note_response_cases += 1;
                }
            }
        }

        summary
    }

    fn observe_max_frames(&mut self, result: &RuntimeProbeResult, frames: u64) {
        if self.max_note_to_audio_frames.is_none_or(|max| frames > max) {
            self.max_note_to_audio_frames = Some(frames);
            self.max_note_to_audio_frames_case = Some(result.case_name.clone());
        }
    }

    fn observe_max_micros(&mut self, result: &RuntimeProbeResult, micros: u64) {
        if self.max_note_to_audio_micros.is_none_or(|max| micros > max) {
            self.max_note_to_audio_micros = Some(micros);
            self.max_note_to_audio_micros_case = Some(result.case_name.clone());
        }
    }
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn optional_i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}
