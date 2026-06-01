use wvst_bridge_server::{BridgeConfig, BridgeServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BridgeConfig::from_env()?;
    let server = BridgeServer::bind(config).await?;
    let addr = server.local_addr()?;

    eprintln!("wvst-bridge-server listening on ws://{addr}");

    server
        .serve_until(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}
