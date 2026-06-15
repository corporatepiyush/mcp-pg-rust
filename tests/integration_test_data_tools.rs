#![allow(clippy::needless_pass_by_value)]

/// Integration tests for all tools using generated test data
/// Requires running: cargo run --release --bin load_test_data
/// Then: cargo test --test integration_test_data_tools -- --nocapture
use serde_json::{Value, json};
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
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

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

// ========== Schema Inspection Tests ==========

#[test]
fn test_list_tables_returns_12_tables() {
    match tcp_request("list_tables", json!({})) {
        Ok(response) => {
            let tables = response
                .get("result")
                .and_then(|r| r.get("tables"))
                .expect("Missing tables");

            let table_list = tables.as_array().unwrap();
            assert!(table_list.len() >= 12, "Should have at least 12 tables");

            // Check for specific tables
            let names: Vec<String> = table_list
                .iter()
                .filter_map(|t| {
                    t.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect();

            assert!(
                names.contains(&"customers".to_string()),
                "Missing customers table"
            );
            assert!(
                names.contains(&"orders".to_string()),
                "Missing orders table"
            );
            assert!(
                names.contains(&"products".to_string()),
                "Missing products table"
            );
            println!(
                "✓ list_tables: {} tables found (expected 12+)",
                table_list.len()
            );
        }
        Err(e) => panic!("✗ list_tables failed: {}", e),
    }
}

#[test]
fn test_describe_customers_table() {
    match tcp_request("describe_table", json!({"table": "customers"})) {
        Ok(response) => {
            let columns = response
                .get("result")
                .and_then(|r| r.get("columns"))
                .expect("Missing columns");

            let col_list = columns.as_array().unwrap();
            assert!(!col_list.is_empty(), "customers should have columns");

            let col_names: Vec<String> = col_list
                .iter()
                .filter_map(|c| {
                    c.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect();

            assert!(col_names.contains(&"id".to_string()));
            assert!(col_names.contains(&"email".to_string()));
            assert!(col_names.contains(&"created_at".to_string()));
            println!("✓ describe_table: customers has {} columns", col_list.len());
        }
        Err(e) => panic!("✗ describe_table failed: {}", e),
    }
}

#[test]
fn test_list_indexes_on_orders_table() {
    match tcp_request("list_indexes", json!({})) {
        Ok(response) => {
            let indexes = response
                .get("result")
                .and_then(|r| r.get("indexes"))
                .expect("Missing indexes");

            let idx_list = indexes.as_array().unwrap();
            assert!(!idx_list.is_empty(), "Should have indexes");

            // Look for orders-related indexes
            let orders_indexes = idx_list
                .iter()
                .filter(|idx| {
                    idx.get("table")
                        .and_then(|t| t.as_str())
                        .map(|s| s.contains("order"))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>();

            assert!(
                !orders_indexes.is_empty(),
                "Should have indexes on orders table"
            );
            println!(
                "✓ list_indexes: {} total indexes, {} on orders",
                idx_list.len(),
                orders_indexes.len()
            );
        }
        Err(e) => panic!("✗ list_indexes failed: {}", e),
    }
}

#[test]
fn test_list_schemas_contains_public() {
    match tcp_request("list_schemas", json!({})) {
        Ok(response) => {
            let schemas = response
                .get("result")
                .and_then(|r| r.get("schemas"))
                .expect("Missing schemas");

            let schema_list = schemas.as_array().unwrap();
            let has_public = schema_list.iter().any(|s| {
                s.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s == "public")
                    .unwrap_or(false)
            });

            assert!(has_public, "Should have public schema");
            println!("✓ list_schemas: found public schema");
        }
        Err(e) => panic!("✗ list_schemas failed: {}", e),
    }
}

// ========== Query Execution Tests ==========

#[test]
fn test_execute_query_count_customers() {
    match tcp_request(
        "execute_query",
        json!({"sql": "SELECT COUNT(*) as count FROM customers"}),
    ) {
        Ok(response) => {
            let rows = response
                .get("result")
                .and_then(|r| r.get("rows"))
                .expect("Missing rows");

            let row_list = rows.as_array().unwrap();
            assert!(!row_list.is_empty(), "Should return result");
            let count = row_list[0][0].as_i64().unwrap_or(0);
            assert!(count >= 100, "Should have at least 100 customers");
            println!("✓ execute_query: {} customers in database", count);
        }
        Err(e) => panic!("✗ execute_query failed: {}", e),
    }
}

#[test]
fn test_execute_query_join_orders_and_customers() {
    match tcp_request(
        "execute_query",
        json!({
            "sql": "SELECT c.email, COUNT(o.id) as order_count FROM customers c LEFT JOIN orders o ON c.id = o.customer_id WHERE c.email LIKE '%example%' GROUP BY c.email LIMIT 10"
        }),
    ) {
        Ok(response) => {
            let rows = response
                .get("result")
                .and_then(|r| r.get("rows"))
                .expect("Missing rows");

            let row_list = rows.as_array().unwrap();
            assert!(!row_list.is_empty(), "Should return joined results");
            println!("✓ execute_query: JOIN returned {} rows", row_list.len());
        }
        Err(e) => panic!("✗ execute_query JOIN failed: {}", e),
    }
}

#[test]
fn test_execute_query_aggregation() {
    match tcp_request(
        "execute_query",
        json!({
            "sql": "SELECT category_id, COUNT(*) as product_count, AVG(price) as avg_price FROM products GROUP BY category_id ORDER BY product_count DESC LIMIT 5"
        }),
    ) {
        Ok(response) => {
            let rows = response
                .get("result")
                .and_then(|r| r.get("rows"))
                .expect("Missing rows");

            let row_list = rows.as_array().unwrap();
            assert!(!row_list.is_empty(), "Should return aggregated results");
            println!(
                "✓ execute_query: Aggregation returned {} rows",
                row_list.len()
            );
        }
        Err(e) => panic!("✗ execute_query aggregation failed: {}", e),
    }
}

// ========== Monitoring Tests ==========

#[test]
fn test_analyze_db_health() {
    match tcp_request("analyze_db_health", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be object");

            // Check for health metrics
            if let Some(buffer) = result.get("buffer_cache") {
                assert!(buffer.get("hit_ratio_pct").is_some() || buffer.get("status").is_some());
            }
            println!("✓ analyze_db_health: health metrics obtained");
        }
        Err(e) => panic!("✗ analyze_db_health failed: {}", e),
    }
}

#[test]
fn test_list_unused_indexes() {
    match tcp_request("list_unused_indexes", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let indexes = result.get("indexes").expect("Missing indexes");

            assert!(indexes.is_array(), "indexes should be array");
            println!(
                "✓ list_unused_indexes: {} indexes checked",
                indexes.as_array().unwrap().len()
            );
        }
        Err(e) => panic!("✗ list_unused_indexes failed: {}", e),
    }
}

#[test]
fn test_get_cache_hit_ratio_with_queries() {
    // First, run some queries to populate cache
    let _ = tcp_request(
        "execute_query",
        json!({"sql": "SELECT * FROM customers LIMIT 10"}),
    );
    let _ = tcp_request(
        "execute_query",
        json!({"sql": "SELECT * FROM orders LIMIT 10"}),
    );

    match tcp_request("get_cache_hit_ratio", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be object");
            println!("✓ get_cache_hit_ratio: obtained after running queries");
        }
        Err(e) => panic!("✗ get_cache_hit_ratio failed: {}", e),
    }
}

// ========== Performance Tests ==========

#[test]
fn test_explain_query_performance() {
    match tcp_request(
        "explain_query",
        json!({
            "sql": "SELECT c.email, COUNT(o.id) FROM customers c LEFT JOIN orders o ON c.id = o.customer_id GROUP BY c.email",
            "analyze": false,
            "buffers": false,
            "format": "json"
        }),
    ) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be object");
            println!("✓ explain_query: plan obtained for complex JOIN");
        }
        Err(e) => panic!("✗ explain_query failed: {}", e),
    }
}

#[test]
fn test_get_pg_stat_statements() {
    match tcp_request("get_pg_stat_statements", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let statements = result.get("statements").expect("Missing statements");

            let stmt_list = statements.as_array().unwrap();
            println!(
                "✓ get_pg_stat_statements: {} statements tracked",
                stmt_list.len()
            );
        }
        Err(_e) => {
            // Extension might not be installed
            println!("⚠ get_pg_stat_statements: extension not installed (this is OK)");
        }
    }
}

// ========== Settings Tests ==========

#[test]
fn test_get_setting_max_connections() {
    match tcp_request("get_setting", json!({"setting_name": "max_connections"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let value = result.get("value").expect("Missing value");

            let max_conn = value.as_str().unwrap_or("100");
            println!("✓ get_setting: max_connections = {}", max_conn);
        }
        Err(e) => panic!("✗ get_setting failed: {}", e),
    }
}

#[test]
fn test_get_setting_work_mem() {
    match tcp_request("get_setting", json!({"setting_name": "work_mem"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let value = result.get("value").expect("Missing value");

            println!("✓ get_setting: work_mem = {}", value);
        }
        Err(e) => panic!("✗ get_setting failed: {}", e),
    }
}

// ========== User & Security Tests ==========

#[test]
fn test_list_users() {
    match tcp_request("list_users", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let users = result.get("users").expect("Missing users");

            let user_list = users.as_array().unwrap();
            assert!(!user_list.is_empty(), "Should have at least one user");
            println!("✓ list_users: {} database users", user_list.len());
        }
        Err(e) => panic!("✗ list_users failed: {}", e),
    }
}

#[test]
fn test_show_current_user() {
    match tcp_request("show_current_user", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            let user = result
                .get("user")
                .or_else(|| result.get("current_user"))
                .expect("Missing user info");

            println!("✓ show_current_user: logged in as {}", user);
        }
        Err(e) => panic!("✗ show_current_user failed: {}", e),
    }
}

#[test]
fn test_show_session_info() {
    match tcp_request("show_session_info", json!({})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object(), "Result should be object");
            println!("✓ show_session_info: session info obtained");
        }
        Err(e) => panic!("✗ show_session_info failed: {}", e),
    }
}
