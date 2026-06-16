use axum::{extract::State, routing::{get, post}, Json, Router};
use serde_json::Value;
use tokio::net::TcpListener;

use crate::mcp::{handle_request, JsonRpcRequest, JsonRpcResponse, ServerState};

pub fn build_router(state: ServerState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/mcp", post(mcp_handler))
        .with_state(state)
}

pub async fn run_http(bind: &str, state: ServerState) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, build_router(state)).await?;
    Ok(())
}

async fn healthz() -> Json<Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn mcp_handler(
    State(state): State<ServerState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    Json(handle_request(&state, request).await)
}
