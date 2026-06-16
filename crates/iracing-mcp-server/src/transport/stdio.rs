use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::{handle_request, JsonRpcRequest, ServerState};

pub async fn run_stdio(state: ServerState) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(&state, req).await,
            Err(_) => crate::mcp::JsonRpcResponse {
                jsonrpc: "2.0",
                id: serde_json::Value::Null,
                result: None,
                error: Some(crate::mcp::JsonRpcError {
                    code: -32700,
                    message: "parse error".to_string(),
                }),
            },
        };

        let payload = serde_json::to_string(&response)?;
        stdout.write_all(payload.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}
