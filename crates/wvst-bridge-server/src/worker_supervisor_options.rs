use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use wvst_process_supervision::WorkerResourceLimits;

use super::{
    DEFAULT_IPC_TIMEOUT, DEFAULT_MAX_WORKER_INSTANCES, DEFAULT_QUARANTINE_DURATION,
    DEFAULT_QUARANTINE_FAILURE_THRESHOLD, WorkerSupervisorOptions,
};
use crate::metrics::BridgeMetrics;

impl WorkerSupervisorOptions {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            executable,
            timeout: DEFAULT_IPC_TIMEOUT,
            quarantine_duration: DEFAULT_QUARANTINE_DURATION,
            quarantine_failure_threshold: DEFAULT_QUARANTINE_FAILURE_THRESHOLD,
            use_audio_ipc: true,
            max_instances: DEFAULT_MAX_WORKER_INSTANCES,
            resource_limits: WorkerResourceLimits::none(),
            metrics: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_quarantine_duration(mut self, duration: Duration) -> Self {
        self.quarantine_duration = duration;
        self
    }

    pub fn with_quarantine_failure_threshold(mut self, failures: u32) -> Self {
        self.quarantine_failure_threshold = failures.max(1);
        self
    }

    pub fn with_audio_ipc(mut self, enabled: bool) -> Self {
        self.use_audio_ipc = enabled;
        self
    }

    pub fn with_max_instances(mut self, max_instances: usize) -> Self {
        self.max_instances = max_instances.max(1);
        self
    }

    pub fn with_resource_limits(mut self, limits: WorkerResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<BridgeMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}
