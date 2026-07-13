use std::env;
use std::error::Error;
use std::io::Write;

use wvst_bridge_server::BridgeConfig;
use wvst_embed::{BridgeRuntime, BridgeRuntimeLogRecord};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env::var("WVST_EMBED_TOKEN")
        .map_err(|_| "WVST_EMBED_TOKEN is required for the embedded bridge")?;
    if token.is_empty() {
        return Err("WVST_EMBED_TOKEN must not be empty".into());
    }
    let config = BridgeConfig::development("127.0.0.1:0".parse()?).with_token(token);
    let runtime = BridgeRuntime::builder(config)
        .max_worker_instances(8)
        .worker_quarantine_failure_threshold(3)
        .worker_memory_limit_bytes(512 * 1024 * 1024)
        .worker_cpu_time_limit_seconds(30)
        .build();
    let mut events = runtime.subscribe_events();

    let handle = runtime.start().await?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "WVST bridge listening on {}", handle.local_addr())?;

    while let Ok(event) = events.try_recv() {
        BridgeRuntimeLogRecord::event(event).write_json_line(&mut stdout)?;
    }

    let diagnostics = handle.diagnostics(None);
    BridgeRuntimeLogRecord::diagnostics(diagnostics).write_json_line(&mut stdout)?;

    handle.shutdown().await?;
    Ok(())
}
