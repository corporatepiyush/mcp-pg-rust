use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, post},
};
use futures::stream::{self, Stream};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::debug;

use crate::config::Config;
use crate::pool::ConnectionPool;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

#[derive(Clone)]
pub struct HttpState {
    pool: Arc<ConnectionPool>,
    config: Config,
}

pub async fn create_http_server(
    pool: Arc<ConnectionPool>,
    config: Config,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let http_state = HttpState {
        pool,
        config: config.clone(),
    };

    let app = Router::new()
        .route("/rpc", post(handle_rpc))
        .route("/subscribe", get(handle_subscribe))
        .route("/health", get(handle_health))
        .with_state(http_state);

    let addr = format!("{}:{}", config.server.host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("HTTP/2 server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Handle JSON-RPC requests via HTTP POST
async fn handle_rpc(
    State(state): State<HttpState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    debug!("HTTP RPC request: {:?}", req.method);

    // Reuse server request processing
    let response = crate::server::process_request_http(&req, &state.pool, &state.config).await;

    Json(response)
}

/// Handle SSE subscriptions
async fn handle_subscribe(
    State(_state): State<HttpState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    debug!("SSE subscription established");

    // For now, just send a simple hello message
    // In production, this would subscribe to query results or database changes
    let stream = stream::iter(vec![Ok(Event::default()
        .event("message")
        .json_data(json!({
            "jsonrpc": "2.0",
            "result": {
                "type": "subscription",
                "message": "Connected to MCP server"
            },
            "id": null
        }))
        .expect("valid json"))]);

    Sse::new(stream)
}

/// Health check endpoint
async fn handle_health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "mcp-postgres",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
