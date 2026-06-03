use std::ffi::OsString;

use serde::Serialize;
use wvst_bridge_server::{BridgeConfig, HostWorkerClient};

const DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeServerDiagnosticSnapshot {
    pub schema_version: u16,
    pub bridge_version: &'static str,
    pub platform: PlatformDiagnostic,
    pub config: BridgeConfigDiagnostic,
    pub host_worker: HostWorkerDiagnostic,
    pub environment: Vec<EnvironmentVariableDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDiagnostic {
    pub os: &'static str,
    pub arch: &'static str,
    pub exe_suffix: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeConfigDiagnostic {
    pub bind_addr: String,
    pub token_required: bool,
    pub allowed_origins: Vec<String>,
    pub worker_auto_restart: bool,
    pub max_worker_instances: usize,
    pub worker_quarantine_failure_threshold: u32,
    pub max_control_message_bytes: usize,
    pub worker_memory_limit_bytes: Option<u64>,
    pub worker_cpu_time_limit_seconds: Option<u64>,
    pub worker_linux_cgroup_parent: Option<String>,
    pub worker_linux_cgroup_memory_max_bytes: Option<u64>,
    pub worker_linux_cgroup_cpu_max_micros: Option<WorkerLinuxCgroupCpuMaxDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerLinuxCgroupCpuMaxDiagnostic {
    pub quota_micros: u64,
    pub period_micros: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostWorkerDiagnostic {
    pub executable: String,
    pub exists: bool,
    pub is_file: bool,
    pub timeout_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariableDiagnostic {
    pub name: &'static str,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub redacted: bool,
}

pub fn collect_bridge_diagnostics(
    config: &BridgeConfig,
    host_worker: &HostWorkerClient,
) -> BridgeServerDiagnosticSnapshot {
    BridgeServerDiagnosticSnapshot {
        schema_version: DIAGNOSTIC_SCHEMA_VERSION,
        bridge_version: env!("CARGO_PKG_VERSION"),
        platform: PlatformDiagnostic {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            exe_suffix: std::env::consts::EXE_SUFFIX,
        },
        config: config_diagnostic(config),
        host_worker: host_worker_diagnostic(host_worker),
        environment: environment_diagnostics(),
    }
}

fn config_diagnostic(config: &BridgeConfig) -> BridgeConfigDiagnostic {
    BridgeConfigDiagnostic {
        bind_addr: config.bind_addr().to_string(),
        token_required: config.token_required(),
        allowed_origins: config.allowed_origins().to_vec(),
        worker_auto_restart: config.worker_auto_restart_enabled(),
        max_worker_instances: config.max_worker_instances(),
        worker_quarantine_failure_threshold: config.worker_quarantine_failure_threshold(),
        max_control_message_bytes: config.max_control_message_bytes(),
        worker_memory_limit_bytes: config.worker_memory_limit_bytes(),
        worker_cpu_time_limit_seconds: config.worker_cpu_time_limit_seconds(),
        worker_linux_cgroup_parent: config
            .worker_linux_cgroup_parent()
            .map(|path| path.display().to_string()),
        worker_linux_cgroup_memory_max_bytes: config.worker_linux_cgroup_memory_max_bytes(),
        worker_linux_cgroup_cpu_max_micros: config.worker_linux_cgroup_cpu_max_micros().map(
            |(quota_micros, period_micros)| WorkerLinuxCgroupCpuMaxDiagnostic {
                quota_micros,
                period_micros,
            },
        ),
    }
}

fn host_worker_diagnostic(host_worker: &HostWorkerClient) -> HostWorkerDiagnostic {
    let executable = host_worker.executable_path();
    HostWorkerDiagnostic {
        executable: executable.display().to_string(),
        exists: executable.exists(),
        is_file: executable.is_file(),
        timeout_ms: host_worker.timeout().as_millis(),
    }
}

fn environment_diagnostics() -> Vec<EnvironmentVariableDiagnostic> {
    [
        ("WVST_BIND_ADDR", false),
        ("WVST_TOKEN", true),
        ("WVST_ALLOWED_ORIGINS", false),
        ("WVST_ALLOW_LOOPBACK_ORIGINS", false),
        ("WVST_HOST_WORKER", false),
        ("WVST_WORKER_AUTO_RESTART", false),
        ("WVST_MAX_WORKER_INSTANCES", false),
        ("WVST_WORKER_QUARANTINE_FAILURES", false),
        ("WVST_MAX_CONTROL_MESSAGE_BYTES", false),
        ("WVST_WORKER_MEMORY_LIMIT_BYTES", false),
        ("WVST_WORKER_CPU_TIME_LIMIT_SECONDS", false),
        ("WVST_WORKER_LINUX_CGROUP_PARENT", false),
        ("WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES", false),
        ("WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS", false),
        ("WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS", false),
    ]
    .into_iter()
    .map(|(name, redacted)| env_var_diagnostic_from_value(name, redacted, std::env::var_os(name)))
    .collect()
}

fn env_var_diagnostic_from_value(
    name: &'static str,
    redacted: bool,
    value: Option<OsString>,
) -> EnvironmentVariableDiagnostic {
    let present = value.is_some();
    EnvironmentVariableDiagnostic {
        name,
        present,
        value: if redacted {
            None
        } else {
            value.map(|value| value.to_string_lossy().into_owned())
        },
        redacted,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;

    #[test]
    fn collects_config_and_worker_diagnostics() {
        let config = BridgeConfig::development("127.0.0.1:0".parse().expect("addr"))
            .with_worker_quarantine_failure_threshold(5)
            .with_worker_memory_limit_bytes(64 * 1024 * 1024);
        let worker = HostWorkerClient::new(
            PathBuf::from("/tmp/wvst-host-worker"),
            Duration::from_millis(250),
        );

        let diagnostics = collect_bridge_diagnostics(&config, &worker);

        assert_eq!(diagnostics.schema_version, 1);
        assert_eq!(diagnostics.config.bind_addr, "127.0.0.1:0");
        assert_eq!(diagnostics.config.worker_quarantine_failure_threshold, 5);
        assert_eq!(
            diagnostics.config.worker_memory_limit_bytes,
            Some(64 * 1024 * 1024)
        );
        assert_eq!(diagnostics.host_worker.timeout_ms, 250);
        assert_eq!(diagnostics.host_worker.executable, "/tmp/wvst-host-worker");
    }

    #[test]
    fn redacts_sensitive_environment_values() {
        let token = env_var_diagnostic_from_value("WVST_TOKEN", true, Some("secret".into()));
        let worker =
            env_var_diagnostic_from_value("WVST_HOST_WORKER", false, Some("/tmp/worker".into()));

        assert!(token.present);
        assert!(token.redacted);
        assert_eq!(token.value, None);
        assert!(worker.present);
        assert!(!worker.redacted);
        assert_eq!(worker.value.as_deref(), Some("/tmp/worker"));
    }
}
