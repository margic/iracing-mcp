use std::sync::Arc;

use clap::{Parser, ValueEnum};
use iracing_mcp_server::{adapter, mcp, transport};
use tracing::info;

use adapter::{AdapterRef, SdkAdapter};

#[derive(Debug, Clone, ValueEnum)]
enum TransportKind {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
#[command(author, version, about = "iRacing MCP server skeleton")]
struct Cli {
    #[arg(long, value_enum, default_value = "http")]
    transport: TransportKind,

    #[arg(long, default_value = "0.0.0.0:8765")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let adapter: AdapterRef = Arc::new(SdkAdapter);
    let state = mcp::ServerState::new(adapter);

    let startup_message = match &cli.transport {
        TransportKind::Stdio => {
            "iracing-mcp-server running (transport=stdio)".to_string()
        }
        TransportKind::Http => {
            format!("iracing-mcp-server running (transport=http, bind={})", cli.bind)
        }
    };
    // Always print startup confirmation so operators can verify the process is live
    // even when RUST_LOG filters suppress tracing output.
    eprintln!("{startup_message}");

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
