use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::sse::{Event, Sse},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures::stream::{self, Stream};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::config::Config;
use crate::pool::ConnectionPool;
use crate::protocol::JsonRpcRequest;

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

/// Check the `Authorization: Bearer <token>` header against the configured
/// secret in constant time. Returns `true` when no token is configured.
fn http_authorized(state: &HttpState, headers: &HeaderMap) -> bool {
    let Some(ref token) = state.config.server.auth_token else {
        return true;
    };
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|presented| crate::auth::verify_token(token, presented))
        .unwrap_or(false)
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32600, "message": "Unauthorized" },
            "id": null
        })),
    )
        .into_response()
}

/// Handle JSON-RPC requests via HTTP POST
async fn handle_rpc(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Response {
    if !http_authorized(&state, &headers) {
        warn!("HTTP RPC request rejected: unauthorized");
        return unauthorized();
    }

    debug!("HTTP RPC request: {:?}", req.method);

    // Reuse server request processing
    let response = crate::server::process_request_http(&req, &state.pool, &state.config).await;

    Json(response).into_response()
}

/// Handle SSE subscriptions
async fn handle_subscribe(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if !http_authorized(&state, &headers) {
        warn!("SSE subscription rejected: unauthorized");
        return unauthorized();
    }
    Sse::new(subscribe_stream()).into_response()
}

fn subscribe_stream() -> impl Stream<Item = Result<Event, axum::Error>> {
    debug!("SSE subscription established");

    // For now, just send a simple hello message
    // In production, this would subscribe to query results or database changes
    stream::iter(vec![Ok(Event::default()
        .event("message")
        .json_data(json!({
            "jsonrpc": "2.0",
            "result": {
                "type": "subscription",
                "message": "Connected to MCP server"
            },
            "id": null
        }))
        .expect("valid json"))])
}

/// Health check endpoint
async fn handle_health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "mcp-postgres",
        "version": env!("CARGO_PKG_VERSION")
    }))
}
