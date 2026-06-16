use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::adapter::AdapterRef;

#[derive(Clone)]
pub struct ServerState {
    adapter: AdapterRef,
}

impl ServerState {
    pub fn new(adapter: AdapterRef) -> Self {
        Self { adapter }
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

pub async fn handle_request(state: &ServerState, req: JsonRpcRequest) -> JsonRpcResponse {
    if req.jsonrpc != "2.0" {
        return err(req.id, -32600, "invalid request: jsonrpc must be 2.0");
    }

    match req.method.as_str() {
        "initialize" => ok(
            req.id,
            json!({
                "protocolVersion": "2025-06-18",
                "serverInfo": {
                    "name": "iracing-mcp-server",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {
                        "listChanged": true
                    }
                }
            }),
        ),
        "tools/list" => ok(
            req.id,
            json!({
                "tools": [
                    {
                        "name": "get_session_overview",
                        "description": "Returns current iRacing session connectivity and mode.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        }
                    }
                ]
            }),
        ),
        "tools/call" => tools_call(state, req.id, req.params).await,
        _ => err(req.id, -32601, "method not found"),
    }
}

async fn tools_call(state: &ServerState, id: Option<Value>, params: Value) -> JsonRpcResponse {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match name {
        "get_session_overview" => {
            let overview = state.adapter.get_session_overview().await;
            ok(
                id,
                json!({
                    "content": [
                        {
                            "type": "json",
                            "json": {
                                "ok": true,
                                "data": overview,
                                "warnings": [],
                                "error": null
                            }
                        }
                    ]
                }),
            )
        }
        _ => err(id, -32602, "unknown tool name"),
    }
}

fn ok(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: id.unwrap_or(Value::Null),
        result: Some(result),
        error: None,
    }
}

fn err(id: Option<Value>, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: id.unwrap_or(Value::Null),
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
        }),
    }
}
