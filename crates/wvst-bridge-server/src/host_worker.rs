use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error_classification::{ErrorClassification, attach_error_classification};

const HOST_WORKER_ENV: &str = "WVST_HOST_WORKER";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_STDERR_CHARS: usize = 4096;

#[derive(Debug, Clone)]
pub struct HostWorkerClient {
    executable: PathBuf,
    timeout: Duration,
    load_timeout: Duration,
}

#[derive(Debug)]
pub enum HostWorkerError {
    Spawn {
        executable: PathBuf,
        message: String,
    },
    Timeout {
        timeout_ms: u128,
    },
    Exit {
        status: String,
        stderr: String,
    },
    InvalidJson {
        message: String,
        stdout: String,
    },
}

impl HostWorkerClient {
    pub fn new(executable: PathBuf, timeout: Duration) -> Self {
        Self {
            executable,
            timeout,
            load_timeout: timeout,
        }
    }

    pub fn with_load_timeout(mut self, timeout: Duration) -> Self {
        self.load_timeout = timeout;
        self
    }

    pub fn from_env() -> Self {
        let executable = std::env::var_os(HOST_WORKER_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(default_worker_executable);

        Self::new(executable, DEFAULT_TIMEOUT).with_load_timeout(Duration::from_secs(120))
    }

    pub async fn factory_info(
        &self,
        plugin_path: impl AsRef<Path>,
    ) -> Result<Value, HostWorkerError> {
        self.run_json("factory-info", plugin_path.as_ref()).await
    }

    pub fn executable_path(&self) -> &Path {
        &self.executable
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    async fn run_json(&self, command: &str, plugin_path: &Path) -> Result<Value, HostWorkerError> {
        let mut child = Command::new(&self.executable);
        child
            .arg(command)
            .arg(plugin_path)
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let output = match timeout(self.load_timeout, child.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return Err(HostWorkerError::Spawn {
                    executable: self.executable.clone(),
                    message: error.to_string(),
                });
            }
            Err(_) => {
                return Err(HostWorkerError::Timeout {
                    timeout_ms: self.load_timeout.as_millis(),
                });
            }
        };

        if !output.status.success() {
            return Err(HostWorkerError::Exit {
                status: output.status.to_string(),
                stderr: truncate_text(&output.stderr),
            });
        }

        serde_json::from_slice(&output.stdout).map_err(|error| HostWorkerError::InvalidJson {
            message: error.to_string(),
            stdout: truncate_text(&output.stdout),
        })
    }
}

impl HostWorkerError {
    pub fn rpc_code(&self) -> i64 {
        match self {
            Self::Spawn { .. } => 5030,
            Self::Timeout { .. } => 5031,
            Self::Exit { .. } => 5032,
            Self::InvalidJson { .. } => 5033,
        }
    }

    pub fn rpc_message(&self) -> String {
        match self {
            Self::Spawn {
                executable,
                message,
            } => format!(
                "failed to start host worker {}: {message}",
                executable.display()
            ),
            Self::Timeout { timeout_ms } => {
                format!("host worker timed out after {timeout_ms}ms")
            }
            Self::Exit { status, stderr } => {
                if stderr.is_empty() {
                    format!("host worker exited with {status}")
                } else {
                    format!("host worker exited with {status}: {stderr}")
                }
            }
            Self::InvalidJson { message, .. } => {
                format!("host worker returned invalid JSON: {message}")
            }
        }
    }

    pub fn rpc_data(&self) -> Value {
        let data = match self {
            Self::Spawn {
                executable,
                message,
            } => json!({
                "kind": "spawn",
                "executable": executable.display().to_string(),
                "message": message,
            }),
            Self::Timeout { timeout_ms } => json!({
                "kind": "timeout",
                "timeoutMs": timeout_ms,
            }),
            Self::Exit { status, stderr } => json!({
                "kind": "exit",
                "status": status,
                "stderr": stderr,
            }),
            Self::InvalidJson { message, stdout } => json!({
                "kind": "invalid-json",
                "message": message,
                "stdout": stdout,
            }),
        };

        attach_error_classification(data, host_worker_classification(self))
    }

    #[cfg(test)]
    fn new_for_test_exited(stderr: impl Into<String>) -> Self {
        Self::Exit {
            status: "exit status: 2".to_string(),
            stderr: stderr.into(),
        }
    }
}

const fn host_worker_classification(error: &HostWorkerError) -> ErrorClassification {
    match error {
        HostWorkerError::Spawn { .. } => ErrorClassification::new(
            "host-worker-launch",
            "verify WVST_HOST_WORKER, executable permissions, architecture, and local installation layout",
        ),
        HostWorkerError::Timeout { .. } => ErrorClassification::new(
            "host-worker-timeout",
            "the one-shot host worker command did not complete before the timeout; inspect plugin scan/probe latency and worker stderr",
        ),
        HostWorkerError::Exit { .. } => ErrorClassification::new(
            "host-worker-exit",
            "the one-shot host worker command exited unsuccessfully; inspect stderr for plugin loading or runtime details",
        ),
        HostWorkerError::InvalidJson { .. } => ErrorClassification::new(
            "host-worker-output",
            "the one-shot host worker returned malformed JSON; verify worker version compatibility and stdout contamination",
        ),
    }
}

fn default_worker_executable() -> PathBuf {
    let binary_name = format!("wvst-host-worker{}", std::env::consts::EXE_SUFFIX);

    match std::env::current_exe() {
        Ok(current) => current
            .parent()
            .map(|directory| directory.join(&binary_name))
            .unwrap_or_else(|| PathBuf::from(binary_name)),
        Err(_) => PathBuf::from(binary_name),
    }
}

fn truncate_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();

    if trimmed.chars().count() <= MAX_STDERR_CHARS {
        return trimmed.to_string();
    }

    let mut truncated = trimmed.chars().take(MAX_STDERR_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
impl HostWorkerClient {
    pub fn new_for_test(executable: PathBuf, timeout: Duration) -> Self {
        Self::new(executable, timeout)
    }
}

#[cfg(test)]
mod tests {
    #![cfg_attr(not(unix), allow(dead_code))]

    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn returns_json_from_worker_stdout() {
        use std::os::unix::fs::PermissionsExt;

        let directory = unique_temp_dir();
        std::fs::create_dir_all(&directory).expect("temp dir");
        let worker = directory.join("worker.sh");
        std::fs::write(
            &worker,
            "#!/bin/sh\nprintf '{\"vendor\":\"WVST\",\"url\":null,\"email\":null,\"flags\":0,\"classes\":[]}\\n'\n",
        )
        .expect("script");
        let mut permissions = std::fs::metadata(&worker).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&worker, permissions).expect("permissions");

        let client = HostWorkerClient::new_for_test(worker, Duration::from_secs(5));
        let value = client.factory_info("/tmp/Fake.vst3").await.expect("json");

        assert_eq!(value["vendor"], "WVST");

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn error_data_includes_worker_stderr() {
        let error = HostWorkerError::new_for_test_exited("module load failed");
        let data = error.rpc_data();

        assert_eq!(error.rpc_code(), 5032);
        assert_eq!(data["schemaVersion"], 1);
        assert_eq!(data["kind"], "exit");
        assert_eq!(data["stderr"], "module load failed");
        assert_eq!(data["classification"]["schemaVersion"], 1);
        assert_eq!(data["classification"]["category"], "host-worker-exit");
    }

    #[test]
    fn classifies_host_worker_launch_errors() {
        let error = HostWorkerError::Spawn {
            executable: PathBuf::from("/tmp/wvst-host-worker"),
            message: "not found".to_string(),
        };
        let data = error.rpc_data();

        assert_eq!(data["kind"], "spawn");
        assert_eq!(data["classification"]["category"], "host-worker-launch");
    }

    fn unique_temp_dir() -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("wvst-host-worker-test-{suffix}"))
    }
}
