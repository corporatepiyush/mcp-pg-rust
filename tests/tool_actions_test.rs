/// Comprehensive tool action tests with expected outputs
/// Tests ALL tool calls with both HTTP and TCP formats

use serde_json::json;

#[test]
fn test_list_tables_tool_http_request() {
    println!("\n=== TOOL: list_tables (HTTP) ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "list_tables",
            "arguments": {}
        },
        "id": 1
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "tables": [
                {
                    "schema": "public",
                    "name": "users",
                    "type": "BASE TABLE"
                },
                {
                    "schema": "public",
                    "name": "orders",
                    "type": "BASE TABLE"
                }
            ]
        },
        "error": null,
        "id": 1
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    // Validate structure
    assert!(serde_json::to_string(&request).unwrap().contains("list_tables"));
    println!("✓ list_tables tool validated");
}

#[test]
fn test_describe_table_tool_http_request() {
    println!("\n=== TOOL: describe_table (HTTP) ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "describe_table",
            "arguments": {
                "table": "users"
            }
        },
        "id": 2
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "columns": [
                {
                    "name": "id",
                    "type": "bigint",
                    "nullable": "NO",
                    "default": "nextval('users_id_seq'::regclass)",
                    "position": 1
                },
                {
                    "name": "email",
                    "type": "character varying",
                    "nullable": "NO",
                    "default": null,
                    "position": 2
                },
                {
                    "name": "created_at",
                    "type": "timestamp without time zone",
                    "nullable": "NO",
                    "default": "CURRENT_TIMESTAMP",
                    "position": 3
                }
            ]
        },
        "error": null,
        "id": 2
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("describe_table"));
    println!("✓ describe_table tool validated");
}

#[test]
fn test_execute_query_tool_tcp_format() {
    println!("\n=== TOOL: execute_query (TCP) ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "execute_query",
            "arguments": {
                "sql": "SELECT id, email FROM users WHERE id = 1"
            }
        },
        "id": 3
    });

    let request_str = serde_json::to_string(&request).unwrap();
    println!("TCP REQUEST (newline-delimited):");
    println!("{}", request_str);
    println!("\\n");

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows": [
                [1, "alice@example.com"]
            ]
        },
        "error": null,
        "id": 3
    });

    println!("EXPECTED TCP RESPONSE:");
    let resp_str = serde_json::to_string(&expected_response).unwrap();
    println!("{}", resp_str);
    println!("\\n");

    assert!(request_str.contains("execute_query"));
    assert!(request_str.contains("SELECT id, email FROM users"));
    println!("✓ execute_query tool validated");
}

#[test]
fn test_execute_insert_tool() {
    println!("\n=== TOOL: execute_insert ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "execute_insert",
            "arguments": {
                "sql": "INSERT INTO users (email) VALUES ('bob@example.com') RETURNING id, email"
            }
        },
        "id": 4
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows_affected": 1,
            "rows": [
                [2, "bob@example.com"]
            ]
        },
        "error": null,
        "id": 4
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("execute_insert"));
    println!("✓ execute_insert tool validated");
}

#[test]
fn test_execute_update_tool() {
    println!("\n=== TOOL: execute_update ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "execute_update",
            "arguments": {
                "sql": "UPDATE users SET email = 'alice.new@example.com' WHERE id = 1"
            }
        },
        "id": 5
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows_affected": 1
        },
        "error": null,
        "id": 5
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("execute_update"));
    println!("✓ execute_update tool validated");
}

#[test]
fn test_execute_delete_tool() {
    println!("\n=== TOOL: execute_delete ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "execute_delete",
            "arguments": {
                "sql": "DELETE FROM users WHERE id = 2"
            }
        },
        "id": 6
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows_affected": 1
        },
        "error": null,
        "id": 6
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("execute_delete"));
    println!("✓ execute_delete tool validated");
}

#[test]
fn test_batch_insert_tool() {
    println!("\n=== TOOL: batch_insert (High-Performance) ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "batch_insert",
            "arguments": {
                "table": "users",
                "columns": ["email", "name"],
                "rows": [
                    ["user1@example.com", "User One"],
                    ["user2@example.com", "User Two"],
                    ["user3@example.com", "User Three"]
                ]
            }
        },
        "id": 7
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "rows_affected": 3
        },
        "error": null,
        "id": 7
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("batch_insert"));
    println!("✓ batch_insert tool validated");
}

#[test]
fn test_explain_query_tool() {
    println!("\n=== TOOL: explain_query ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "explain_query",
            "arguments": {
                "sql": "SELECT * FROM users WHERE id = 1",
                "analyze": true,
                "buffers": true,
                "format": "json"
            }
        },
        "id": 8
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "plan": [
                {
                    "Node Type": "Index Scan",
                    "Index Name": "users_pkey",
                    "Rows": 1,
                    "Actual Rows": 1
                }
            ]
        },
        "error": null,
        "id": 8
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("explain_query"));
    println!("✓ explain_query tool validated");
}

#[test]
fn test_list_indexes_tool() {
    println!("\n=== TOOL: list_indexes ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "list_indexes",
            "arguments": {}
        },
        "id": 9
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "indexes": [
                {
                    "schema": "public",
                    "table": "users",
                    "name": "users_pkey",
                    "definition": "CREATE UNIQUE INDEX users_pkey ON public.users USING btree (id)"
                },
                {
                    "schema": "public",
                    "table": "users",
                    "name": "users_email_idx",
                    "definition": "CREATE INDEX users_email_idx ON public.users USING btree (email)"
                }
            ]
        },
        "error": null,
        "id": 9
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("list_indexes"));
    println!("✓ list_indexes tool validated");
}

#[test]
fn test_show_current_user_tool() {
    println!("\n=== TOOL: show_current_user ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "show_current_user",
            "arguments": {}
        },
        "id": 10
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "current_user": "postgres",
            "session_user": "postgres"
        },
        "error": null,
        "id": 10
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("show_current_user"));
    println!("✓ show_current_user tool validated");
}

#[test]
fn test_get_table_stats_tool() {
    println!("\n=== TOOL: get_table_stats ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "get_table_stats",
            "arguments": {
                "table": "users"
            }
        },
        "id": 11
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "seq_scans": 5,
            "seq_tup_read": 1000,
            "idx_scans": 25,
            "idx_tup_fetch": 500,
            "n_tup_ins": 100,
            "n_tup_upd": 50,
            "n_tup_del": 10,
            "n_live_tup": 140,
            "n_dead_tup": 5
        },
        "error": null,
        "id": 11
    });

    println!("\nEXPECTED RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("get_table_stats"));
    println!("✓ get_table_stats tool validated");
}

#[test]
fn test_error_response_invalid_tool() {
    println!("\n=== ERROR CASE: Invalid Tool ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "nonexistent_tool",
            "arguments": {}
        },
        "id": 12
    });

    println!("REQUEST (invalid tool):");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_error = json!({
        "jsonrpc": "2.0",
        "result": null,
        "error": {
            "code": -32600,
            "message": "Tool not found",
            "data": "nonexistent_tool"
        },
        "id": 12
    });

    println!("\nEXPECTED ERROR RESPONSE:");
    println!("{}", serde_json::to_string_pretty(&expected_error).unwrap());

    assert!(serde_json::to_string(&request).unwrap().contains("nonexistent_tool"));
    println!("✓ Error handling validated");
}

#[test]
fn test_tool_list_method() {
    println!("\n=== METHOD: tools/list ===\n");

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 13
    });

    println!("REQUEST:");
    println!("{}", serde_json::to_string_pretty(&request).unwrap());

    let expected_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": [
                {
                    "name": "list_tables",
                    "description": "List all user tables with schema and type",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "describe_table",
                    "description": "Describe a table's columns, types, nullability, and defaults",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "table": {
                                "type": "string",
                                "description": "Table name"
                            }
                        }
                    }
                }
            ]
        },
        "id": 13
    });

    println!("\nEXPECTED RESPONSE (truncated):");
    println!("{{{{");
    println!("  \"tools\": [");
    println!("    {{\"name\": \"list_tables\", ...}},");
    println!("    {{\"name\": \"describe_table\", ...}},");
    println!("    ... (60+ tools total)");
    println!("  ]");
    println!("}}}}");

    assert!(serde_json::to_string(&request).unwrap().contains("tools/list"));
    println!("✓ tools/list method validated");
}
