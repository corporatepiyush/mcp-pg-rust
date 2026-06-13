/// Real integration tests - validate actual tool responses
/// These require a running server on localhost:3000
/// Run: cargo run --release -- --database-url "postgres://..."

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

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

    // Send request with newline
    stream.write_all(request_str.as_bytes())?;
    stream.write_all(b"\n")?;

    // Read response
    let mut buffer = vec![0; 65536];
    let n = stream.read(&mut buffer)?;
    let response_str = String::from_utf8(buffer[..n].to_vec())?;

    // Parse response
    let response: Value = serde_json::from_str(&response_str)?;

    // Check for error
    if let Some(error) = response.get("error") {
        if !error.is_null() {
            return Err(format!("Tool error: {}", error).into());
        }
    }

    Ok(response)
}

#[test]
fn test_list_tables_returns_table_array() {
    match tcp_request("list_tables", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            let tables = result.get("tables").expect("Missing tables array");
            assert!(tables.is_array(), "tables should be an array");

            // Should have at least system tables
            let table_list = tables.as_array().unwrap();
            assert!(!table_list.is_empty(), "Should return some tables");

            // Check structure of first table
            if let Some(first_table) = table_list.first() {
                assert!(first_table.get("schema").is_some(), "Table should have schema");
                assert!(first_table.get("name").is_some(), "Table should have name");
                assert!(first_table.get("type").is_some(), "Table should have type");
            }
            println!("✓ list_tables returned {} tables", table_list.len());
        }
        Err(e) => panic!("list_tables failed: {}", e),
    }
}

#[test]
fn test_describe_table_returns_columns() {
    match tcp_request("describe_table", json!({"table": "pg_tables"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            let columns = result.get("columns").expect("Missing columns");
            assert!(columns.is_array(), "columns should be an array");

            let col_list = columns.as_array().unwrap();
            assert!(!col_list.is_empty(), "Should return column info");

            // Check column structure
            if let Some(first_col) = col_list.first() {
                assert!(first_col.get("name").is_some(), "Column should have name");
                assert!(first_col.get("type").is_some(), "Column should have type");
            }
            println!("✓ describe_table returned {} columns", col_list.len());
        }
        Err(e) => panic!("describe_table failed: {}", e),
    }
}

#[test]
fn test_execute_query_returns_rows() {
    match tcp_request("execute_query", json!({"sql": "SELECT 1 as test_col"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            let rows = result.get("rows").expect("Missing rows");
            assert!(rows.is_array(), "rows should be an array");

            let row_list = rows.as_array().unwrap();
            assert!(!row_list.is_empty(), "SELECT 1 should return one row");
            assert_eq!(row_list[0][0], 1, "First column should be 1");
            println!("✓ execute_query returned {} rows", row_list.len());
        }
        Err(e) => panic!("execute_query failed: {}", e),
    }
}

#[test]
fn test_show_current_user_returns_user_info() {
    match tcp_request("show_current_user", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            // Should have user field
            assert!(
                result.get("user").is_some() || result.get("current_user").is_some(),
                "Should return user information"
            );

            if let Some(user) = result.get("user") {
                assert!(user.is_string(), "user should be a string");
                let username = user.as_str().unwrap();
                assert!(!username.is_empty(), "username should not be empty");
                println!("✓ show_current_user returned: {}", username);
            }
        }
        Err(e) => panic!("show_current_user failed: {}", e),
    }
}

#[test]
fn test_list_indexes_returns_indexes() {
    match tcp_request("list_indexes", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            let indexes = result.get("indexes").expect("Missing indexes");
            assert!(indexes.is_array(), "indexes should be an array");

            let idx_list = indexes.as_array().unwrap();
            // Should have at least primary key indexes
            assert!(!idx_list.is_empty(), "Should return some indexes");

            if let Some(first_idx) = idx_list.first() {
                assert!(first_idx.get("name").is_some(), "Index should have name");
                assert!(first_idx.get("table").is_some(), "Index should have table");
            }
            println!("✓ list_indexes returned {} indexes", idx_list.len());
        }
        Err(e) => panic!("list_indexes failed: {}", e),
    }
}

#[test]
fn test_get_cache_hit_ratio_returns_numeric_value() {
    match tcp_request("get_cache_hit_ratio", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be an object");

            // Should have ratio or relevant cache metrics
            assert!(
                result.get("ratio").is_some() ||
                result.get("heap_blks_hit").is_some(),
                "Should return cache metrics"
            );

            if let Some(ratio) = result.get("ratio") {
                println!("✓ get_cache_hit_ratio returned: {}", ratio);
            }
        }
        Err(e) => panic!("get_cache_hit_ratio failed: {}", e),
    }
}

#[test]
fn test_tools_list_returns_all_tools() {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    let request_str = serde_json::to_string(&request).unwrap();
    let mut stream = TcpStream::connect("127.0.0.1:3000").expect("Failed to connect");
    stream.write_all(request_str.as_bytes()).expect("Failed to write");
    stream.write_all(b"\n").expect("Failed to write newline");

    let mut buffer = vec![0; 65536];
    let n = stream.read(&mut buffer).expect("Failed to read");
    let response_str = String::from_utf8(buffer[..n].to_vec()).unwrap();
    let response: Value = serde_json::from_str(&response_str).expect("Failed to parse JSON");

    let result = response.get("result").expect("Missing result");
    let tools = result.get("tools").expect("Missing tools");
    assert!(tools.is_array(), "tools should be an array");

    let tool_list = tools.as_array().unwrap();
    assert!(tool_list.len() > 0, "Should have at least one tool");

    // Check that we have the expected tools
    let tool_names: Vec<String> = tool_list
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect();

    assert!(tool_names.contains(&"list_tables".to_string()), "Should have list_tables");
    assert!(tool_names.contains(&"execute_query".to_string()), "Should have execute_query");
    println!("✓ tools/list returned {} tools", tool_list.len());
}

#[test]
fn test_json_rpc_error_on_invalid_tool() {
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool_xyz",
            "arguments": {}
        },
        "id": 1
    });

    let request_str = serde_json::to_string(&request).unwrap();
    let mut stream = TcpStream::connect("127.0.0.1:3000").expect("Failed to connect");
    stream.write_all(request_str.as_bytes()).expect("Failed to write");
    stream.write_all(b"\n").expect("Failed to write newline");

    let mut buffer = vec![0; 8192];
    let n = stream.read(&mut buffer).expect("Failed to read");
    let response_str = String::from_utf8(buffer[..n].to_vec()).unwrap();
    let response: Value = serde_json::from_str(&response_str).expect("Failed to parse JSON");

    // Should have error
    let error = response.get("error").expect("Should have error field");
    assert!(!error.is_null(), "error should not be null for invalid tool");

    let error_obj = error.as_object().expect("error should be an object");
    assert!(error_obj.get("code").is_some(), "error should have code");
    assert!(error_obj.get("message").is_some(), "error should have message");
    println!("✓ Invalid tool correctly returned error: {:?}", error_obj.get("message"));
}
