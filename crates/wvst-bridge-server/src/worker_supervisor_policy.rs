use std::time::{Duration, Instant};

use super::{WorkerQuarantineStatus, WorkerSupervisor, WorkerSupervisorError};

const FAILURE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(super) struct FailureRecord {
    count: u32,
    last_failure: Instant,
    release_at: Option<Instant>,
}

impl WorkerSupervisor {
    pub async fn quarantine_failures(&self, plugin_id: &str) -> Option<u32> {
        self.quarantine_status(plugin_id)
            .await
            .map(|status| status.failures)
    }

    pub async fn quarantine_status(&self, plugin_id: &str) -> Option<WorkerQuarantineStatus> {
        let failures = self.failures.lock().await;
        let record = failures.get(plugin_id)?;
        Some(WorkerQuarantineStatus {
            failures: record.count,
            release_after_ms: record
                .release_at?
                .saturating_duration_since(Instant::now())
                .as_millis(),
        })
    }

    pub async fn release_expired_quarantine(&self, plugin_id: &str) -> Option<u32> {
        let mut failures = self.failures.lock().await;
        let record = failures.get(plugin_id)?;
        if Instant::now() < record.release_at? {
            return None;
        }
        failures.remove(plugin_id).map(|record| record.count)
    }

    pub(super) async fn reject_if_quarantined(
        &self,
        plugin_id: &str,
    ) -> Result<(), WorkerSupervisorError> {
        let mut failures = self.failures.lock().await;
        if let Some(record) = failures.get(plugin_id) {
            if let Some(release_at) = record.release_at {
                let now = Instant::now();
                if now < release_at {
                    return Err(WorkerSupervisorError::Quarantined {
                        plugin_id: plugin_id.to_string(),
                        failures: record.count,
                        release_after_ms: release_at.saturating_duration_since(now).as_millis(),
                    });
                }
                failures.remove(plugin_id);
            }
        }
        Ok(())
    }

    pub(super) async fn record_failure(&self, plugin_id: &str) -> Option<u32> {
        // One lock for counts and quarantine avoids inverted lock ordering during release.
        let mut failures = self.failures.lock().await;
        let now = Instant::now();
        let record = failures
            .entry(plugin_id.to_string())
            .or_insert(FailureRecord {
                count: 0,
                last_failure: now,
                release_at: None,
            });
        if record.release_at.is_some_and(|release_at| now < release_at) {
            return Some(record.count);
        }
        if now.duration_since(record.last_failure) >= FAILURE_WINDOW {
            record.count = 0;
        }
        record.count = record.count.saturating_add(1);
        record.last_failure = now;
        if record.count >= self.quarantine_failure_threshold {
            record.release_at = now.checked_add(self.quarantine_duration);
            return Some(record.count);
        }
        None
    }

    pub(super) fn request_timeout(&self, method: &str) -> Duration {
        match method {
            "instance.create"
            | "instance.setState"
            | "instance.setUnitProgramData"
            | "instance.programData.set"
            | "instance.unitData.set" => self.load_timeout,
            _ => self.timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker_supervisor::WorkerSupervisorOptions;

    #[test]
    fn state_loading_has_an_independent_budget() {
        let supervisor = WorkerSupervisor::with_options(
            WorkerSupervisorOptions::new("worker".into())
                .with_timeout(Duration::from_millis(50))
                .with_load_timeout(Duration::from_secs(120)),
        );
        for method in [
            "instance.create",
            "instance.setState",
            "instance.programData.set",
            "instance.unitData.set",
        ] {
            assert_eq!(supervisor.request_timeout(method), Duration::from_secs(120));
        }
        assert_eq!(
            supervisor.request_timeout("worker.metrics"),
            Duration::from_millis(50)
        );
        assert_eq!(
            supervisor.request_timeout("audio.processFrame"),
            Duration::from_millis(50)
        );
    }

    #[tokio::test]
    async fn isolated_failures_do_not_accumulate_forever() {
        let supervisor = WorkerSupervisor::new("worker".into());
        supervisor.record_failure("plugin").await;
        supervisor
            .failures
            .lock()
            .await
            .get_mut("plugin")
            .unwrap()
            .last_failure -= FAILURE_WINDOW;
        assert_eq!(supervisor.record_failure("plugin").await, None);
        assert_eq!(supervisor.failures.lock().await["plugin"].count, 1);
    }
}
