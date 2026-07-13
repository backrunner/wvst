use std::env;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use wvst_bridge_server::BridgeConfig;
use wvst_embed::{BridgeRuntime, BridgeRuntimeLogRecord};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:0";
const DEFAULT_ORIGIN: &str = "wvst-desktop://local";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let layout = PackagedBridgeLayout::from_env_or_current_exe()?;
    let bind_addr = env::var("WVST_EMBED_BIND_ADDR")
        .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string())
        .parse()?;

    let token = env::var("WVST_EMBED_TOKEN")
        .map_err(|_| "WVST_EMBED_TOKEN is required for the packaged desktop bridge")?;
    if token.is_empty() {
        return Err("WVST_EMBED_TOKEN must not be empty".into());
    }

    let builder = BridgeRuntime::builder(BridgeConfig::development(bind_addr))
        .token(token)
        .worker_executable(layout.host_worker)
        .worker_timeout(Duration::from_secs(5))
        .allowed_origins([
            env::var("WVST_EMBED_ALLOWED_ORIGIN").unwrap_or_else(|_| DEFAULT_ORIGIN.to_string())
        ])
        .allow_loopback_origins(true)
        .worker_auto_restart(true)
        .max_worker_instances(16)
        .worker_quarantine_failure_threshold(3)
        .worker_memory_limit_bytes(768 * 1024 * 1024)
        .worker_cpu_time_limit_seconds(60);

    let runtime = builder.build();
    let handle = runtime.start().await?;
    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "WVST embedded bridge listening on {}",
        handle.local_addr()
    )?;

    BridgeRuntimeLogRecord::diagnostics(handle.diagnostics(None)).write_json_line(&mut stdout)?;

    handle.shutdown().await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackagedBridgeLayout {
    host_worker: PathBuf,
}

impl PackagedBridgeLayout {
    fn from_env_or_current_exe() -> Result<Self, Box<dyn Error>> {
        if let Some(path) = env_path("WVST_EMBED_HOST_WORKER") {
            return Ok(Self { host_worker: path });
        }

        if let Some(package_dir) = env_path("WVST_EMBED_PACKAGE_DIR") {
            return Ok(Self {
                host_worker: package_dir
                    .join("bin")
                    .join(executable_name("wvst-host-worker")),
            });
        }

        let executable = env::current_exe()?;
        let app_bin_dir = executable
            .parent()
            .ok_or("current executable has no parent directory")?;

        Ok(Self {
            host_worker: app_bin_dir.join(executable_name("wvst-host-worker")),
        })
    }
}

fn env_path(name: &'static str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn executable_name(name: &'static str) -> String {
    format!("{name}{}", env::consts::EXE_SUFFIX)
}
