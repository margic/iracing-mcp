use std::sync::Arc;

use clap::{Parser, ValueEnum};
use iracing_mcp_server::{adapter, mcp, transport};
use tracing::info;

use adapter::{AdapterRef, StubAdapter};

#[derive(Debug, Clone, ValueEnum)]
enum TransportKind {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
#[command(author, version, about = "iRacing MCP server skeleton")]
struct Cli {
    #[arg(long, value_enum, default_value = "stdio")]
    transport: TransportKind,

    #[arg(long, default_value = "127.0.0.1:8765")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let adapter: AdapterRef = Arc::new(StubAdapter::default());
    let state = mcp::ServerState::new(adapter);

    match cli.transport {
        TransportKind::Stdio => {
            info!("starting stdio transport");
            transport::stdio::run_stdio(state).await?;
        }
        TransportKind::Http => {
            info!(bind = %cli.bind, "starting http transport");
            transport::http::run_http(&cli.bind, state).await?;
        }
    }

    Ok(())
}
