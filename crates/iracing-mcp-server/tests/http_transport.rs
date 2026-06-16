use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use iracing_mcp_server::{adapter, mcp, transport};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn http_healthz_works() {
    let state = mcp::ServerState::new(std::sync::Arc::new(adapter::StubAdapter::default()));
    let app = transport::http::build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .method("GET")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn http_mcp_initialize_and_tools_call_work() {
    let state = mcp::ServerState::new(std::sync::Arc::new(adapter::StubAdapter::default()));
    let app = transport::http::build_router(state);

    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let init_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/mcp")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(init_req.to_string()))
                .expect("valid request"),
        )
        .await
        .expect("router response");

    assert_eq!(init_res.status(), StatusCode::OK);
    let init_body = to_bytes(init_res.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let init_json: Value = serde_json::from_slice(&init_body).expect("json body");
    assert_eq!(
        init_json["result"]["protocolVersion"],
        Value::String("2025-06-18".to_string())
    );

    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "get_session_overview",
            "arguments": {}
        }
    });

    let call_res = app
        .oneshot(
            Request::builder()
                .uri("/mcp")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(call_req.to_string()))
                .expect("valid request"),
        )
        .await
        .expect("router response");

    assert_eq!(call_res.status(), StatusCode::OK);
    let call_body = to_bytes(call_res.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let call_json: Value = serde_json::from_slice(&call_body).expect("json body");

    assert_eq!(call_json["result"]["content"][0]["type"], "json");
    assert_eq!(
        call_json["result"]["content"][0]["json"]["data"]["connected"],
        Value::Bool(true)
    );
}
