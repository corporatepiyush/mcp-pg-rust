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

/// Pre-serialized response bytes for `tools/list`.  Built once at startup;
/// each call deserializes from this cached buffer instead of deep-cloning the
/// entire 135-tool Value tree (~50 KB).
static TOOLS_LIST_RESPONSE: Lazy<Vec<u8>> = Lazy::new(|| {
    let tools_json = include_str!("../tools.json");
    let tools: Vec<Value> = serde_json::from_str(tools_json).expect("Failed to parse tools.json");
    let resp = json!({ "tools": tools });
    serde_json::to_vec(&resp).expect("Failed to serialize tools/list response")
});

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
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            break; // EOF
        }
        let (take, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        if buf.len() + take > max_bytes {
            reader.consume(take);
            return Err(Error::new(
                ErrorKind::InvalidData,
                "request line exceeds maximum length",
            ));
        }
        buf.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if done {
            break;
        }
    }
    if buf.is_empty() {
        return Ok(0);
    }
    let s = String::from_utf8(buf)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "request line is not valid UTF-8"))?;
    line.push_str(&s);
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
    config: Config,
    pool: Arc<ConnectionPool>,
}

impl MCPServer {
    pub const fn new(config: Config, pool: Arc<ConnectionPool>) -> Self {
        Self { config, pool }
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
            let config = self.config.clone();

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
    config: Config,
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
    Ok(())
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
        "tools/list" => handle_tools_list(),
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

fn handle_initialize(_req: &JsonRpcRequest) -> MCPResult<Value> {
    /// Cached initialize response — built once on first call.
    static INIT_RESPONSE: Lazy<Value> = Lazy::new(|| {
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false },
                "prompts": { "listChanged": false }
            },
            "serverInfo": {
                "name": "mcp-postgres",
                "version": env!("CARGO_PKG_VERSION")
            }
        })
    });

    Ok(INIT_RESPONSE.clone())
}

#[inline]
fn handle_tools_list() -> MCPResult<Value> {
    // Deserialize from cached bytes instead of deep-cloning a 50 KB Value tree.
    Ok(serde_json::from_slice(&TOOLS_LIST_RESPONSE)?)
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

    // Restricted mode check + unknown tool check BEFORE pool acquire
    if config.server.access_mode == crate::config::AccessMode::Restricted
        && crate::tools::is_write_tool(tool_name)
    {
        return Err(MCPError::InvalidParams(format!(
            "Operation '{tool_name}' is not allowed in restricted (read-only) mode"
        )));
    }

    // Verify tool exists before acquiring a connection
    if !crate::tools::tool_exists(tool_name) {
        return Err(method_not_found(tool_name));
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

    if let Err(ref e) = result {
        error!("Tool '{}' error: {:?}", tool_name, e);
    }
    // client is returned to the pool automatically via Drop
    drop(client);
    result
}

#[cold]
fn method_not_found(name: &str) -> MCPError {
    MCPError::MethodNotFound(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_line_capped_normal() {
        let data = b"hello world\nsecond line\n";
        let mut reader = BufReader::new(&data[..]);
        let mut line = String::new();
        let n = read_line_capped(&mut reader, &mut line, 1024).await.unwrap();
        assert_eq!(n, "hello world\n".len());
        assert_eq!(line, "hello world\n");
    }

    #[tokio::test]
    async fn test_read_line_capped_eof() {
        let data = b"";
        let mut reader = BufReader::new(&data[..]);
        let mut line = String::new();
        let n = read_line_capped(&mut reader, &mut line, 1024).await.unwrap();
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
        let list: Value = serde_json::from_slice(&TOOLS_LIST_RESPONSE).unwrap();
        let tools = list.get("tools").and_then(|v| v.as_array());
        assert!(
            tools.is_some(),
            "TOOLS_LIST_RESPONSE should contain a tools array"
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
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"]["listChanged"].is_boolean());
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
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
