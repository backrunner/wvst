use serde_json::{Value, json};
use wvst_vst3_host::HostError;

use crate::vst3_error_data::vst3_error_data;

pub(crate) const RUNTIME_PROBE_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeProbeFailure {
    pub(crate) message: String,
    data: Option<Value>,
    cleanup: Option<Box<RuntimeProbeFailure>>,
}

impl RuntimeProbeFailure {
    pub(crate) fn plain(kind: &'static str, stage: &'static str, message: String) -> Self {
        Self {
            data: Some(json!({
                "kind": kind,
                "stage": stage,
                "message": message,
            })),
            message,
            cleanup: None,
        }
    }

    fn vst3(kind: &'static str, stage: &'static str, error: HostError) -> Self {
        Self {
            message: error.to_string(),
            data: Some(vst3_error_data(kind, stage, &error)),
            cleanup: None,
        }
    }

    pub(crate) fn with_cleanup(mut self, cleanup: RuntimeProbeFailure) -> Self {
        self.cleanup = Some(Box::new(cleanup));
        self
    }

    pub(crate) fn to_report(&self) -> Value {
        json!({
            "schemaVersion": RUNTIME_PROBE_REPORT_SCHEMA_VERSION,
            "ok": false,
            "message": self.message,
            "data": self.data,
            "cleanup": self.cleanup.as_ref().map(|cleanup| {
                json!({
                    "message": cleanup.message,
                    "data": cleanup.data,
                })
            }),
        })
    }
}

pub(crate) fn vst3_init<T>(
    stage: &'static str,
    result: Result<T, HostError>,
) -> Result<T, RuntimeProbeFailure> {
    result.map_err(|error| RuntimeProbeFailure::vst3("vst3-runtime-init", stage, error))
}

pub(crate) fn vst3_process<T>(
    stage: &'static str,
    result: Result<T, HostError>,
) -> Result<T, RuntimeProbeFailure> {
    result.map_err(|error| RuntimeProbeFailure::vst3("vst3-runtime-process", stage, error))
}

pub(crate) fn vst3_control<T>(
    stage: &'static str,
    result: Result<T, HostError>,
) -> Result<T, RuntimeProbeFailure> {
    result.map_err(|error| RuntimeProbeFailure::vst3("vst3-runtime-control", stage, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_plain_failure_report() {
        let failure = RuntimeProbeFailure::plain(
            "runtime-probe-options",
            "options.parse",
            "missing path".to_string(),
        );
        let report = failure.to_report();

        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["ok"], false);
        assert_eq!(report["data"]["kind"], "runtime-probe-options");
        assert_eq!(report["data"]["stage"], "options.parse");
    }

    #[test]
    fn maps_vst3_failure_report() {
        let failure = vst3_init::<()>("processing.config", Err(HostError::InvalidSampleRate(0)))
            .expect_err("invalid config");
        let report = failure.to_report();

        assert_eq!(report["data"]["kind"], "vst3-runtime-init");
        assert_eq!(report["data"]["stage"], "processing.config");
        assert_eq!(report["data"]["hostError"], "invalid-sample-rate");
        assert_eq!(
            report["data"]["compatibility"]["category"],
            "processing-configuration"
        );
    }
}
