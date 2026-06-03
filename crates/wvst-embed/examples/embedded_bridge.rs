use std::error::Error;

use wvst_bridge_server::BridgeConfig;
use wvst_embed::BridgeRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = BridgeConfig::development("127.0.0.1:0".parse()?);
    let runtime = BridgeRuntime::builder(config)
        .max_worker_instances(8)
        .worker_quarantine_failure_threshold(3)
        .worker_memory_limit_bytes(512 * 1024 * 1024)
        .worker_cpu_time_limit_seconds(30)
        .build();
    let mut events = runtime.subscribe_events();

    let handle = runtime.start().await?;
    println!("WVST bridge listening on {}", handle.local_addr());

    while let Ok(event) = events.try_recv() {
        println!("event #{}: {:?}", event.sequence, event.kind);
    }

    let diagnostics = handle.diagnostics(None);
    println!(
        "metrics: websocketConnections={}, workerFailures={}, workerRestarts={}",
        diagnostics.metrics.websocket_connections,
        diagnostics.metrics.worker_failures,
        diagnostics.metrics.worker_restarts
    );

    handle.shutdown().await?;
    Ok(())
}
