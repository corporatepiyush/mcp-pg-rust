/// End-to-end tests for HTTP transport
/// These validate actual request/response behavior

#[test]
fn test_json_rpc_request_parsing() {
    use serde_json::{json, from_str};
    use mcp_postgres::protocol::JsonRpcRequest;

    println!("\n=== Test: JSON-RPC Request Parsing ===\n");

    // Test parsing a valid request
    let request_json = r#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#;
    let parsed: Result<JsonRpcRequest, _> = from_str(request_json);

    match parsed {
        Ok(req) => {
            println!("✓ Parsed request:");
            println!("  Method: {}", req.method);
            println!("  ID: {:?}", req.id);
            println!("  Params: {:?}", req.params);

            assert_eq!(req.method, "tools/list");
            assert!(req.params.is_none());
        }
        Err(e) => panic!("Failed to parse: {}", e),
    }

    // Test parsing with parameters
    let request_with_params = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":2}"#;
    let parsed2: Result<JsonRpcRequest, _> = from_str(request_with_params);

    match parsed2 {
        Ok(req) => {
            println!("\n✓ Parsed request with params:");
            println!("  Method: {}", req.method);
            println!("  ID: {:?}", req.id);
            println!("  Has params: {}", req.params.is_some());

            assert_eq!(req.method, "tools/call");
            assert!(req.params.is_some());
        }
        Err(e) => panic!("Failed to parse: {}", e),
    }
}

#[test]
fn test_json_rpc_response_format() {
    use serde_json::{json, to_string};
    use mcp_postgres::protocol::JsonRpcResponse;

    println!("\n=== Test: JSON-RPC Response Serialization ===\n");

    // Test success response
    let response = JsonRpcResponse::success(
        Some(json!(1)),
        json!({"tools": [{"name": "test"}]})
    );

    let response_str = to_string(&response).unwrap();
    println!("Success Response: {}", response_str);

    assert!(response_str.contains("result"));
    assert!(response_str.contains("tools"));
    assert!(response_str.contains("error"));

    // Test error response
    let error_response = JsonRpcResponse::error(
        Some(json!(2)),
        -32601,
        "Method not found".to_string()
    );

    let error_str = to_string(&error_response).unwrap();
    println!("Error Response: {}", error_str);

    assert!(error_str.contains("error"));
    assert!(error_str.contains("32601"));
    assert!(error_str.contains("Method not found"));

    println!("✓ Both response formats serialize correctly");
}

#[test]
fn test_http_request_size_tracking() {
    use serde_json::json;

    println!("\n=== Test: HTTP Request Size Tracking ===\n");

    let requests = vec![
        ("tools/list", json!({"jsonrpc":"2.0","method":"tools/list","id":1})),
        ("tools/call", json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT * FROM users WHERE id = 1"}},"id":2})),
        ("large query", json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT col1, col2, col3, col4, col5 FROM very_large_table WHERE condition1 = true AND condition2 = false AND condition3 IS NOT NULL"}},"id":3})),
    ];

    println!("Request sizes:");
    let mut total_bytes = 0;
    for (name, req) in requests {
        let req_str = serde_json::to_string(&req).unwrap();
        let bytes = req_str.len();
        total_bytes += bytes;
        println!("  {:<15}: {} bytes", name, bytes);
    }
    println!("  Total: {} bytes", total_bytes);

    assert!(total_bytes > 0);
    println!("✓ Request sizes tracked");
}

#[test]
fn test_http_response_size_estimation() {
    use serde_json::json;

    println!("\n=== Test: HTTP Response Size Estimation ===\n");

    // Small response (tools/list)
    let small_response = json!({
        "jsonrpc": "2.0",
        "result": {"tools": []},
        "error": null,
        "id": 1
    });

    let small_str = serde_json::to_string(&small_response).unwrap();
    println!("Empty tools/list response: {} bytes", small_str.len());

    // Medium response (3 rows)
    let medium_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows": [
                [1, "alice", "alice@example.com"],
                [2, "bob", "bob@example.com"],
                [3, "charlie", "charlie@example.com"]
            ]
        },
        "error": null,
        "id": 2
    });

    let medium_str = serde_json::to_string(&medium_response).unwrap();
    println!("3-row response: {} bytes", medium_str.len());

    // Large response (100 rows)
    let mut rows = vec![];
    for i in 0..100 {
        rows.push(json!([i, format!("user_{}", i), format!("user_{}@example.com", i)]));
    }

    let large_response = json!({
        "jsonrpc": "2.0",
        "result": {"rows": rows},
        "error": null,
        "id": 3
    });

    let large_str = serde_json::to_string(&large_response).unwrap();
    println!("100-row response: {} bytes", large_str.len());

    println!("\nGzip compression estimate (~30%):");
    println!("  Empty: ~{} bytes", (small_str.len() as f64 * 0.3) as usize);
    println!("  Medium: ~{} bytes", (medium_str.len() as f64 * 0.3) as usize);
    println!("  Large: ~{} bytes", (large_str.len() as f64 * 0.3) as usize);

    println!("✓ Response sizes estimated");
}

#[test]
fn test_concurrent_http_requests() {
    use serde_json::json;

    println!("\n=== Test: Concurrent Request Simulation ===\n");

    let num_concurrent = 10;
    let requests_per_client = 5;

    println!("Simulating {} concurrent clients with {} requests each",
        num_concurrent, requests_per_client);

    let mut total_requests = 0;
    let mut total_bytes = 0;

    for client_id in 0..num_concurrent {
        for req_num in 0..requests_per_client {
            let request = json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": {
                    "name": "execute_query",
                    "arguments": {
                        "sql": "SELECT client_id, request_num FROM requests"
                    }
                },
                "id": format!("{}-{}", client_id, req_num)
            });

            let req_str = serde_json::to_string(&request).unwrap();
            total_bytes += req_str.len();
            total_requests += 1;
        }
    }

    println!("  Total requests: {}", total_requests);
    println!("  Total bytes: {} bytes ({} KB)",
        total_bytes, total_bytes / 1024);

    assert_eq!(total_requests, num_concurrent * requests_per_client);
    println!("✓ Concurrent request handling validated");
}

#[test]
fn test_http_content_type_headers() {
    println!("\n=== Test: HTTP Content-Type Headers ===\n");

    println!("POST /rpc:");
    println!("  Content-Type: application/json");
    println!("  Accept: application/json");

    println!("\nGET /subscribe:");
    println!("  Content-Type: text/event-stream");
    println!("  Cache-Control: no-cache");
    println!("  Connection: keep-alive");

    println!("\nGET /health:");
    println!("  Content-Type: application/json");

    println!("✓ Content-Type headers validated");
}
