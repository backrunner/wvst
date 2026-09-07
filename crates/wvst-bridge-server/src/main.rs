use wvst_bridge_server::{BridgeConfig, BridgeServer, HostWorkerClient};

mod diagnostics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(std::env::args().skip(1).collect()).await
}

async fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("wvst-bridge-server {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None | Some("serve") => serve().await,
        Some("diagnose" | "diagnostics") => print_diagnostics(),
        Some("--help" | "-h" | "help") => {
            println!(
                "usage: wvst-bridge-server [serve|diagnose]\n\nserve      start the local WVST bridge server\ndiagnose   print JSON diagnostics for config, platform, env, and host worker"
            );
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}").into()),
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
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

fn print_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let config = BridgeConfig::from_env_allow_missing_token()?;
    let host_worker = HostWorkerClient::from_env();
    let diagnostics = diagnostics::collect_bridge_diagnostics(&config, &host_worker);

    println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    Ok(())
}
