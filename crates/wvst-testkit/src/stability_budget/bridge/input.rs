use serde::Deserialize;
use serde_json::Value;

use super::BridgeStabilityMetrics;

impl BridgeStabilityMetrics {
    pub fn from_json_str(text: &str) -> Result<Self, serde_json::Error> {
        let value = serde_json::from_str::<Value>(text)?;
        Self::from_json_value(value)
    }

    pub fn from_json_value(value: Value) -> Result<Self, serde_json::Error> {
        let root = value.get("result").unwrap_or(&value);
        let metrics_value = root
            .get("bridgeMetrics")
            .or_else(|| root.get("metrics"))
            .unwrap_or(root);
        let mut metrics = serde_json::from_value::<Self>(metrics_value.clone())?;
        if let Some(status_value) = pump_status_value(root) {
            let status = serde_json::from_value::<BridgePumpStatusMetrics>(status_value.clone())?;
            metrics.apply_pump_status(status);
        }
        Ok(metrics)
    }

    fn apply_pump_status(&mut self, status: BridgePumpStatusMetrics) {
        apply_if_zero(
            &mut self.shared_memory_pump_preflight_skips,
            status.preflight_skips,
        );
        apply_if_zero(&mut self.shared_memory_pump_overruns, status.overruns);
        apply_if_zero(
            &mut self.shared_memory_pump_input_underruns,
            status.input_underruns,
        );
        apply_if_zero(
            &mut self.shared_memory_pump_output_backpressure,
            status.output_backpressure,
        );
        apply_if_zero(
            &mut self.shared_memory_pump_worker_errors,
            status.worker_errors,
        );
        apply_optional_if_missing(
            &mut self.shared_memory_pump_last_process_micros,
            status.last_process_micros,
        );
        apply_optional_if_missing(
            &mut self.shared_memory_pump_max_process_micros,
            status.max_process_micros,
        );
        apply_optional_if_missing(
            &mut self.shared_memory_pump_last_overrun_micros,
            status.last_overrun_micros,
        );
        if let Some(events) = status.events {
            apply_if_zero(
                &mut self.shared_memory_pump_events_enqueued,
                events.enqueued_events,
            );
            apply_if_zero(
                &mut self.shared_memory_pump_events_drained,
                events.drained_events,
            );
            apply_if_zero(&mut self.shared_memory_pump_events_late, events.late_events);
            apply_if_zero(
                &mut self.shared_memory_pump_events_dropped,
                events.dropped_events,
            );
            apply_if_zero(
                &mut self.shared_memory_pump_events_cleared,
                events.cleared_events,
            );
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgePumpStatusMetrics {
    #[serde(default)]
    preflight_skips: u64,
    #[serde(default)]
    overruns: u64,
    #[serde(default)]
    input_underruns: u64,
    #[serde(default)]
    output_backpressure: u64,
    #[serde(default)]
    worker_errors: u64,
    #[serde(default)]
    last_process_micros: Option<u64>,
    #[serde(default)]
    max_process_micros: Option<u64>,
    #[serde(default)]
    last_overrun_micros: Option<u64>,
    #[serde(default)]
    events: Option<BridgePumpEventMetrics>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgePumpEventMetrics {
    #[serde(default)]
    enqueued_events: u64,
    #[serde(default)]
    drained_events: u64,
    #[serde(default)]
    late_events: u64,
    #[serde(default)]
    dropped_events: u64,
    #[serde(default)]
    cleared_events: u64,
}

fn pump_status_value(root: &Value) -> Option<&Value> {
    [
        &["status"][..],
        &["pumpStatus"][..],
        &["sharedMemoryPumpStatus"][..],
        &["sharedMemoryPump", "status"][..],
        &["dataPlane", "sharedMemoryPump", "status"][..],
        &["sharedMemoryPump"][..],
    ]
    .iter()
    .find_map(|path| value_at(root, path).filter(|value| !value.is_null()))
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |current, key| current.get(key))
}

fn apply_if_zero(target: &mut u64, value: u64) {
    if *target == 0 {
        *target = value;
    }
}

fn apply_optional_if_missing(target: &mut Option<u64>, value: Option<u64>) {
    if target.is_none() {
        *target = value;
    }
}
