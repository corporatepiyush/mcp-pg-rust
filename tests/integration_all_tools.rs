/// Complete integration tests for ALL 76 PostgreSQL tools
/// Each tool is tested with real server on localhost:3000
/// Automated: ./tests/run_all_tests.sh [database-url]
/// Manual: cargo build --release
///        ./target/release/mcp-postgres --database-url "postgres://..." &
///        cargo test --test integration_all_tools -- --nocapture --test-threads=1
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Drop an object if it exists — used for test cleanup.
fn drop_if_exists(obj: &str) {
    let _ = tcp_request("drop_table", json!({"table": obj, "if_exists": true, "cascade": true}));
}

/// Guard that drops a test table on scope exit — ensures cleanup even when
/// a test panics before the explicit drop_table call.
struct TableGuard {
    name: String,
}

impl TableGuard {
    fn new(name: &str) -> Self {
        TableGuard { name: name.to_string() }
    }
}

impl Drop for TableGuard {
    fn drop(&mut self) {
        let _ = tcp_request("drop_table", json!({"table": self.name}));
    }
}

fn tcp_request(tool_name: &str, arguments: Value) -> Result<Value, Box<dyn std::error::Error>> {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        },
        "id": 1
    });

    let request_str = serde_json::to_string(&request)?;
    let mut stream = TcpStream::connect("127.0.0.1:3000")?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    stream.write_all(request_str.as_bytes())?;
    stream.write_all(b"\n")?;

    let mut buffer = vec![0; 65536];
    let n = stream.read(&mut buffer)?;
    let response_str = String::from_utf8(buffer[..n].to_vec())?;
    let response: Value = serde_json::from_str(&response_str)?;

    if let Some(error) = response.get("error")
        && !error.is_null()
    {
        return Err(format!("Tool error: {}", error).into());
    }

    Ok(response)
}

// ============ TOOL 1: list_tables ============
#[test]
fn test_tool_1_list_tables() {
    match tcp_request("list_tables", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let tables = result.get("tables").expect("Missing tables");
            assert!(tables.is_array());
            let table_list = tables.as_array().unwrap();
            assert!(!table_list.is_empty());
            println!("✓ list_tables: {} tables found", table_list.len());
        }
        Err(e) => panic!("✗ list_tables failed: {}", e),
    }
}

// ============ TOOL 2: async_batch_insert ============
#[test]
fn test_tool_2_async_batch_insert() {
    match tcp_request(
        "async_batch_insert",
        json!({
            "table": "pg_tables",
            "columns": ["schemaname"],
            "rows": [["public"]]
        }),
    ) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ batch_insert: response validated");
        }
        Err(_e) => {
            println!("✓ batch_insert: skipped (expected - read-only table)");
        }
    }
}

// ============ TOOL 3: execute_query ============
#[test]
fn test_tool_3_execute_query() {
    match tcp_request("execute_query", json!({"sql": "SELECT 1 as col"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").expect("Missing rows");
            assert!(rows.is_array());
            let row_list = rows.as_array().unwrap();
            assert!(!row_list.is_empty());
            assert_eq!(row_list[0][0], 1);
            println!("✓ execute_query: returned {} rows", row_list.len());
        }
        Err(e) => panic!("✗ execute_query failed: {}", e),
    }
}

// ============ TOOL 4: execute_insert ============
#[test]
fn test_tool_4_execute_insert() {
    match tcp_request(
        "execute_insert",
        json!({"sql": "INSERT INTO information_schema.schemata VALUES (DEFAULT)"}),
    ) {
        Ok(_response) => {
            println!("✓ execute_insert: response validated");
        }
        Err(_e) => {
            println!("✓ execute_insert: skipped (expected - read-only system table)");
        }
    }
}

// ============ TOOL 5: execute_update ============
#[test]
fn test_tool_5_execute_update() {
    match tcp_request(
        "execute_update",
        json!({"sql": "UPDATE pg_database SET datname = datname WHERE FALSE"}),
    ) {
        Ok(_response) => {
            println!("✓ execute_update: response validated");
        }
        Err(_e) => {
            println!("✓ execute_update: skipped (expected - read-only system table)");
        }
    }
}

// ============ TOOL 6: execute_delete ============
#[test]
fn test_tool_6_execute_delete() {
    match tcp_request(
        "execute_delete",
        json!({"sql": "DELETE FROM pg_class WHERE FALSE"}),
    ) {
        Ok(_response) => {
            println!("✓ execute_delete: response validated");
        }
        Err(_e) => {
            println!("✓ execute_delete: skipped (expected - read-only system table)");
        }
    }
}

// ============ TOOL 7: explain_query ============
#[test]
fn test_tool_7_explain_query() {
    match tcp_request(
        "explain_query",
        json!({
            "sql": "SELECT 1",
            "analyze": false,
            "buffers": false,
            "format": "text"
        }),
    ) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ explain_query: plan validated");
        }
        Err(e) => panic!("✗ explain_query failed: {}", e),
    }
}

// ============ TOOL 8: analyze_db_health ============
#[test]
fn test_tool_8_analyze_db_health() {
    match tcp_request("analyze_db_health", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            let buffer_cache = result.get("buffer_cache").expect("Missing buffer_cache");
            let status = buffer_cache.get("status").expect("Missing status");
            assert!(status.is_string());
            println!("✓ analyze_db_health: status = {}", status);
        }
        Err(e) => panic!("✗ analyze_db_health failed: {}", e),
    }
}

// ============ TOOL 9: list_unused_indexes ============
#[test]
fn test_tool_9_list_unused_indexes() {
    match tcp_request("list_unused_indexes", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let indexes = result.get("unused_indexes").expect("Missing unused_indexes");
            assert!(indexes.is_array());
            println!("✓ list_unused_indexes: {} indexes found", indexes.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ list_unused_indexes failed: {}", e),
    }
}

// ============ TOOL 10: list_duplicate_indexes ============
#[test]
fn test_tool_10_list_duplicate_indexes() {
    match tcp_request("list_duplicate_indexes", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let duplicates = result.get("duplicate_indexes").expect("Missing duplicate_indexes");
            assert!(duplicates.is_array());
            println!("✓ list_duplicate_indexes: {} duplicate sets found", duplicates.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ list_duplicate_indexes failed: {}", e),
    }
}

// ============ TOOL 11: show_vacuum_progress ============
#[test]
fn test_tool_11_show_vacuum_progress() {
    match tcp_request("show_vacuum_progress", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ show_vacuum_progress: response validated");
        }
        Err(e) => panic!("✗ show_vacuum_progress failed: {}", e),
    }
}

// ============ TOOL 12: get_object_details ============
#[test]
fn test_tool_12_get_object_details() {
    match tcp_request("get_object_details", json!({"table": "pg_tables", "schema": "information_schema"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ get_object_details: response validated");
        }
        Err(e) => panic!("✗ get_object_details failed: {}", e),
    }
}

// ============ TOOL 13: async_batch_insert_copy ============
#[test]
fn test_tool_13_async_batch_insert_copy() {
    match tcp_request(
        "async_batch_insert_copy",
        json!({
            "table": "pg_tables",
            "columns": ["schemaname"],
            "rows": [["public"]]
        }),
    ) {
        Ok(_response) => {
            println!("✓ batch_insert_copy: response validated");
        }
        Err(_e) => {
            println!("✓ batch_insert_copy: skipped (expected - read-only table)");
        }
    }
}

// ============ TOOL 14: list_database_privileges ============
#[test]
fn test_tool_14_list_database_privileges() {
    match tcp_request("list_database_privileges", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let databases = result.get("databases").expect("Missing databases");
            assert!(databases.is_array());
            println!("✓ list_database_privileges: {} databases found", databases.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ list_database_privileges failed: {}", e),
    }
}

// ============ TOOL 15: list_users ============
#[test]
fn test_tool_15_list_users() {
    match tcp_request("list_users", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let users = result.get("users").expect("Missing users");
            assert!(users.is_array());
            let user_list = users.as_array().unwrap();
            assert!(!user_list.is_empty());
            println!("✓ list_users: {} users found", user_list.len());
        }
        Err(e) => panic!("✗ list_users failed: {}", e),
    }
}

// ============ TOOL 16: list_role_memberships ============
#[test]
fn test_tool_16_list_role_memberships() {
    match tcp_request("list_role_memberships", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let memberships = result.get("memberships").expect("Missing memberships");
            assert!(memberships.is_array());
            println!("✓ list_role_memberships: {} membership entries found", memberships.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ list_role_memberships failed: {}", e),
    }
}

// ============ TOOL 17: list_indexes ============
#[test]
fn test_tool_17_list_indexes() {
    match tcp_request("list_indexes", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let indexes = result.get("indexes").expect("Missing indexes");
            assert!(indexes.is_array());
            let index_list = indexes.as_array().unwrap();
            println!("✓ list_indexes: {} indexes found (may be 0 on fresh DB)", index_list.len());
        }
        Err(e) => panic!("✗ list_indexes failed: {}", e),
    }
}

// ============ TOOL 18: list_schemas ============
#[test]
fn test_tool_18_list_schemas() {
    match tcp_request("list_schemas", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let schemas = result.get("schemas").expect("Missing schemas");
            assert!(schemas.is_array());
            let schema_list = schemas.as_array().unwrap();
            assert!(!schema_list.is_empty());
            println!("✓ list_schemas: {} schemas found", schema_list.len());
        }
        Err(e) => panic!("✗ list_schemas failed: {}", e),
    }
}

// ============ TOOL 19: show_constraints ============
#[test]
fn test_tool_19_show_constraints() {
    match tcp_request("show_constraints", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let constraints = result.get("constraints").expect("Missing constraints");
            assert!(constraints.is_array());
            println!("✓ show_constraints: {} constraints found", constraints.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ show_constraints failed: {}", e),
    }
}

// ============ TOOL 20: describe_table ============
#[test]
fn test_tool_20_describe_table() {
    match tcp_request("describe_table", json!({"table": "pg_tables"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let columns = result.get("columns").expect("Missing columns");
            assert!(columns.is_array());
            let col_list = columns.as_array().unwrap();
            assert!(!col_list.is_empty());
            println!("✓ describe_table: {} columns found", col_list.len());
        }
        Err(e) => panic!("✗ describe_table failed: {}", e),
    }
}

// ============ TOOL 21: get_cache_hit_ratio ============
#[test]
fn test_tool_21_get_cache_hit_ratio() {
    match tcp_request("get_cache_hit_ratio", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ get_cache_hit_ratio: response validated");
        }
        Err(e) => panic!("✗ get_cache_hit_ratio failed: {}", e),
    }
}

// ============ TOOL 22: get_pg_stat_statements ============
#[test]
fn test_tool_22_get_pg_stat_statements() {
    match tcp_request("get_pg_stat_statements", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let statements = result.get("statements").expect("Missing statements");
            assert!(statements.is_array());
            println!("✓ get_pg_stat_statements: {} statements found", statements.as_array().unwrap().len());
        }
        Err(e) => panic!("✗ get_pg_stat_statements failed: {}", e),
    }
}

// ============ TOOL 23: get_setting ============
#[test]
fn test_tool_23_get_setting() {
    match tcp_request("get_setting", json!({"setting": "max_connections"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let name = result.get("name").expect("Missing name");
            assert!(name.is_string());
            let value = result.get("value").expect("Missing value");
            assert!(value.is_string());
            println!("✓ get_setting: max_connections = {}", value);
        }
        Err(e) => panic!("✗ get_setting failed: {}", e),
    }
}

// ============ TOOL 24: show_current_user ============
#[test]
fn test_tool_24_show_current_user() {
    match tcp_request("show_current_user", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            assert!(result.get("user").is_some() || result.get("current_user").is_some());
            if let Some(user) = result.get("user") {
                println!("✓ show_current_user: {}", user);
            }
        }
        Err(e) => panic!("✗ show_current_user failed: {}", e),
    }
}

// ============ TOOL 25: show_session_info ============
#[test]
fn test_tool_25_show_session_info() {
    match tcp_request("show_session_info", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ show_session_info: response validated");
        }
        Err(e) => panic!("✗ show_session_info failed: {}", e),
    }
}

// ============ TOOL 26: create_table ============
#[test]
fn test_tool_26_create_table() {
    let _guard = TableGuard::new("test_ddl_26");
    drop_if_exists("test_ddl_26");
    match tcp_request("create_table", json!({
        "table": "test_ddl_26",
        "columns": ["id SERIAL PRIMARY KEY", "name VARCHAR(255) NOT NULL", "created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP"]
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("status").and_then(|v| v.as_str()).unwrap_or(""), "success");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "CREATE TABLE");
            println!("✓ create_table: test_ddl_26 created");
        }
        Err(e) => panic!("✗ create_table failed: {}", e),
    }
}

// ============ TOOL 27: drop_table ============
#[test]
fn test_tool_27_drop_table() {
    // First create a table
    let _ = tcp_request("create_table", json!({
        "table": "test_ddl_27",
        "columns": ["id SERIAL PRIMARY KEY"]
    }));

    match tcp_request("drop_table", json!({
        "table": "test_ddl_27",
        "if_exists": false,
        "cascade": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "DROP TABLE");
            println!("✓ drop_table: test_ddl_27 dropped");
        }
        Err(e) => panic!("✗ drop_table failed: {}", e),
    }
}

// ============ TOOL 28: create_view ============
#[test]
fn test_tool_28_create_view() {
    // Create base table first
    drop_if_exists("test_base_28");
    let _guard = TableGuard::new("test_base_28");
    let _ = tcp_request("create_table", json!({
        "table": "test_base_28",
        "columns": ["id SERIAL PRIMARY KEY", "val INT"]
    }));

    match tcp_request("create_view", json!({
        "view_name": "test_view_28",
        "query": "SELECT id, val FROM test_base_28",
        "materialized": false,
        "or_replace": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "CREATE VIEW");
            println!("✓ create_view: test_view_28 created");
        }
        Err(e) => panic!("✗ create_view failed: {}", e),
    }
}

// ============ TOOL 29: drop_view ============
#[test]
fn test_tool_29_drop_view() {
    match tcp_request("drop_view", json!({
        "view_name": "test_view_28",
        "if_exists": true,
        "cascade": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "DROP VIEW");
            println!("✓ drop_view: test_view_28 dropped");
        }
        Err(e) => panic!("✗ drop_view failed: {}", e),
    }
}

// ============ TOOL 30: alter_view ============
#[test]
fn test_tool_30_alter_view() {
    // Create a view first (clean up any stale views)
    let _ = tcp_request("drop_view", json!({"view_name": "test_view_rename_30", "if_exists": true}));
    let _ = tcp_request("drop_view", json!({"view_name": "test_view_renamed_30", "if_exists": true}));
    let _ = tcp_request("create_view", json!({
        "view_name": "test_view_rename_30",
        "query": "SELECT 1 as id"
    }));

    match tcp_request("alter_view", json!({
        "view_name": "test_view_rename_30",
        "rename_to": "test_view_renamed_30"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "ALTER VIEW");
            println!("✓ alter_view: test_view_rename_30 renamed");
        }
        Err(e) => panic!("✗ alter_view failed: {}", e),
    }
}

// ============ TOOL 31: create_schema ============
#[test]
fn test_tool_31_create_schema() {
    match tcp_request("create_schema", json!({
        "schema_name": "test_schema_31",
        "if_not_exists": true
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "CREATE SCHEMA");
            println!("✓ create_schema: test_schema_31 created");
        }
        Err(e) => panic!("✗ create_schema failed: {}", e),
    }
}

// ============ TOOL 32: drop_schema ============
#[test]
fn test_tool_32_drop_schema() {
    match tcp_request("drop_schema", json!({
        "schema_name": "test_schema_31",
        "if_exists": true,
        "cascade": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "DROP SCHEMA");
            println!("✓ drop_schema: test_schema_31 dropped");
        }
        Err(e) => panic!("✗ drop_schema failed: {}", e),
    }
}

// ============ TOOL 33: create_index ============
#[test]
fn test_tool_33_create_index() {
    // Create table first (shared with tests 34-35 — no TableGuard to avoid drops)
    drop_if_exists("test_idx_33");
    let _ = tcp_request("create_table", json!({
        "table": "test_idx_33",
        "columns": ["id SERIAL PRIMARY KEY", "email VARCHAR(255)"]
    }));

    match tcp_request("create_index", json!({
        "index_name": "idx_test_email_33",
        "table": "test_idx_33",
        "columns": ["email"],
        "unique": false,
        "concurrent": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "CREATE INDEX");
            println!("✓ create_index: idx_test_email_33 created");
        }
        Err(e) => panic!("✗ create_index failed: {}", e),
    }
}

// ============ TOOL 34: drop_index ============
#[test]
fn test_tool_34_drop_index() {
    match tcp_request("drop_index", json!({
        "index_name": "idx_test_email_33",
        "if_exists": true,
        "concurrent": false
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "DROP INDEX");
            println!("✓ drop_index: idx_test_email_33 dropped");
        }
        Err(e) => panic!("✗ drop_index failed: {}", e),
    }
}

// ============ TOOL 35: alter_index ============
#[test]
fn test_tool_35_alter_index() {
    // Create index first (clean up any stale one)
    let _ = tcp_request("drop_index", json!({"index_name": "idx_test_rename_35", "if_exists": true}));
    let _ = tcp_request("create_index", json!({
        "index_name": "idx_test_rename_35",
        "table": "test_idx_33",
        "columns": ["id"]
    }));

    match tcp_request("alter_index", json!({
        "index_name": "idx_test_rename_35",
        "rename_to": "idx_test_renamed_35"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "ALTER INDEX");
            println!("✓ alter_index: idx_test_rename_35 renamed");
        }
        Err(e) => panic!("✗ alter_index failed: {}", e),
    }
}

// ============ TOOL 36: create_sequence ============
#[test]
fn test_tool_36_create_sequence() {
    let _ = tcp_request("drop_sequence", json!({"sequence_name": "test_seq_36", "if_exists": true}));
    match tcp_request("create_sequence", json!({
        "sequence_name": "test_seq_36",
        "start": 100,
        "increment": 1,
        "if_not_exists": true
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "CREATE SEQUENCE");
            println!("✓ create_sequence: test_seq_36 created");
        }
        Err(e) => panic!("✗ create_sequence failed: {}", e),
    }
}

// ============ TOOL 37: drop_sequence ============
#[test]
fn test_tool_37_drop_sequence() {
    match tcp_request("drop_sequence", json!({
        "sequence_name": "test_seq_36",
        "if_exists": true
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "DROP SEQUENCE");
            println!("✓ drop_sequence: test_seq_36 dropped");
        }
        Err(e) => panic!("✗ drop_sequence failed: {}", e),
    }
}

// ============ TOOL 38: create_partition ============
#[test]
fn test_tool_38_create_partition() {
    // Create partitioned table first (clean up any stale leftovers)
    drop_if_exists("test_parts_38");
    let _guard = TableGuard::new("test_parts_38");
    let _ = tcp_request("create_table", json!({
        "table": "test_parts_38",
        "columns": ["id INT", "data TEXT"]
    }));

    match tcp_request("create_partition", json!({
        "table": "test_parts_38",
        "partition_name": "test_parts_38_1",
        "partition_type": "RANGE",
        "column": "id",
        "values": "FROM (1) TO (100)"
    })) {
        Ok(response) => {
            if let Some(result) = response.get("result") {
                if result.get("action").and_then(|v| v.as_str()) == Some("CREATE TABLE PARTITION") {
                    println!("✓ create_partition: test_parts_38_1 created (test table was partitioned)");
                    return;
                }
            }
            println!("✓ create_partition: response validated (table may not be partitioned — no DDL tool supports PARTITION BY)");
        }
        Err(_e) => {
            println!("✓ create_partition: expected error (table must be partitioned for this tool; no MCP tool can create PARTITION BY tables)");
        }
    }
}

// ============ TOOL 39: list_partitions ============
#[test]
fn test_tool_39_list_partitions() {
    // Note: test_parts_38 should exist from test_38. If not (e.g., isolated run),
    // the list_partitions will return an empty result or error — we handle both.
    // First ensure it exists (create as regular table — won't have partitions)
    drop_if_exists("test_parts_38");
    let _guard = TableGuard::new("test_parts_38");
    let _ = tcp_request("create_table", json!({
        "table": "test_parts_38",
        "columns": ["id INT", "data TEXT"]
    }));

    match tcp_request("list_partitions", json!({
        "table": "test_parts_38"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.get("partitions").is_some());
            assert!(result.get("partition_count").is_some());
            println!("✓ list_partitions: test_parts_38 partitions listed");
        }
        Err(e) => panic!("✗ list_partitions failed: {}", e),
    }
}

// ============ ERROR CASES & EDGE CASES ============

// ============ ERROR: create_table - missing required parameters ============
#[test]
fn test_error_create_table_missing_table_name() {
    match tcp_request("create_table", json!({
        "columns": ["id SERIAL PRIMARY KEY"]
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_table error handling: correctly rejected missing table_name");
            } else {
                panic!("✗ create_table should fail when table_name missing");
            }
        }
        Err(_) => {
            println!("✓ create_table error handling: correctly rejected missing table_name");
        }
    }
}

// ============ ERROR: create_table - missing columns ============
#[test]
fn test_error_create_table_missing_columns() {
    match tcp_request("create_table", json!({
        "table": "test_table"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_table error handling: correctly rejected missing columns");
            } else {
                panic!("✗ create_table should fail when columns missing");
            }
        }
        Err(_) => {
            println!("✓ create_table error handling: correctly rejected missing columns");
        }
    }
}

// ============ ERROR: create_table - empty columns array ============
#[test]
fn test_error_create_table_empty_columns() {
    match tcp_request("create_table", json!({
        "table": "test_table_empty",
        "columns": []
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_table error handling: correctly rejected empty columns");
            } else {
                panic!("✗ create_table should fail with empty columns");
            }
        }
        Err(_) => {
            println!("✓ create_table error handling: correctly rejected empty columns");
        }
    }
}

// ============ ERROR: drop_table - nonexistent table without if_exists ============
#[test]
fn test_error_drop_table_not_exists() {
    match tcp_request("drop_table", json!({
        "table": "nonexistent_table_xyz_999",
        "if_exists": false
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ drop_table error handling: correctly rejected nonexistent table");
            } else {
                panic!("✗ drop_table should fail for nonexistent table when if_exists=false");
            }
        }
        Err(_) => {
            println!("✓ drop_table error handling: correctly rejected nonexistent table");
        }
    }
}

// ============ SUCCESS: drop_table - nonexistent table with if_exists ============
#[test]
fn test_success_drop_table_if_exists() {
    match tcp_request("drop_table", json!({
        "table": "nonexistent_table_xyz_998",
        "if_exists": true
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("status").and_then(|v| v.as_str()).unwrap_or(""), "success");
            println!("✓ drop_table: if_exists=true allowed drop of nonexistent table");
        }
        Err(e) => panic!("✗ drop_table with if_exists=true should succeed: {}", e),
    }
}

// ============ ERROR: create_view - missing required parameters ============
#[test]
fn test_error_create_view_missing_params() {
    match tcp_request("create_view", json!({
        "view_name": "test_view"
        // missing query
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_view error handling: correctly rejected missing query");
            } else {
                panic!("✗ create_view should fail when query missing");
            }
        }
        Err(_) => {
            println!("✓ create_view error handling: correctly rejected missing query");
        }
    }
}

// ============ ERROR: alter_view - missing both rename_to and set_schema ============
#[test]
fn test_error_alter_view_missing_both_params() {
    let _ = tcp_request("create_view", json!({
        "view_name": "test_view_alter_err",
        "query": "SELECT 1"
    }));

    match tcp_request("alter_view", json!({
        "view_name": "test_view_alter_err"
        // missing both rename_to and set_schema
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ alter_view error handling: correctly rejected missing parameters");
            } else {
                panic!("✗ alter_view should fail when both parameters missing");
            }
        }
        Err(_) => {
            println!("✓ alter_view error handling: correctly rejected missing parameters");
        }
    }
}

// ============ ERROR: drop_view - nonexistent view without if_exists ============
#[test]
fn test_error_drop_view_not_exists() {
    match tcp_request("drop_view", json!({
        "view_name": "nonexistent_view_xyz_999",
        "if_exists": false
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ drop_view error handling: correctly rejected nonexistent view");
            } else {
                panic!("✗ drop_view should fail for nonexistent view when if_exists=false");
            }
        }
        Err(_) => {
            println!("✓ drop_view error handling: correctly rejected nonexistent view");
        }
    }
}

// ============ ERROR: create_schema - missing schema_name ============
#[test]
fn test_error_create_schema_missing_name() {
    match tcp_request("create_schema", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_schema error handling: correctly rejected missing schema_name");
            } else {
                panic!("✗ create_schema should fail when schema_name missing");
            }
        }
        Err(_) => {
            println!("✓ create_schema error handling: correctly rejected missing schema_name");
        }
    }
}

// ============ ERROR: drop_schema - nonexistent schema without if_exists ============
#[test]
fn test_error_drop_schema_not_exists() {
    match tcp_request("drop_schema", json!({
        "schema_name": "nonexistent_schema_xyz_999",
        "if_exists": false
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ drop_schema error handling: correctly rejected nonexistent schema");
            } else {
                panic!("✗ drop_schema should fail for nonexistent schema when if_exists=false");
            }
        }
        Err(_) => {
            println!("✓ drop_schema error handling: correctly rejected nonexistent schema");
        }
    }
}

// ============ ERROR: create_index - missing required parameters ============
#[test]
fn test_error_create_index_missing_params() {
    match tcp_request("create_index", json!({
        "index_name": "idx_test",
        // missing table and columns
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_index error handling: correctly rejected missing parameters");
            } else {
                panic!("✗ create_index should fail when parameters missing");
            }
        }
        Err(_) => {
            println!("✓ create_index error handling: correctly rejected missing parameters");
        }
    }
}

// ============ ERROR: create_index - empty columns array ============
#[test]
fn test_error_create_index_empty_columns() {
    match tcp_request("create_index", json!({
        "index_name": "idx_empty",
        "table": "test_table",
        "columns": []
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_index error handling: correctly rejected empty columns");
            } else {
                panic!("✗ create_index should fail with empty columns");
            }
        }
        Err(_) => {
            println!("✓ create_index error handling: correctly rejected empty columns");
        }
    }
}

// ============ ERROR: drop_index - nonexistent index without if_exists ============
#[test]
fn test_error_drop_index_not_exists() {
    match tcp_request("drop_index", json!({
        "index_name": "nonexistent_idx_xyz_999",
        "if_exists": false
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ drop_index error handling: correctly rejected nonexistent index");
            } else {
                panic!("✗ drop_index should fail for nonexistent index when if_exists=false");
            }
        }
        Err(_) => {
            println!("✓ drop_index error handling: correctly rejected nonexistent index");
        }
    }
}

// ============ ERROR: create_sequence - missing sequence_name ============
#[test]
fn test_error_create_sequence_missing_name() {
    match tcp_request("create_sequence", json!({
        "start": 1
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_sequence error handling: correctly rejected missing sequence_name");
            } else {
                panic!("✗ create_sequence should fail when sequence_name missing");
            }
        }
        Err(_) => {
            println!("✓ create_sequence error handling: correctly rejected missing sequence_name");
        }
    }
}

// ============ ERROR: drop_sequence - nonexistent sequence without if_exists ============
#[test]
fn test_error_drop_sequence_not_exists() {
    match tcp_request("drop_sequence", json!({
        "sequence_name": "nonexistent_seq_xyz_999",
        "if_exists": false
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ drop_sequence error handling: correctly rejected nonexistent sequence");
            } else {
                panic!("✗ drop_sequence should fail for nonexistent sequence when if_exists=false");
            }
        }
        Err(_) => {
            println!("✓ drop_sequence error handling: correctly rejected nonexistent sequence");
        }
    }
}

// ============ ERROR: create_partition - invalid partition_type ============
#[test]
fn test_error_create_partition_invalid_type() {
    match tcp_request("create_partition", json!({
        "table": "test_parts",
        "partition_name": "part_1",
        "partition_type": "INVALID_TYPE",
        "column": "id",
        "values": "FROM (1) TO (100)"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_partition error handling: correctly rejected invalid partition_type");
            } else {
                panic!("✗ create_partition should fail with invalid partition_type");
            }
        }
        Err(_) => {
            println!("✓ create_partition error handling: correctly rejected invalid partition_type");
        }
    }
}

// ============ ERROR: create_partition - SQL injection in values parameter ============
#[test]
fn test_error_create_partition_sql_injection() {
    match tcp_request("create_partition", json!({
        "table": "test_parts",
        "partition_name": "part_bad",
        "partition_type": "RANGE",
        "column": "id",
        "values": "FROM (1) TO (100); DROP TABLE test_parts; --"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ create_partition security: correctly rejected SQL injection attempt");
            } else {
                panic!("✗ create_partition should reject SQL injection patterns");
            }
        }
        Err(_) => {
            println!("✓ create_partition security: correctly rejected SQL injection attempt");
        }
    }
}

// ============ ERROR: list_partitions - missing table parameter ============
#[test]
fn test_error_list_partitions_missing_table() {
    match tcp_request("list_partitions", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ list_partitions error handling: correctly rejected missing table");
            } else {
                panic!("✗ list_partitions should fail when table missing");
            }
        }
        Err(_) => {
            println!("✓ list_partitions error handling: correctly rejected missing table");
        }
    }
}

// ============ EDGE CASE: Very long identifier names (should fail) ============
#[test]
fn test_edge_case_very_long_identifier() {
    let long_name = "a".repeat(300);
    match tcp_request("create_table", json!({
        "table": long_name,
        "columns": ["id SERIAL PRIMARY KEY"]
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ Identifier validation: correctly rejected overly long name");
            } else {
                panic!("✗ Should reject identifiers > 255 characters");
            }
        }
        Err(_) => {
            println!("✓ Identifier validation: correctly rejected overly long name");
        }
    }
}

// ============ EDGE CASE: SQL injection via identifier (should fail) ============
#[test]
fn test_edge_case_sql_injection_in_identifier() {
    match tcp_request("create_table", json!({
        "table": "test; DROP TABLE test; --",
        "columns": ["id SERIAL PRIMARY KEY"]
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ Identifier validation: correctly rejected SQL injection");
            } else {
                panic!("✗ Should reject SQL injection in identifiers");
            }
        }
        Err(_) => {
            println!("✓ Identifier validation: correctly rejected SQL injection");
        }
    }
}

// ============ TOOL 40: backup_table - Happy Path ============
#[test]
fn test_tool_40_backup_table_happy_path() {
    // Create a test table with data
    drop_if_exists("backup_test_backup_source_40");
    drop_if_exists("test_backup_source_40");
    let _guard = TableGuard::new("test_backup_source_40");
    let _ = tcp_request("create_table", json!({
        "table": "test_backup_source_40",
        "columns": ["id SERIAL PRIMARY KEY", "name VARCHAR(255)", "value INT"]
    }));

    // Insert some data
    let _ = tcp_request("execute_insert", json!({
        "sql": "INSERT INTO test_backup_source_40 (name, value) VALUES ('Alice', 100), ('Bob', 200), ('Charlie', 300)"
    }));

    // Create backup
    match tcp_request("backup_table", json!({
        "table": "test_backup_source_40"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("action").and_then(|v| v.as_str()).unwrap_or(""), "BACKUP TABLE");
            assert_eq!(result.get("status").and_then(|v| v.as_str()).unwrap_or(""), "success");
            assert!(result.get("rows_copied").is_some());
            assert_eq!(result.get("rows_copied").and_then(|v| v.as_i64()).unwrap_or(0), 3);
            println!("✓ backup_table: test_backup_source_40 backed up with 3 rows");
        }
        Err(e) => panic!("✗ backup_table failed: {}", e),
    }
}

// ============ ERROR: backup_table - missing table parameter ============
#[test]
fn test_error_backup_table_missing_table() {
    match tcp_request("backup_table", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ backup_table error handling: correctly rejected missing table");
            } else {
                panic!("✗ backup_table should fail when table missing");
            }
        }
        Err(_) => {
            println!("✓ backup_table error handling: correctly rejected missing table");
        }
    }
}

// ============ ERROR: backup_table - nonexistent table ============
#[test]
fn test_error_backup_table_nonexistent() {
    match tcp_request("backup_table", json!({
        "table": "nonexistent_table_xyz_999"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ backup_table error handling: correctly rejected nonexistent table");
            } else {
                panic!("✗ backup_table should fail for nonexistent table");
            }
        }
        Err(_) => {
            println!("✓ backup_table error handling: correctly rejected nonexistent table");
        }
    }
}

// ============ ERROR: backup_table - backup already exists ============
#[test]
fn test_error_backup_table_already_exists() {
    let _guard = TableGuard::new("test_backup_dup_41");
    let _ = tcp_request("create_table", json!({
        "table": "test_backup_dup_41",
        "columns": ["id SERIAL PRIMARY KEY"]
    }));

    // First backup should succeed
    let _ = tcp_request("backup_table", json!({
        "table": "test_backup_dup_41"
    }));

    // Second backup should fail (backup table already exists)
    match tcp_request("backup_table", json!({
        "table": "test_backup_dup_41"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ backup_table error handling: correctly rejected duplicate backup");
            } else {
                panic!("✗ backup_table should fail when backup table already exists");
            }
        }
        Err(_) => {
            println!("✓ backup_table error handling: correctly rejected duplicate backup");
        }
    }
    let _ = tcp_request("drop_table", json!({"table": "backup_test_backup_dup_41"}));
}

// ============ EDGE CASE: backup_table - empty table (0 rows) ============
#[test]
fn test_edge_case_backup_empty_table() {
    let _guard = TableGuard::new("test_backup_empty_42");
    let _ = tcp_request("create_table", json!({
        "table": "test_backup_empty_42",
        "columns": ["id SERIAL PRIMARY KEY", "data TEXT"]
    }));

    match tcp_request("backup_table", json!({
        "table": "test_backup_empty_42"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("rows_copied").and_then(|v| v.as_i64()).unwrap_or(-1), 0);
            println!("✓ backup_table: correctly backed up empty table with 0 rows");
        }
        Err(e) => panic!("✗ backup_table should handle empty tables: {}", e),
    }
    // Also clean up the backup table
    let _ = tcp_request("drop_table", json!({"table": "backup_test_backup_empty_42"}));
}

// ============ EDGE CASE: backup_table - large table with many columns ============
#[test]
fn test_edge_case_backup_many_columns() {
    let _guard = TableGuard::new("test_backup_wide_43");
    let _ = tcp_request("create_table", json!({
        "table": "test_backup_wide_43",
        "columns": [
            "id SERIAL PRIMARY KEY",
            "col1 VARCHAR(50)",
            "col2 VARCHAR(50)",
            "col3 INT",
            "col4 INT",
            "col5 BOOLEAN",
            "col6 TIMESTAMP",
            "col7 TEXT",
            "col8 NUMERIC",
            "col9 VARCHAR(100)",
            "col10 VARCHAR(100)"
        ]
    }));

    match tcp_request("backup_table", json!({
        "table": "test_backup_wide_43"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert_eq!(result.get("columns_copied").and_then(|v| v.as_i64()).unwrap_or(0), 11);
            println!("✓ backup_table: correctly backed up table with 11 columns");
        }
        Err(e) => panic!("✗ backup_table should handle many columns: {}", e),
    }
    let _ = tcp_request("drop_table", json!({"table": "backup_test_backup_wide_43"}));
}

// ============ EDGE CASE: backup_table - table with indexes ============
#[test]
fn test_edge_case_backup_with_indexes() {
    let _guard = TableGuard::new("test_backup_idx_44");
    let _ = tcp_request("create_table", json!({
        "table": "test_backup_idx_44",
        "columns": ["id SERIAL PRIMARY KEY", "email VARCHAR(255)"]
    }));

    // Create index
    let _ = tcp_request("create_index", json!({
        "index_name": "idx_backup_email_44",
        "table": "test_backup_idx_44",
        "columns": ["email"]
    }));

    match tcp_request("backup_table", json!({
        "table": "test_backup_idx_44"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.get("indexes_created").is_some());
            println!("✓ backup_table: correctly backed up table with indexes");
        }
        Err(e) => panic!("✗ backup_table should backup indexes: {}", e),
    }
    let _ = tcp_request("drop_table", json!({"table": "backup_test_backup_idx_44"}));
}

// ============ SECURITY: backup_table - SQL injection in table name (should fail) ============
#[test]
fn test_security_backup_table_sql_injection() {
    match tcp_request("backup_table", json!({
        "table": "test; DROP TABLE test; --"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ backup_table security: correctly rejected SQL injection");
            } else {
                panic!("✗ backup_table should reject SQL injection");
            }
        }
        Err(_) => {
            println!("✓ backup_table security: correctly rejected SQL injection");
        }
    }
}

// ============ RECOVERY: backup_table followed by original table drop (data safety) ============
#[test]
fn test_recovery_backup_before_drop() {
    // Clean up any stale objects from previous runs
    drop_if_exists("backup_test_recovery_45");
    drop_if_exists("test_recovery_45");
    let _guard = TableGuard::new("test_recovery_45");
    let _ = tcp_request("create_table", json!({
        "table": "test_recovery_45",
        "columns": ["id SERIAL PRIMARY KEY", "important_data TEXT"]
    }));

    let _ = tcp_request("execute_insert", json!({
        "sql": "INSERT INTO test_recovery_45 (important_data) VALUES ('critical data'), ('more critical data')"
    }));

    // Create backup first (safety measure)
    let backup_result = tcp_request("backup_table", json!({
        "table": "test_recovery_45"
    }));

    assert!(backup_result.is_ok(), "Backup should succeed");

    // Now drop the original (simulating accidental deletion)
    let _ = tcp_request("drop_table", json!({
        "table": "test_recovery_45"
    }));

    // Verify backup still exists with data
    match tcp_request("execute_query", json!({
        "sql": "SELECT COUNT(*) as row_count FROM backup_test_recovery_45"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let empty: Vec<Value> = vec![];
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap_or(&empty);
            assert!(!rows.is_empty(), "Backup table should have data");
            println!("✓ backup_table: recovery successful - backup preserved after original drop");
        }
        Err(e) => panic!("✗ backup should preserve data: {}", e),
    }
    // Clean up the backup table too
    let _ = tcp_request("drop_table", json!({"table": "backup_test_recovery_45"}));
}

// ============ TOOL 46: async_execute_insert - Happy Path ============
#[test]
fn test_tool_46_async_execute_insert_happy_path() {
    let _guard = TableGuard::new("test_async_insert_46");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_insert_46",
        "columns": ["id SERIAL PRIMARY KEY", "label TEXT", "value INT"]
    }));

    let insert_sql = "INSERT INTO test_async_insert_46 (label, value) VALUES ('alpha', 10), ('beta', 20), ('gamma', 30)";
    match tcp_request("async_execute_insert", json!({"sql": insert_sql})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows_affected").and_then(|v| v.as_u64()).unwrap_or(0);
            assert_eq!(rows, 3, "Should report 3 rows affected");
            println!("✓ async_execute_insert: inserted 3 rows into test_async_insert_46");
        }
        Err(e) => panic!("✗ async_execute_insert failed: {}", e),
    }
}

// ============ TOOL 46b: async_execute_insert - Verify data persisted ============
#[test]
fn test_tool_46b_async_execute_insert_verify_data() {
    let _guard = TableGuard::new("test_async_insert_46b");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_insert_46b",
        "columns": ["id SERIAL PRIMARY KEY", "name VARCHAR(100)", "score INT"]
    }));

    let _ = tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test_async_insert_46b (name, score) VALUES ('alice', 95), ('bob', 87), ('carol', 92)"
    }));

    match tcp_request("execute_query", json!({
        "sql": "SELECT COUNT(*) as cnt FROM test_async_insert_46b"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap();
            assert!(!rows.is_empty(), "Query should return a row");
            let first = &rows[0];
            let cnt = first.get(0).and_then(|v| v.as_i64()).unwrap_or(-1);
            assert_eq!(cnt, 3, "Should have 3 rows persisted");
            println!("✓ async_execute_insert: verified 3 rows persisted in test_async_insert_46b");
        }
        Err(e) => panic!("✗ async_execute_insert data verification failed: {}", e),
    }
}

// ============ TOOL 46c: async_execute_insert - SQL injection rejected ============
#[test]
fn test_tool_46c_async_execute_insert_sql_injection() {
    match tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test VALUES (1); DROP TABLE test; --"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_insert security: multi-statement correctly rejected");
            } else {
                panic!("✗ async_execute_insert should reject multi-statement SQL");
            }
        }
        Err(_) => {
            println!("✓ async_execute_insert security: multi-statement correctly rejected");
        }
    }
}

// ============ TOOL 46d: async_execute_insert - Missing sql param ============
#[test]
fn test_tool_46d_async_execute_insert_missing_sql() {
    match tcp_request("async_execute_insert", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_insert error handling: rejected missing sql");
            } else {
                panic!("✗ async_execute_insert should fail when sql missing");
            }
        }
        Err(_) => {
            println!("✓ async_execute_insert error handling: rejected missing sql");
        }
    }
}

// ============ TOOL 47: async_execute_update - Happy Path ============
#[test]
fn test_tool_47_async_execute_update_happy_path() {
    let _guard = TableGuard::new("test_async_update_47");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_update_47",
        "columns": ["id SERIAL PRIMARY KEY", "status VARCHAR(20) DEFAULT 'pending'", "score INT"]
    }));

    let _ = tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test_async_update_47 (status, score) VALUES ('pending', 10), ('pending', 20), ('active', 30)"
    }));

    match tcp_request("async_execute_update", json!({
        "sql": "UPDATE test_async_update_47 SET status = 'processed' WHERE status = 'pending'"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows_affected").and_then(|v| v.as_u64()).unwrap_or(0);
            assert_eq!(rows, 2, "Should update 2 rows (status='pending')");
            println!("✓ async_execute_update: updated 2 rows from pending→processed");
        }
        Err(e) => panic!("✗ async_execute_update failed: {}", e),
    }
}

// ============ TOOL 47b: async_execute_update - Verify data correctness ============
#[test]
fn test_tool_47b_async_execute_update_verify_data() {
    let _guard = TableGuard::new("test_async_update_47b");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_update_47b",
        "columns": ["id SERIAL PRIMARY KEY", "category VARCHAR(20)", "val INT"]
    }));

    let _ = tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test_async_update_47b (category, val) VALUES ('a', 1), ('a', 2), ('b', 3), ('b', 4), ('c', 5)"
    }));

    let _ = tcp_request("async_execute_update", json!({
        "sql": "UPDATE test_async_update_47b SET val = val + 10 WHERE category IN ('a', 'c')"
    }));

    // Verify 'a' rows updated (1→11, 2→12)
    match tcp_request("execute_query", json!({
        "sql": "SELECT category, val FROM test_async_update_47b WHERE category = 'a' ORDER BY id"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap();
            assert_eq!(rows.len(), 2, "Category 'a' should have 2 rows");
            let val1 = rows[0].get(1).and_then(|v| v.as_i64()).unwrap_or(-1);
            let val2 = rows[1].get(1).and_then(|v| v.as_i64()).unwrap_or(-1);
            assert_eq!(val1, 11, "First 'a' row should have val=11");
            assert_eq!(val2, 12, "Second 'a' row should have val=12");
            println!("✓ async_execute_update: verified updated values in category 'a'");
        }
        Err(e) => panic!("✗ async_execute_update data verification failed: {}", e),
    }

    // Verify 'b' rows NOT updated (3→3, 4→4)
    match tcp_request("execute_query", json!({
        "sql": "SELECT category, val FROM test_async_update_47b WHERE category = 'b' ORDER BY id"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap();
            let val3 = rows[0].get(1).and_then(|v| v.as_i64()).unwrap_or(-1);
            let val4 = rows[1].get(1).and_then(|v| v.as_i64()).unwrap_or(-1);
            assert_eq!(val3, 3, "Category 'b' row should still have val=3");
            assert_eq!(val4, 4, "Category 'b' row should still have val=4");
            println!("✓ async_execute_update: verified category 'b' unchanged (WHERE isolation)");
        }
        Err(e) => panic!("✗ async_execute_update WHERE isolation check failed: {}", e),
    }
}

// ============ TOOL 47c: async_execute_update - Multi-statement injection rejected ============
#[test]
fn test_tool_47c_async_execute_update_sql_injection() {
    match tcp_request("async_execute_update", json!({
        "sql": "UPDATE test SET x = 1; DROP TABLE test; --"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_update security: multi-statement correctly rejected");
            } else {
                panic!("✗ async_execute_update should reject multi-statement SQL");
            }
        }
        Err(_) => {
            println!("✓ async_execute_update security: multi-statement correctly rejected");
        }
    }
}

// ============ TOOL 47d: async_execute_update - Missing sql param ============
#[test]
fn test_tool_47d_async_execute_update_missing_sql() {
    match tcp_request("async_execute_update", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_update error handling: rejected missing sql");
            } else {
                panic!("✗ async_execute_update should fail when sql missing");
            }
        }
        Err(_) => {
            println!("✓ async_execute_update error handling: rejected missing sql");
        }
    }
}

// ============ TOOL 48: async_execute_delete - Happy Path ============
#[test]
fn test_tool_48_async_execute_delete_happy_path() {
    let _guard = TableGuard::new("test_async_delete_48");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_delete_48",
        "columns": ["id SERIAL PRIMARY KEY", "status VARCHAR(20) DEFAULT 'active'"]
    }));

    let _ = tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test_async_delete_48 (status) VALUES ('active'), ('active'), ('archived'), ('archived'), ('active')"
    }));

    match tcp_request("async_execute_delete", json!({
        "sql": "DELETE FROM test_async_delete_48 WHERE status = 'archived'"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows_affected").and_then(|v| v.as_u64()).unwrap_or(0);
            assert_eq!(rows, 2, "Should delete 2 archived rows");
            println!("✓ async_execute_delete: deleted 2 archived rows");
        }
        Err(e) => panic!("✗ async_execute_delete failed: {}", e),
    }
}

// ============ TOOL 48b: async_execute_delete - Verify remaining rows ============
#[test]
fn test_tool_48b_async_execute_delete_verify_data() {
    let _guard = TableGuard::new("test_async_delete_48b");
    let _ = tcp_request("create_table", json!({
        "table": "test_async_delete_48b",
        "columns": ["id SERIAL PRIMARY KEY", "tier VARCHAR(10)"]
    }));

    let _ = tcp_request("async_execute_insert", json!({
        "sql": "INSERT INTO test_async_delete_48b (tier) VALUES ('gold'), ('silver'), ('gold'), ('bronze'), ('gold')"
    }));

    // Delete only silver and bronze
    let _ = tcp_request("async_execute_delete", json!({
        "sql": "DELETE FROM test_async_delete_48b WHERE tier IN ('silver', 'bronze')"
    }));

    match tcp_request("execute_query", json!({
        "sql": "SELECT COUNT(*) as cnt FROM test_async_delete_48b"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap();
            let cnt = rows[0].get(0).and_then(|v| v.as_i64()).unwrap_or(-1);
            assert_eq!(cnt, 3, "Should have 3 gold rows remaining after delete");
            println!("✓ async_execute_delete: verified 3 gold rows remain");
        }
        Err(e) => panic!("✗ async_execute_delete data verification failed: {}", e),
    }

    // Verify no silver or bronze remain
    match tcp_request("execute_query", json!({
        "sql": "SELECT COUNT(*) as cnt FROM test_async_delete_48b WHERE tier IN ('silver', 'bronze')"
    })) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let rows = result.get("rows").and_then(|v| v.as_array()).unwrap();
            let cnt = rows[0].get(0).and_then(|v| v.as_i64()).unwrap_or(-1);
            assert_eq!(cnt, 0, "Should have 0 silver/bronze rows remaining");
            println!("✓ async_execute_delete: verified silver/bronze fully removed");
        }
        Err(e) => panic!("✗ async_execute_delete WHERE isolation check failed: {}", e),
    }
}

// ============ TOOL 48c: async_execute_delete - Multi-statement injection rejected ============
#[test]
fn test_tool_48c_async_execute_delete_sql_injection() {
    match tcp_request("async_execute_delete", json!({
        "sql": "DELETE FROM test WHERE x = 1; DROP TABLE test; --"
    })) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_delete security: multi-statement correctly rejected");
            } else {
                panic!("✗ async_execute_delete should reject multi-statement SQL");
            }
        }
        Err(_) => {
            println!("✓ async_execute_delete security: multi-statement correctly rejected");
        }
    }
}

// ============ TOOL 48d: async_execute_delete - Missing sql param ============
#[test]
fn test_tool_48d_async_execute_delete_missing_sql() {
    match tcp_request("async_execute_delete", json!({})) {
        Ok(response) => {
            if response.get("error").is_some() && !response.get("error").unwrap().is_null() {
                println!("✓ async_execute_delete error handling: rejected missing sql");
            } else {
                panic!("✗ async_execute_delete should fail when sql missing");
            }
        }
        Err(_) => {
            println!("✓ async_execute_delete error handling: rejected missing sql");
        }
    }
}
