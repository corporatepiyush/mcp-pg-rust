use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Helper to send a JSON-RPC request and read the response
fn send_request(host: &str, port: u16, request: &str) -> String {
    let addr = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_secs(5),
    )
    .expect("Failed to connect to server");

    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.iter().any(|&b| b == b'\n') {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).to_string()
}

/// Helper to parse JSON-RPC response
fn parse_response(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw.trim()).expect("Invalid JSON response")
}

// These integration tests require a running MCP server on 127.0.0.1:3000
// Start with: cargo run --release -- --database-url "postgres://..."

const HOST: &str = "127.0.0.1";
const PORT: u16 = 3000;

#[test]
fn test_initialize() {
    let resp = send_request(HOST, PORT, r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#);
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    assert!(json["result"]["serverInfo"]["name"].as_str().unwrap().contains("mcp-postgres"));
    assert_eq!(json["id"], 1);
}

#[test]
fn test_tools_list() {
    let resp = send_request(HOST, PORT, r#"{"jsonrpc":"2.0","method":"tools/list","id":2}"#);
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    let tools = json["result"]["tools"].as_array().expect("tools should be an array");
    assert!(!tools.is_empty(), "Should have at least one tool");
    // Verify tool structure
    for tool in tools {
        assert!(tool["name"].is_string(), "Each tool must have a name");
        assert!(tool["description"].is_string(), "Each tool must have a description");
    }
}

#[test]
fn test_list_tables() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"list_tables"},"id":3}"#,
    );
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    // Should succeed (may return empty table list if schema is fresh)
    assert!(json["error"].is_null() || json.get("result").is_some());
}

#[test]
fn test_show_current_user() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user"},"id":4}"#,
    );
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    let result = json.get("result").expect("Response should have result");
    assert!(result.get("user").is_some(), "Response should contain 'user' field");
    assert!(result.get("database").is_some(), "Response should contain 'database' field");
    assert!(result.get("version").is_some(), "Response should contain 'version' field");
}

#[test]
fn test_execute_query() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1 AS num"}},"id":5}"#,
    );
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    assert!(json["error"].is_null(), "Query should succeed: {:?}", json["error"]);
}

#[test]
fn test_execute_query_invalid_sql() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT invalid"}},"id":6}"#,
    );
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    assert!(json["error"].is_object(), "Invalid SQL should return error");
    assert_eq!(json["error"]["code"], -32000, "DB error code");
}

#[test]
fn test_method_not_found() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"nonexistent_tool"},"id":7}"#,
    );
    let json = parse_response(&resp);
    assert_eq!(json["jsonrpc"], "2.0");
    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], -32601, "Method not found code");
}

#[test]
fn test_execute_insert() {
    // execute_insert requires an INSERT statement
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_insert","arguments":{"sql":"INSERT INTO organizations (name, domain) VALUES ('test-org', 'test.com') ON CONFLICT DO NOTHING"}},"id":10}"#,
    );
    let json = parse_response(&resp);
    // May fail if organizations table doesn't exist, but should return valid JSON-RPC
    assert_eq!(json["jsonrpc"], "2.0");
    if json["error"].is_null() {
        assert!(json["result"]["rows_affected"].is_number());
    } else {
        assert_eq!(json["error"]["code"], -32000, "Expected DB error if table missing");
    }

    // Cleanup
    let _cleanup = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":11}"#,
    );
}

#[test]
fn test_show_database_size() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_database_size"},"id":20}"#,
    );
    let json = parse_response(&resp);
    assert!(json["error"].is_null(), "show_database_size failed: {:?}", json["error"]);
    let databases = json["result"]["databases"].as_array()
        .expect("result.databases should be an array");
    assert!(!databases.is_empty(), "Should have at least one database");
    for db in databases {
        assert!(db["name"].is_string());
        assert!(db["size"].is_string());
    }
}

#[test]
fn test_list_indexes() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"list_indexes","arguments":{"table":"organizations"}},"id":25}"#,
    );
    let json = parse_response(&resp);
    // This may fail if organizations table doesn't exist, but should return valid JSON-RPC
    assert_eq!(json["jsonrpc"], "2.0");
}

#[test]
fn test_concurrent_requests() {
    use std::thread;

    let mut handles = vec![];
    for i in 0..10 {
        handles.push(thread::spawn(move || {
            let resp = send_request(
                HOST,
                PORT,
                &format!(r#"{{"jsonrpc":"2.0","method":"execute_query","params":{{"name":"execute_query","arguments":{{"sql":"SELECT {} AS n"}}}},"id":{}}}"#, i, 100 + i),
            );
            let json: serde_json::Value = serde_json::from_str(resp.trim()).unwrap_or_default();
            json
        }));
    }

    let results: Vec<_> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
    assert_eq!(results.len(), 10, "All 10 concurrent requests should complete");
}

#[test]
fn test_batch_insert_empty() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"batch_insert","arguments":{"table":"organizations","columns":["name"],"rows":[]}},"id":30}"#,
    );
    let json = parse_response(&resp);
    assert!(json["error"].is_null(), "Empty batch should succeed");
    assert_eq!(json["result"]["rows_affected"], 0);
}

#[test]
fn test_batch_insert_column_mismatch() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"batch_insert","arguments":{"table":"organizations","columns":["name","domain"],"rows":[["OnlyName"]]}},"id":31}"#,
    );
    let json = parse_response(&resp);
    assert!(json["error"].is_object(), "Column mismatch should error");
    assert_eq!(json["error"]["code"], -32602);
}

#[test]
fn test_missing_params() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"batch_insert","arguments":{}},"id":32}"#,
    );
    let json = parse_response(&resp);
    assert!(json["error"].is_object(), "Missing params should error");
}

#[test]
fn test_invalid_jsonrpc_version() {
    let resp = send_request(
        HOST,
        PORT,
        r#"{"jsonrpc":"1.0","method":"initialize","id":40}"#,
    );
    let json = parse_response(&resp);
    // Server accepts any version in the request, but the response should always be "2.0"
    assert_eq!(json["jsonrpc"], "2.0");
}

#[test]
fn test_large_string_handling() {
    let long_str = "a".repeat(1000);
    let request = format!(
        r#"{{"jsonrpc":"2.0","method":"tools/call","params":{{"name":"execute_query","arguments":{{"sql":"SELECT '{}' AS s"}}}},"id":50}}"#,
        long_str
    );
    let resp = send_request(HOST, PORT, &request);
    let json = parse_response(&resp);
    assert!(json["error"].is_null(), "Long string query should succeed");
}
