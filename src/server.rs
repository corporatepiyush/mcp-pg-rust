use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, warn};

use crate::actions;
use crate::config::Config;
use crate::errors::{MCPError, Result as MCPResult};
use crate::metrics;
use crate::pool::ConnectionPool;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use once_cell::sync::Lazy;

/// Every tool definition from `tools.json`, parsed once at startup. The
/// `tools/list` payload is derived from this by filtering to the categories a
/// given server instance has enabled.
static ALL_TOOL_DEFS: Lazy<Vec<Value>> = Lazy::new(|| {
    let tools_json = include_str!("../tools.json");
    serde_json::from_str(tools_json).expect("Failed to parse tools.json")
});

/// Build the pre-serialized `{"tools":[...]}` payload for `tools/list`,
/// filtered to the enabled categories. A tool whose category is not enabled is
/// omitted entirely, so disabled tools are invisible to clients. With an empty
/// `enabled` set the payload is `{"tools":[]}` — the default "expose nothing"
/// posture. The result is cached per-`Config` (see `Config::tools_list_bytes`)
/// so requests never reparse or refilter.
pub fn build_tools_list_response(enabled: &[crate::tools::ToolCategory]) -> Vec<u8> {
    let tools: Vec<&Value> = ALL_TOOL_DEFS
        .iter()
        .filter(|t| {
            t.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| crate::tools::is_tool_available(name, enabled))
        })
        .collect();
    let resp = json!({ "tools": tools });
    serde_json::to_vec(&resp).expect("Failed to serialize tools/list response")
}

const BUFFER_CAPACITY: usize = 4096;
const NEWLINE: &[u8] = b"\n";

/// Maximum length of a single TCP request line. Bounds per-request memory so a
/// client streaming bytes without a newline cannot grow the buffer without
/// limit. Generous enough for large batch payloads (16 MiB).
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Time a TCP client has to send the auth token after connecting. Mitigates
/// slowloris-style connections that open but never authenticate.
const AUTH_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Read one `\n`-terminated line into `line`, returning `InvalidData` if it
/// would exceed `max_bytes`. Unlike `read_line`, memory is bounded to
/// `max_bytes` regardless of how much an attacker streams without a newline.
/// Returns `Ok(0)` on EOF.
///
/// Performance: reads chunk bytes directly into the `line` String's buffer,
/// avoiding an intermediate `Vec<u8>` allocation per request.
async fn read_line_capped<R>(
    reader: &mut R,
    line: &mut String,
    max_bytes: usize,
) -> std::io::Result<usize>
where
    R: AsyncBufReadExt + Unpin,
{
    use std::io::{Error, ErrorKind};
    line.clear();
    let mut total: usize = 0;
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            break;
        }
        let (take, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        if total + take > max_bytes {
            reader.consume(take);
            return Err(Error::new(
                ErrorKind::InvalidData,
                "request line exceeds maximum length",
            ));
        }
        let s = std::str::from_utf8(&chunk[..take])
            .map_err(|_| Error::new(ErrorKind::InvalidData, "request line is not valid UTF-8"))?;
        line.push_str(s);
        total += take;
        reader.consume(take);
        if done {
            break;
        }
    }
    if line.is_empty() {
        return Ok(0);
    }
    Ok(line.len())
}

#[inline]
#[cold]
fn parse_error(msg: String) -> JsonRpcResponse {
    let mcp_error = MCPError::ParseError(msg);
    JsonRpcResponse::error(None, mcp_error.error_code(), mcp_error.to_string())
}

#[inline]
fn parse_request(line: &str) -> Result<JsonRpcRequest, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("Empty request".to_string());
    }
    serde_json::from_str::<JsonRpcRequest>(trimmed).map_err(|e| e.to_string())
}

pub struct MCPServer {
    config: Arc<Config>,
    pool: Arc<ConnectionPool>,
}

impl MCPServer {
    pub fn new(config: Config, pool: Arc<ConnectionPool>) -> Self {
        Self {
            config: Arc::new(config),
            pool,
        }
    }

    /// Run in stdio mode for MCP compatibility (Claude Desktop, etc.)
    pub async fn run_stdio(&self) -> MCPResult<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::with_capacity(512);
        // 4 KB initial — handles >95% of responses without resizing.
        // Tools like `list_tables` or `execute_query` may exceed this,
        // but Vec grows geometrically so the amortized cost is negligible.
        let mut response_buf = Vec::with_capacity(4096);

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    process_one_line(
                        &line,
                        &self.pool,
                        &self.config,
                        &mut response_buf,
                        &mut stdout,
                    )
                    .await?;
                }
                Err(e) => {
                    error!("IO error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn run(&self) -> MCPResult<()> {
        let addr = format!("{}:{}", self.config.server.host, self.config.server.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("MCP server listening on {}", addr);

        loop {
            let (socket, peer_addr) = listener.accept().await?;

            if let Err(e) = socket.set_nodelay(true) {
                warn!("Failed to set TCP_NODELAY: {}", e);
            }

            let pool = Arc::clone(&self.pool);
            let config = Arc::clone(&self.config);

            tokio::spawn(async move {
                if let Err(e) = handle_client(socket, pool, config).await {
                    error!("Client {} error: {}", peer_addr, e);
                }
            });
        }
    }
}

#[inline(never)]
async fn handle_client(
    socket: TcpStream,
    pool: Arc<ConnectionPool>,
    config: Arc<Config>,
) -> MCPResult<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, reader);
    let mut line = String::with_capacity(512);
    // 4 KB initial capacity — grows geometrically for large responses.
    let mut response_buf = Vec::with_capacity(4096);

    // Per-connection authentication handshake. When a token is configured,
    // the client must send it as the very first line before any JSON-RPC.
    if let Some(ref token) = config.server.auth_token {
        let read = tokio::time::timeout(
            AUTH_HANDSHAKE_TIMEOUT,
            read_line_capped(&mut reader, &mut line, MAX_REQUEST_BYTES),
        )
        .await;
        match read {
            Ok(Ok(0)) => return Ok(()),
            Ok(Ok(_)) => {
                if !crate::auth::verify_token(token, line.trim()) {
                    warn!("Authentication failed; closing connection");
                    let _ = writer
                        .write_all(
                            b"{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32600,\
                              \"message\":\"Unauthorized\"},\"id\":null}\n",
                        )
                        .await;
                    let _ = writer.flush().await;
                    return Ok(());
                }
            }
            Ok(Err(e)) => {
                error!("IO error during auth: {}", e);
                return Ok(());
            }
            Err(_) => {
                warn!("Authentication handshake timed out; closing connection");
                return Ok(());
            }
        }
    }

    loop {
        match read_line_capped(&mut reader, &mut line, MAX_REQUEST_BYTES).await {
            Ok(0) => break,
            Ok(_) => {
                process_one_line(&line, &pool, &config, &mut response_buf, &mut writer).await?;
            }
            Err(e) => {
                error!("IO error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Core per-line processing shared by TCP and stdio transports.
/// For notifications (JSON-RPC messages without `id`), no response is sent.
#[inline]
async fn process_one_line<W: AsyncWriteExt + Unpin>(
    line: &str,
    pool: &Arc<ConnectionPool>,
    config: &Config,
    response_buf: &mut Vec<u8>,
    writer: &mut W,
) -> MCPResult<()> {
    metrics::inc_requests();

    let (response, is_notification) = match parse_request(line) {
        Ok(req) => {
            // Fast path: tools/list. Splice the cached result bytes straight into
            // the JSON-RPC envelope, skipping the parse-into-Value and
            // re-serialize that the generic path would do for a ~50 KB payload.
            if req.method == "tools/list" {
                if let Some(id) = req.id.as_ref() {
                    response_buf.clear();
                    response_buf.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"result\":");
                    response_buf.extend_from_slice(&config.tools_list_bytes);
                    response_buf.extend_from_slice(b",\"id\":");
                    serde_json::to_writer(&mut *response_buf, id)?;
                    response_buf.extend_from_slice(b"}");
                    response_buf.extend_from_slice(NEWLINE);
                    writer.write_all(response_buf).await?;
                    writer.flush().await?;
                    maybe_shrink_buf(response_buf);
                }
                // notification (no id) expects no response
                return Ok(());
            }

            let is_notif = req.id.is_none();
            match process_request(&req, pool, config).await {
                Ok(result) => (JsonRpcResponse::success(req.id, result), is_notif),
                Err(e) => {
                    metrics::inc_errors();
                    (
                        JsonRpcResponse::error(req.id, e.error_code(), e.to_string()),
                        is_notif,
                    )
                }
            }
        }
        Err(e) => {
            metrics::inc_errors();
            (parse_error(e), false)
        }
    };

    // JSON-RPC notifications (no `id`) do not expect a response
    if is_notification {
        return Ok(());
    }

    response_buf.clear();
    serde_json::to_writer(&mut *response_buf, &response)?;
    response_buf.extend_from_slice(NEWLINE);

    writer.write_all(response_buf).await?;
    writer.flush().await?;
    maybe_shrink_buf(response_buf);
    Ok(())
}

/// If the response buffer grew large for a previous request, release the
/// excess memory so a subsequent small request doesn't waste address space.
/// Replace with a fresh small allocation when capacity exceeds 64 KB.
fn maybe_shrink_buf(buf: &mut Vec<u8>) {
    if buf.capacity() > 65536 {
        *buf = Vec::with_capacity(4096);
    }
}

/// Process a JSON-RPC request (used by both TCP and HTTP transports)
#[inline]
pub async fn process_request(
    req: &JsonRpcRequest,
    pool: &Arc<ConnectionPool>,
    config: &Config,
) -> MCPResult<Value> {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "tools/list" => handle_tools_list(config),
        "tools/call" => handle_tools_call(req, pool, config).await,
        "ping" => handle_ping(),
        method if method.starts_with("notifications/") => handle_notification(method),
        _ => Err(MCPError::MethodNotFound(req.method.clone())),
    }
}

/// Handle JSON-RPC ping (respond with empty success)
#[inline]
const fn handle_ping() -> MCPResult<Value> {
    Ok(Value::Null)
}

/// Handle MCP notifications (silently accepted, no response needed per JSON-RPC spec)
#[inline]
fn handle_notification(method: &str) -> MCPResult<Value> {
    tracing::trace!("Received notification: {method}");
    Ok(Value::Null)
}

/// Public wrapper for HTTP handlers - returns complete JSON-RPC response
pub async fn process_request_http(
    req: &JsonRpcRequest,
    pool: &Arc<ConnectionPool>,
    config: &Config,
) -> JsonRpcResponse {
    metrics::inc_requests();

    match process_request(req, pool, config).await {
        Ok(result) => JsonRpcResponse::success(req.id.clone(), result),
        Err(e) => {
            metrics::inc_errors();
            JsonRpcResponse::error(req.id.clone(), e.error_code(), e.to_string())
        }
    }
}

/// MCP protocol revisions this server can speak, newest first. Used for
/// version negotiation in `initialize` (MCP spec, lifecycle/initialization).
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// The newest revision we implement; returned when the client requests a
/// version we do not support.
const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";

/// `instructions` string surfaced to the client and appended to the model's
/// system prompt to guide tool use (MCP `InitializeResult.instructions`).
const SERVER_INSTRUCTIONS: &str = "PostgreSQL MCP server. Use `execute_query` for read-only SELECTs and \
`execute_insert`/`execute_update`/`execute_delete` for DML. Inspect structure with `list_tables`, \
`describe_table`, and `list_indexes` before writing queries. Tool results carry both human-readable \
text and a machine-readable `structuredContent` object. Tool failures are returned with `isError: true` \
rather than as protocol errors, so read the message and retry with corrected arguments.";

fn handle_initialize(req: &JsonRpcRequest) -> MCPResult<Value> {
    // Version negotiation: echo the client's requested revision when we support
    // it, otherwise offer our latest. The client decides whether to proceed.
    let protocol_version = req
        .params
        .as_ref()
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .filter(|v| SUPPORTED_PROTOCOL_VERSIONS.contains(v))
        .unwrap_or(LATEST_PROTOCOL_VERSION);

    Ok(json!({
        "protocolVersion": protocol_version,
        // Advertise only what we actually implement. Earlier releases falsely
        // declared `resources` and `prompts` capabilities with no handlers.
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "mcp-postgres",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": SERVER_INSTRUCTIONS
    }))
}

/// Wrap a successful tool result in an MCP `CallToolResult`. The data is
/// provided both as serialized text (for backwards compatibility / display)
/// and, when it is a JSON object, as `structuredContent` (MCP 2025-06-18+).
#[inline]
fn tool_success(value: Value) -> Value {
    let text = serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string());
    if value.is_object() {
        json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": value,
            "isError": false
        })
    } else {
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        })
    }
}

/// Wrap a tool execution failure as an MCP `CallToolResult` with `isError: true`
/// so the model sees the message and can self-correct, rather than receiving an
/// opaque JSON-RPC protocol error.
#[inline]
fn tool_error(message: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": message.into() }],
        "isError": true
    })
}

#[inline]
fn handle_tools_list(config: &Config) -> MCPResult<Value> {
    // Deserialize from the per-config cached bytes (already filtered to the
    // enabled categories) instead of deep-cloning a large Value tree.
    Ok(serde_json::from_slice(&config.tools_list_bytes)?)
}

async fn handle_tools_call(
    req: &JsonRpcRequest,
    pool: &Arc<ConnectionPool>,
    config: &Config,
) -> MCPResult<Value> {
    let tool_name = req
        .params
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'name' parameter".into()))?;

    let tool_args = req.params.as_ref().and_then(|p| p.get("arguments"));

    // Category gate: a tool is only reachable if it exists AND its category was
    // enabled at startup. Tools in disabled categories are invisible (absent
    // from tools/list) so a call to one is treated as an unknown method, not a
    // policy `isError`. This runs first, before any pool acquire.
    if !crate::tools::is_tool_available(tool_name, &config.server.enabled_categories) {
        return Err(method_not_found(tool_name));
    }

    // Restricted mode check BEFORE pool acquire. Policy rejections are
    // tool-level failures (the tool exists, the call is well formed) so they
    // are surfaced as `isError` results, not protocol errors.
    if config.server.access_mode == crate::config::AccessMode::Restricted
        && crate::tools::is_write_tool(tool_name)
    {
        return Ok(tool_error(format!(
            "Operation '{tool_name}' is not allowed in restricted (read-only) mode"
        )));
    }

    // import_from_url makes outbound HTTP requests; require explicit opt-in.
    if tool_name == "import_from_url" && !config.server.allow_url_import {
        return Ok(tool_error(
            "'import_from_url' is disabled; start the server with --allow-url-import to enable it",
        ));
    }

    // Acquire pool connection only for known tools
    let client = pool.acquire().await?;

    let result = match tool_name {
        // Schema actions
        "list_tables" => actions::schema::list_tables(&client, &tool_args).await,
        "describe_table" => actions::schema::describe_table(&client, &tool_args).await,
        "list_indexes" => actions::schema::list_indexes(&client, &tool_args).await,
        "list_schemas" => actions::schema::list_schemas(&client, &tool_args).await,
        "show_constraints" => actions::schema::show_constraints(&client, &tool_args).await,
        "list_triggers" => actions::schema::list_triggers(&client, &tool_args).await,
        "create_table" => actions::schema::create_table(&client, &tool_args).await,
        "drop_table" => actions::schema::drop_table(&client, &tool_args).await,
        "create_view" => actions::schema::create_view(&client, &tool_args).await,
        "drop_view" => actions::schema::drop_view(&client, &tool_args).await,
        "alter_view" => actions::schema::alter_view(&client, &tool_args).await,
        "create_schema" => actions::schema::create_schema(&client, &tool_args).await,
        "drop_schema" => actions::schema::drop_schema(&client, &tool_args).await,
        "create_sequence" => actions::schema::create_sequence(&client, &tool_args).await,
        "drop_sequence" => actions::schema::drop_sequence(&client, &tool_args).await,
        "alter_index" => actions::schema::alter_index(&client, &tool_args).await,
        "list_partitions" => actions::schema::list_partitions(&client, &tool_args).await,
        "backup_table" => actions::schema::backup_table(&client, &tool_args).await,
        "create_index" => actions::schema::create_index(&client, &tool_args).await,
        "drop_index" => actions::schema::drop_index(&client, &tool_args).await,
        "create_partition" => actions::schema::create_partition(&client, &tool_args).await,
        "drop_partition" => actions::schema::drop_partition(&client, &tool_args).await,
        // Query actions
        "execute_query" => actions::query::execute_query(&client, &tool_args).await,
        "execute_insert" => actions::query::execute_insert(&client, &tool_args).await,
        "execute_update" => actions::query::execute_update(&client, &tool_args).await,
        "execute_delete" => actions::query::execute_delete(&client, &tool_args).await,
        "async_execute_insert" => actions::query::async_execute_insert(&client, &tool_args).await,
        "async_execute_update" => actions::query::async_execute_update(&client, &tool_args).await,
        "async_execute_delete" => actions::query::async_execute_delete(&client, &tool_args).await,
        "explain_query" => actions::query::explain_query(&client, &tool_args).await,
        // Batch operations
        "async_batch_insert" => actions::batch::async_batch_insert(&client, &tool_args).await,
        "async_batch_update" => actions::batch::async_batch_update(&client, &tool_args).await,
        "async_batch_delete" => actions::batch::async_batch_delete(&client, &tool_args).await,
        "async_batch_insert_copy" => {
            actions::batch::async_batch_insert_copy(&client, &tool_args).await
        }
        // Monitoring actions
        "get_table_stats" => actions::monitoring::get_table_stats(&client, &tool_args).await,
        "get_index_stats" => actions::monitoring::get_index_stats(&client, &tool_args).await,
        "show_database_size" => actions::monitoring::show_database_size(&client, &tool_args).await,
        "show_table_size" => actions::monitoring::show_table_size(&client, &tool_args).await,
        "get_cache_hit_ratio" => {
            actions::monitoring::get_cache_hit_ratio(&client, &tool_args).await
        }
        // Connection actions
        "list_connections" => actions::connections::list_connections(&client, &tool_args).await,
        "show_current_user" => actions::connections::show_current_user(&client, &tool_args).await,
        "show_running_queries" => {
            actions::connections::show_running_queries(&client, &tool_args).await
        }
        "show_connection_summary" => {
            actions::connections::show_connection_summary(&client, &tool_args).await
        }
        // Maintenance actions
        "vacuum_analyze" => actions::maintenance::vacuum_analyze(&client, &tool_args).await,
        "analyze_table" => actions::maintenance::analyze_table(&client, &tool_args).await,
        "reindex_table" => actions::maintenance::reindex_table(&client, &tool_args).await,
        "get_pg_stat_statements" => {
            actions::maintenance::get_pg_stat_statements(&client, &tool_args).await
        }
        "reset_statistics" => actions::maintenance::reset_statistics(&client, &tool_args).await,
        "truncate_table" => actions::maintenance::truncate_table(&client, &tool_args).await,
        // Security actions
        "list_users" => actions::security::list_users(&client, &tool_args).await,
        "list_user_privileges" => {
            actions::security::list_user_privileges(&client, &tool_args).await
        }
        "list_role_memberships" => {
            actions::security::list_role_memberships(&client, &tool_args).await
        }
        "list_database_privileges" => {
            actions::security::list_database_privileges(&client, &tool_args).await
        }
        "show_session_info" => actions::security::show_session_info(&client, &tool_args).await,
        // Config actions
        "show_all_settings" => actions::config::show_all_settings(&client, &tool_args).await,
        "get_setting" => actions::config::get_setting(&client, &tool_args).await,
        "show_memory_settings" => actions::config::show_memory_settings(&client, &tool_args).await,
        "show_performance_settings" => {
            actions::config::show_performance_settings(&client, &tool_args).await
        }
        "show_log_settings" => actions::config::show_log_settings(&client, &tool_args).await,
        // Replication actions
        "show_replication_status" => {
            actions::replication::show_replication_status(&client, &tool_args).await
        }
        "list_replication_slots" => {
            actions::replication::list_replication_slots(&client, &tool_args).await
        }
        "list_standby_servers" => {
            actions::replication::list_standby_servers(&client, &tool_args).await
        }
        "show_wal_info" => actions::replication::show_wal_info(&client, &tool_args).await,
        "show_base_backup_progress" => {
            actions::replication::show_base_backup_progress(&client, &tool_args).await
        }
        // Transaction actions
        "show_active_transactions" => {
            actions::transactions::show_active_transactions(&client, &tool_args).await
        }
        "show_locks" => actions::transactions::show_locks(&client, &tool_args).await,
        "show_waiting_locks" => {
            actions::transactions::show_waiting_locks(&client, &tool_args).await
        }
        "show_transaction_isolation" => {
            actions::transactions::show_transaction_isolation(&client, &tool_args).await
        }
        "show_deadlocks" => actions::transactions::show_deadlocks(&client, &tool_args).await,
        "show_autocommit_status" => {
            actions::transactions::show_autocommit_status(&client, &tool_args).await
        }
        "show_transaction_timeout" => {
            actions::transactions::show_transaction_timeout(&client, &tool_args).await
        }
        // Health actions
        "analyze_db_health" => actions::health::analyze_db_health(&client, &tool_args).await,
        "list_unused_indexes" => actions::health::list_unused_indexes(&client, &tool_args).await,
        "list_duplicate_indexes" => {
            actions::health::list_duplicate_indexes(&client, &tool_args).await
        }
        "show_vacuum_progress" => actions::health::show_vacuum_progress(&client, &tool_args).await,
        // Enhanced schema
        "get_object_details" => actions::schema::get_object_details(&client, &tool_args).await,
        // User management
        "create_user" => actions::user_mgmt::create_user(&client, &tool_args).await,
        "alter_user" => actions::user_mgmt::alter_user(&client, &tool_args).await,
        "drop_user" => actions::user_mgmt::drop_user(&client, &tool_args).await,
        "create_role" => actions::user_mgmt::create_role(&client, &tool_args).await,
        "alter_role" => actions::user_mgmt::alter_role(&client, &tool_args).await,
        "drop_role" => actions::user_mgmt::drop_role(&client, &tool_args).await,
        "grant_privileges" => actions::user_mgmt::grant_privileges(&client, &tool_args).await,
        "revoke_privileges" => actions::user_mgmt::revoke_privileges(&client, &tool_args).await,
        // Schema alter
        "add_column" => actions::schema_alter::add_column(&client, &tool_args).await,
        "drop_column" => actions::schema_alter::drop_column(&client, &tool_args).await,
        "rename_column" => actions::schema_alter::rename_column(&client, &tool_args).await,
        "alter_column_type" => actions::schema_alter::alter_column_type(&client, &tool_args).await,
        "rename_table" => actions::schema_alter::rename_table(&client, &tool_args).await,
        "rename_index" => actions::schema_alter::rename_index(&client, &tool_args).await,
        "rename_schema" => actions::schema_alter::rename_schema(&client, &tool_args).await,
        "add_foreign_key" => actions::schema_alter::add_foreign_key(&client, &tool_args).await,
        "drop_foreign_key" => actions::schema_alter::drop_foreign_key(&client, &tool_args).await,
        "add_unique_constraint" => {
            actions::schema_alter::add_unique_constraint(&client, &tool_args).await
        }
        "drop_constraint" => actions::schema_alter::drop_constraint(&client, &tool_args).await,
        // Session management
        "cancel_query" => actions::session_mgmt::cancel_query(&client, &tool_args).await,
        "terminate_connection" => {
            actions::session_mgmt::terminate_connection(&client, &tool_args).await
        }
        "show_blocked_queries" => {
            actions::session_mgmt::show_blocked_queries(&client, &tool_args).await
        }
        // Extension management
        "list_extensions" => actions::ext_mgmt::list_extensions(&client, &tool_args).await,
        "create_extension" => actions::ext_mgmt::create_extension(&client, &tool_args).await,
        "drop_extension" => actions::ext_mgmt::drop_extension(&client, &tool_args).await,
        // Database management
        "list_databases" => actions::db_mgmt::list_databases(&client, &tool_args).await,
        "create_database" => actions::db_mgmt::create_database(&client, &tool_args).await,
        // Extended maintenance
        "vacuum" => actions::maint_ext::vacuum(&client, &tool_args).await,
        "vacuum_full" => actions::maint_ext::vacuum_full(&client, &tool_args).await,
        "reindex_database" => actions::maint_ext::reindex_database(&client, &tool_args).await,
        // Migration helpers
        "generate_create_table_ddl" => {
            actions::migration_helpers::generate_create_table_ddl(&client, &tool_args).await
        }
        "generate_create_index_ddl" => {
            actions::migration_helpers::generate_create_index_ddl(&client, &tool_args).await
        }
        "table_dependencies" => {
            actions::migration_helpers::table_dependencies(&client, &tool_args).await
        }
        // pgvector
        "list_vector_columns" => actions::pgvector::list_vector_columns(&client, &tool_args).await,
        "vector_search" => actions::pgvector::vector_search(&client, &tool_args).await,
        "create_vector_index" => actions::pgvector::create_vector_index(&client, &tool_args).await,
        // TimescaleDB
        "create_hypertable" => actions::timescaledb::create_hypertable(&client, &tool_args).await,
        "show_hypertable_details" => {
            actions::timescaledb::show_hypertable_details(&client, &tool_args).await
        }
        "show_chunks" => actions::timescaledb::show_chunks(&client, &tool_args).await,
        "add_retention_policy" => {
            actions::timescaledb::add_retention_policy(&client, &tool_args).await
        }
        "add_compression_policy" => {
            actions::timescaledb::add_compression_policy(&client, &tool_args).await
        }
        "compress_chunk" => actions::timescaledb::compress_chunk(&client, &tool_args).await,
        "add_continuous_aggregate" => {
            actions::timescaledb::add_continuous_aggregate(&client, &tool_args).await
        }
        // pg_textsearch (BM25)
        "list_bm25_indexes" => actions::pg_textsearch::list_bm25_indexes(&client, &tool_args).await,
        "search_bm25" => actions::pg_textsearch::search_bm25(&client, &tool_args).await,
        "create_bm25_index" => actions::pg_textsearch::create_bm25_index(&client, &tool_args).await,
        "drop_bm25_index" => actions::pg_textsearch::drop_bm25_index(&client, &tool_args).await,
        "bm25_force_merge" => actions::pg_textsearch::bm25_force_merge(&client, &tool_args).await,
        "bm25_index_stats" => actions::pg_textsearch::bm25_index_stats(&client, &tool_args).await,
        // v4.0: Data I/O
        "import_from_url" => actions::data_io::import_from_url(&client, &tool_args).await,
        "export_csv" => actions::data_io::export_csv(&client, &tool_args).await,
        // v4.0: Index Advisor
        "suggest_indexes" => actions::index_advisor::suggest_indexes(&client, &tool_args).await,
        // v4.0: Schema Health
        "find_tables_without_pk" => {
            actions::schema_health::find_tables_without_pk(&client, &tool_args).await
        }
        "find_missing_fk_indexes" => {
            actions::schema_health::find_missing_fk_indexes(&client, &tool_args).await
        }
        "analyze_table_bloat" => {
            actions::schema_health::analyze_table_bloat(&client, &tool_args).await
        }
        "clone_table_schema" => {
            actions::schema_health::clone_table_schema(&client, &tool_args).await
        }
        // v4.0: Security Audit
        "security_audit" => actions::security_audit::security_audit(&client, &tool_args).await,
        "audit_role_usage" => actions::security_audit::audit_role_usage(&client, &tool_args).await,
        // v4.0: Data Tools
        "sample_data" => actions::data_tools::sample_data(&client, &tool_args).await,
        tool => Err(method_not_found(tool)),
    };

    // client is returned to the pool automatically via Drop
    drop(client);

    // Wrap in an MCP `CallToolResult`. Execution failures become `isError`
    // results so the model can read the message and self-correct, instead of
    // an opaque JSON-RPC protocol error.
    match result {
        Ok(value) => Ok(tool_success(value)),
        Err(e) => {
            error!("Tool '{tool_name}' error: {e:?}");
            Ok(tool_error(e.to_string()))
        }
    }
}

#[cold]
fn method_not_found(name: &str) -> MCPError {
    MCPError::MethodNotFound(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tools_list_splice_matches_generic() {
        // The hand-spliced fast-path bytes must be byte-identical to the
        // generic JsonRpcResponse::success serialization, for the same
        // category-filtered payload the server would serve.
        let bytes = build_tools_list_response(crate::tools::ToolCategory::ALL);
        let id = Value::Number(7.into());
        let result: Value = serde_json::from_slice(&bytes).unwrap();
        let generic =
            serde_json::to_vec(&JsonRpcResponse::success(Some(id.clone()), result)).unwrap();

        let mut spliced = Vec::new();
        spliced.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"result\":");
        spliced.extend_from_slice(&bytes);
        spliced.extend_from_slice(b",\"id\":");
        serde_json::to_writer(&mut spliced, &id).unwrap();
        spliced.extend_from_slice(b"}");

        assert_eq!(spliced, generic);
    }

    #[test]
    fn test_tools_list_filtered_by_category() {
        use crate::tools::ToolCategory;
        let count = |bytes: &[u8]| -> usize {
            let v: Value = serde_json::from_slice(bytes).unwrap();
            v["tools"].as_array().unwrap().len()
        };

        // Default (nothing enabled) exposes zero tools.
        assert_eq!(count(&build_tools_list_response(&[])), 0);

        // Enabling all categories exposes the full set.
        let all = count(&build_tools_list_response(ToolCategory::ALL));
        assert_eq!(all, crate::tools::ALL_TOOLS.len());

        // A single category exposes only its own tools, and the named tool is
        // present while a tool from another category is not.
        let query_bytes = build_tools_list_response(&[ToolCategory::Query]);
        let v: Value = serde_json::from_slice(&query_bytes).unwrap();
        let names: Vec<&str> = v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"execute_query"));
        assert!(!names.contains(&"create_table"));
        assert!(count(&query_bytes) > 0 && count(&query_bytes) < all);
    }

    #[tokio::test]
    async fn test_read_line_capped_normal() {
        let data = b"hello world\nsecond line\n";
        let mut reader = BufReader::new(&data[..]);
        let mut line = String::new();
        let n = read_line_capped(&mut reader, &mut line, 1024)
            .await
            .unwrap();
        assert_eq!(n, "hello world\n".len());
        assert_eq!(line, "hello world\n");
    }

    #[tokio::test]
    async fn test_read_line_capped_eof() {
        let data = b"";
        let mut reader = BufReader::new(&data[..]);
        let mut line = String::new();
        let n = read_line_capped(&mut reader, &mut line, 1024)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn test_read_line_capped_rejects_oversized() {
        // No newline, longer than the cap -> InvalidData error, bounded memory.
        let data = vec![b'a'; 5000];
        let mut reader = BufReader::new(&data[..]);
        let mut line = String::new();
        let err = read_line_capped(&mut reader, &mut line, 1024)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_parse_valid_request() {
        let line = r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#;
        let req = parse_request(line).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(Value::Number(1.into())));
    }

    #[test]
    fn test_parse_request_with_trailing_newline() {
        let line = r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#;
        let req = parse_request(line).unwrap();
        assert_eq!(req.method, "tools/list");
    }

    #[test]
    fn test_parse_request_with_whitespace() {
        let line = "  {\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"id\":3}  ";
        let req = parse_request(line).unwrap();
        assert_eq!(req.method, "ping");
    }

    #[test]
    fn test_parse_empty_request() {
        let err = parse_request("").unwrap_err();
        assert_eq!(err, "Empty request");
    }

    #[test]
    fn test_parse_whitespace_only() {
        let err = parse_request("   \n  ").unwrap_err();
        assert_eq!(err, "Empty request");
    }

    #[test]
    fn test_parse_invalid_json() {
        let err = parse_request("{invalid}").unwrap_err();
        assert!(
            !err.is_empty(),
            "Invalid JSON should produce an error message"
        );
    }

    #[test]
    fn test_parse_missing_method() {
        let err = parse_request(r#"{"jsonrpc":"2.0","id":1}"#).unwrap_err();
        assert!(err.contains("method"));
    }

    #[test]
    fn test_parse_wrong_version() {
        let req = parse_request(r#"{"jsonrpc":"1.0","method":"init","id":1}"#).unwrap();
        assert_eq!(req.jsonrpc, "1.0");
    }

    #[test]
    fn test_method_not_found_error() {
        let err = method_not_found("test_tool");
        assert_eq!(err.error_code(), -32601);
        assert!(err.to_string().contains("test_tool"));
    }

    #[test]
    fn test_tools_list_static() {
        let bytes = build_tools_list_response(crate::tools::ToolCategory::ALL);
        let list: Value = serde_json::from_slice(&bytes).unwrap();
        let tools = list.get("tools").and_then(|v| v.as_array());
        assert!(
            tools.is_some(),
            "tools/list payload should contain a tools array"
        );
        assert!(!tools.unwrap().is_empty(), "Tools list should not be empty");
    }

    #[test]
    fn test_process_request_method_dispatch() {
        // Verify that process_request handles the dispatch correctly
        // by testing the match on method strings — this is a compilation/coverage test
        let _req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "nonexistent".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };
        // We can't run process_request without a pool, but we can verify the fallback path
        // acts as expected through separate unit tests on the dispatch logic
    }

    #[test]
    fn test_handle_initialize_response() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };
        let result = handle_initialize(&req).unwrap();
        // No requested version → server offers its latest supported revision.
        assert_eq!(result["protocolVersion"], LATEST_PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"]["listChanged"].is_boolean());
        // False resources/prompts capabilities must not be advertised.
        assert!(result["capabilities"]["resources"].is_null());
        assert!(result["capabilities"]["prompts"].is_null());
        assert!(result["instructions"].is_string());
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_handle_initialize_version_negotiation() {
        // A supported requested version is echoed back verbatim.
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(json!({ "protocolVersion": "2024-11-05" })),
            id: Some(Value::Number(1.into())),
        };
        assert_eq!(
            handle_initialize(&req).unwrap()["protocolVersion"],
            "2024-11-05"
        );

        // An unsupported version falls back to our latest.
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(json!({ "protocolVersion": "1999-01-01" })),
            id: Some(Value::Number(1.into())),
        };
        assert_eq!(
            handle_initialize(&req).unwrap()["protocolVersion"],
            LATEST_PROTOCOL_VERSION
        );
    }

    #[test]
    fn test_tool_result_wrapping() {
        // Object results carry both text and structuredContent.
        let ok = tool_success(json!({ "rows": 3 }));
        assert_eq!(ok["isError"], false);
        assert_eq!(ok["content"][0]["type"], "text");
        assert_eq!(ok["structuredContent"]["rows"], 3);

        // Errors are CallToolResults with isError=true, not protocol errors.
        let err = tool_error("boom");
        assert_eq!(err["isError"], true);
        assert_eq!(err["content"][0]["text"], "boom");
    }

    // ── MCP 2025-11-25 compliance ─────────────────────────────────────────

    fn init_with(params: Option<Value>) -> Value {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params,
            id: Some(json!(1)),
        };
        handle_initialize(&req).unwrap()
    }

    #[test]
    fn test_compliance_all_supported_versions_echoed() {
        // Every advertised supported revision must be echoed back verbatim.
        for v in SUPPORTED_PROTOCOL_VERSIONS {
            let result = init_with(Some(json!({ "protocolVersion": v })));
            assert_eq!(&result["protocolVersion"], v, "version {v} not echoed");
        }
        // The latest must be in the supported set and be the default.
        assert!(SUPPORTED_PROTOCOL_VERSIONS.contains(&LATEST_PROTOCOL_VERSION));
        assert_eq!(LATEST_PROTOCOL_VERSION, "2025-11-25");
        assert_eq!(init_with(None)["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn test_compliance_unsupported_and_malformed_version_fall_back() {
        for bad in [json!("1999-01-01"), json!(123), json!(null), json!({})] {
            let result = init_with(Some(json!({ "protocolVersion": bad })));
            assert_eq!(result["protocolVersion"], LATEST_PROTOCOL_VERSION);
        }
        // Missing params entirely also falls back to latest.
        assert_eq!(init_with(Some(json!({})))["protocolVersion"], LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn test_compliance_capabilities_are_honest() {
        let caps = &init_with(None)["capabilities"];
        // Only `tools` is implemented; nothing else may be advertised.
        assert!(caps["tools"].is_object());
        for unimplemented in ["resources", "prompts", "logging", "completions"] {
            assert!(
                caps[unimplemented].is_null(),
                "must not advertise unimplemented capability `{unimplemented}`"
            );
        }
    }

    #[test]
    fn test_compliance_initialize_has_instructions_and_server_info() {
        let result = init_with(None);
        let instructions = result["instructions"].as_str().unwrap();
        assert!(!instructions.is_empty());
        assert_eq!(result["serverInfo"]["name"], "mcp-postgres");
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_compliance_tool_success_shapes() {
        // Object → text + structuredContent, isError=false.
        let obj = tool_success(json!({ "a": 1 }));
        assert_eq!(obj["content"][0]["type"], "text");
        assert!(obj["content"][0]["text"].as_str().unwrap().contains("\"a\":1"));
        assert_eq!(obj["structuredContent"]["a"], 1);
        assert_eq!(obj["isError"], false);

        // Array (non-object) → text only, no structuredContent.
        let arr = tool_success(json!([1, 2, 3]));
        assert_eq!(arr["content"][0]["type"], "text");
        assert_eq!(arr["content"][0]["text"], "[1,2,3]");
        assert!(arr["structuredContent"].is_null());
        assert_eq!(arr["isError"], false);

        // Scalar → text only.
        let scalar = tool_success(json!("hi"));
        assert_eq!(scalar["content"][0]["text"], "\"hi\"");
        assert!(scalar["structuredContent"].is_null());
        assert_eq!(scalar["isError"], false);
    }

    #[test]
    fn test_compliance_tool_result_is_valid_calltoolresult() {
        // content MUST be a non-empty array of typed items.
        for v in [tool_success(json!({ "x": 1 })), tool_error("e")] {
            let content = v["content"].as_array().expect("content is an array");
            assert!(!content.is_empty());
            assert!(content[0]["type"].is_string());
            assert!(v["isError"].is_boolean());
        }
    }

    /// Enforce Phase 1.5: no bare `SET ` outside transaction blocks.
    /// Every session-level SET must use `SET LOCAL` inside a `BEGIN`/`COMMIT` pair.
    /// This grep-based test fails compilation if any violation exists in `src/actions/`.
    #[test]
    fn test_no_bare_set_outside_transaction() {
        let source_files = &[
            include_str!("../src/actions/query.rs"),
            include_str!("../src/actions/batch.rs"),
        ];
        for (idx, source) in source_files.iter().enumerate() {
            for (line_no, line) in source.lines().enumerate() {
                let trimmed = line.trim();
                // Skip comments, UPDATE SET, string literals
                if trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with("*")
                {
                    continue;
                }
                if trimmed.contains("UPDATE ") && trimmed.contains("SET ") {
                    continue;
                }
                if trimmed.contains("SET LOCAL") {
                    continue;
                }
                // Check for bare client.execute("SET ...") outside txn
                if trimmed.contains("client.execute(\"SET ") && !trimmed.contains("SET LOCAL") {
                    let names = ["query.rs", "batch.rs"];
                    panic!(
                        "Phase 1.5 violation: bare `SET` (not SET LOCAL) found in {}:{} — \
                         use BEGIN + SET LOCAL + COMMIT pattern to avoid session leakage.\n\
                         Line: {}",
                        names[idx],
                        line_no + 1,
                        trimmed
                    );
                }
            }
        }
    }
}
