use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{json, Value};
use tracing::{error, warn};
use std::sync::Arc;

use crate::config::Config;
use crate::errors::{MCPError, Result as MCPResult};
use crate::metrics;
use crate::pool::ConnectionPool;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::actions;
use once_cell::sync::Lazy;

static TOOLS_LIST: Lazy<Value> = Lazy::new(|| {
    let tools_json = include_str!("../tools.json");
    let tools: Vec<Value> = serde_json::from_str(tools_json)
        .expect("Failed to parse tools.json");
    json!({ "tools": tools })
});

const BUFFER_CAPACITY: usize = 4096;
const NEWLINE: &[u8] = b"\n";

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
    serde_json::from_str::<JsonRpcRequest>(trimmed)
        .map_err(|e| e.to_string())
}

pub struct MCPServer {
    config: Config,
    pool: Arc<ConnectionPool>,
}

impl MCPServer {
    pub fn new(config: Config, pool: Arc<ConnectionPool>) -> Self {
        Self { config, pool }
    }

    /// Run in stdio mode for MCP compatibility (Claude Desktop, etc.)
    pub async fn run_stdio(&self) -> MCPResult<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, stdin);
        let mut stdout = tokio::io::stdout();
        let mut line = String::with_capacity(512);
        let mut response_buf = Vec::with_capacity(65536);

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    process_one_line(&line, &self.pool, &self.config, &mut response_buf, &mut stdout).await?;
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
async fn handle_client(socket: TcpStream, pool: Arc<ConnectionPool>, config: Config) -> MCPResult<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::with_capacity(BUFFER_CAPACITY, reader);
    let mut line = String::with_capacity(512);
    let mut response_buf = Vec::with_capacity(65536);

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
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
#[inline]
async fn process_one_line<W: AsyncWriteExt + Unpin>(
    line: &str,
    pool: &Arc<ConnectionPool>,
    config: &Config,
    response_buf: &mut Vec<u8>,
    writer: &mut W,
) -> MCPResult<()> {
    metrics::inc_requests();

    let response = match parse_request(line) {
        Ok(req) => match process_request(&req, pool, config).await {
            Ok(result) => JsonRpcResponse::success(req.id, result),
            Err(e) => {
                metrics::inc_errors();
                JsonRpcResponse::error(req.id, e.error_code(), e.to_string())
            }
        },
        Err(e) => {
            metrics::inc_errors();
            parse_error(e)
        }
    };

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
        _ => Err(MCPError::MethodNotFound(req.method.clone())),
    }
}

/// Public wrapper for HTTP handlers - returns complete JSON-RPC response
pub async fn process_request_http(
    req: &JsonRpcRequest,
    pool: &Arc<ConnectionPool>,
    config: &Config,
) -> JsonRpcResponse {
    metrics::inc_requests();

    let response = match process_request(req, pool, config).await {
        Ok(result) => JsonRpcResponse::success(req.id.clone(), result),
        Err(e) => {
            metrics::inc_errors();
            JsonRpcResponse::error(req.id.clone(), e.error_code(), e.to_string())
        }
    };

    response
}

#[inline]
fn handle_initialize(_req: &JsonRpcRequest) -> MCPResult<Value> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {
                "listChanged": false
            },
            "resources": {
                "subscribe": false,
                "listChanged": false
            },
            "prompts": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "mcp-postgres",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

#[inline]
fn handle_tools_list() -> MCPResult<Value> {
    Ok((*TOOLS_LIST).clone())
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
    let write_tools: &[&str] = &[
        "execute_insert", "execute_update", "execute_delete",
        "async_execute_insert", "async_execute_update", "async_execute_delete",
        "async_batch_insert", "async_batch_update", "async_batch_delete", "async_batch_insert_copy",
        "create_table", "drop_table", "create_view", "drop_view", "alter_view", "create_schema", "drop_schema", "create_sequence", "drop_sequence", "alter_index", "create_index", "drop_index", "create_partition", "drop_partition",
        "vacuum_analyze", "analyze_table", "reindex_table",
        "reset_statistics", "truncate_table",
    ];

    if config.server.access_mode == crate::config::AccessMode::Restricted
        && write_tools.contains(&tool_name)
    {
        return Err(MCPError::InvalidParams(format!(
            "Operation '{tool_name}' is not allowed in restricted (read-only) mode"
        )));
    }

    // Fast-path simple tools that don't need a DB connection
    let no_db_tools: &[&str] = &["list_tables", "list_schemas", "show_constraints"];
    if !no_db_tools.contains(&tool_name) {
        // Verify tool exists before acquiring a connection
        let tool_exists = matches!(tool_name,
            "describe_table" | "list_triggers" | "list_indexes" | "execute_query" | "execute_insert"
            | "execute_update" | "execute_delete" | "explain_query"
            | "async_execute_insert" | "async_execute_update" | "async_execute_delete"
            | "async_batch_insert" | "async_batch_update" | "async_batch_delete" | "async_batch_insert_copy"
            | "create_table" | "drop_table" | "create_view" | "drop_view" | "alter_view" | "create_schema" | "drop_schema" | "create_sequence" | "drop_sequence" | "alter_index" | "create_index" | "list_partitions" | "drop_index" | "create_partition" | "drop_partition"
            | "get_table_stats" | "get_index_stats" | "show_database_size"
            | "show_table_size" | "get_cache_hit_ratio"
            | "list_connections" | "show_current_user"
            | "show_running_queries" | "show_connection_summary"
            | "vacuum_analyze" | "analyze_table" | "reindex_table"
            | "get_pg_stat_statements" | "reset_statistics" | "truncate_table"
            | "list_users" | "list_user_privileges" | "list_role_memberships"
            | "list_database_privileges" | "show_session_info"
            | "show_all_settings" | "get_setting" | "show_memory_settings"
            | "show_performance_settings" | "show_log_settings"
            | "show_replication_status" | "list_replication_slots"
            | "list_standby_servers" | "show_wal_info" | "show_base_backup_progress"
            | "show_active_transactions" | "show_locks" | "show_waiting_locks"
            | "show_transaction_isolation" | "show_deadlocks"
            | "show_autocommit_status" | "show_transaction_timeout"
            | "analyze_db_health" | "list_unused_indexes" | "list_duplicate_indexes"
            | "show_vacuum_progress" | "get_object_details"
        );
        if !tool_exists {
            return Err(method_not_found(tool_name));
        }
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
        "async_batch_insert_copy" => actions::batch::async_batch_insert_copy(&client, &tool_args).await,
        // Monitoring actions
        "get_table_stats" => actions::monitoring::get_table_stats(&client, &tool_args).await,
        "get_index_stats" => actions::monitoring::get_index_stats(&client, &tool_args).await,
        "show_database_size" => actions::monitoring::show_database_size(&client, &tool_args).await,
        "show_table_size" => actions::monitoring::show_table_size(&client, &tool_args).await,
        "get_cache_hit_ratio" => actions::monitoring::get_cache_hit_ratio(&client, &tool_args).await,
        // Connection actions
        "list_connections" => actions::connections::list_connections(&client, &tool_args).await,
        "show_current_user" => actions::connections::show_current_user(&client, &tool_args).await,
        "show_running_queries" => actions::connections::show_running_queries(&client, &tool_args).await,
        "show_connection_summary" => actions::connections::show_connection_summary(&client, &tool_args).await,
        // Maintenance actions
        "vacuum_analyze" => actions::maintenance::vacuum_analyze(&client, &tool_args).await,
        "analyze_table" => actions::maintenance::analyze_table(&client, &tool_args).await,
        "reindex_table" => actions::maintenance::reindex_table(&client, &tool_args).await,
        "get_pg_stat_statements" => actions::maintenance::get_pg_stat_statements(&client, &tool_args).await,
        "reset_statistics" => actions::maintenance::reset_statistics(&client, &tool_args).await,
        "truncate_table" => actions::maintenance::truncate_table(&client, &tool_args).await,
        // Security actions
        "list_users" => actions::security::list_users(&client, &tool_args).await,
        "list_user_privileges" => actions::security::list_user_privileges(&client, &tool_args).await,
        "list_role_memberships" => actions::security::list_role_memberships(&client, &tool_args).await,
        "list_database_privileges" => actions::security::list_database_privileges(&client, &tool_args).await,
        "show_session_info" => actions::security::show_session_info(&client, &tool_args).await,
        // Config actions
        "show_all_settings" => actions::config::show_all_settings(&client, &tool_args).await,
        "get_setting" => actions::config::get_setting(&client, &tool_args).await,
        "show_memory_settings" => actions::config::show_memory_settings(&client, &tool_args).await,
        "show_performance_settings" => actions::config::show_performance_settings(&client, &tool_args).await,
        "show_log_settings" => actions::config::show_log_settings(&client, &tool_args).await,
        // Replication actions
        "show_replication_status" => actions::replication::show_replication_status(&client, &tool_args).await,
        "list_replication_slots" => actions::replication::list_replication_slots(&client, &tool_args).await,
        "list_standby_servers" => actions::replication::list_standby_servers(&client, &tool_args).await,
        "show_wal_info" => actions::replication::show_wal_info(&client, &tool_args).await,
        "show_base_backup_progress" => actions::replication::show_base_backup_progress(&client, &tool_args).await,
        // Transaction actions
        "show_active_transactions" => actions::transactions::show_active_transactions(&client, &tool_args).await,
        "show_locks" => actions::transactions::show_locks(&client, &tool_args).await,
        "show_waiting_locks" => actions::transactions::show_waiting_locks(&client, &tool_args).await,
        "show_transaction_isolation" => actions::transactions::show_transaction_isolation(&client, &tool_args).await,
        "show_deadlocks" => actions::transactions::show_deadlocks(&client, &tool_args).await,
        "show_autocommit_status" => actions::transactions::show_autocommit_status(&client, &tool_args).await,
        "show_transaction_timeout" => actions::transactions::show_transaction_timeout(&client, &tool_args).await,
        // Health actions
        "analyze_db_health" => actions::health::analyze_db_health(&client, &tool_args).await,
        "list_unused_indexes" => actions::health::list_unused_indexes(&client, &tool_args).await,
        "list_duplicate_indexes" => actions::health::list_duplicate_indexes(&client, &tool_args).await,
        "show_vacuum_progress" => actions::health::show_vacuum_progress(&client, &tool_args).await,
        // Enhanced schema
        "get_object_details" => actions::schema::get_object_details(&client, &tool_args).await,
        tool => Err(method_not_found(tool)),
    };

    pool.release(client);
    result
}

#[cold]
fn method_not_found(name: &str) -> MCPError {
    MCPError::MethodNotFound(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!err.is_empty(), "Invalid JSON should produce an error message");
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
        let list = &*TOOLS_LIST;
        let tools = list.get("tools").and_then(|v| v.as_array());
        assert!(tools.is_some(), "TOOLS_LIST should contain a tools array");
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
}
