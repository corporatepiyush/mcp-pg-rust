#![allow(clippy::cast_precision_loss, clippy::needless_pass_by_value)]

use once_cell::sync::Lazy;
/// Dual-protocol integration tests for ALL PostgreSQL tools
/// Tests both TCP (port 3000) and HTTP (port 3001)
/// Records latency, throughput, and success/failure rates
/// Run: cargo test --test integration_dual_protocol -- --nocapture
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct ProtocolStats {
    _name: String,
    protocol: String,
    success: usize,
    _failed: usize,
    total_ms: u128,
    min_ms: u128,
    max_ms: u128,
    requests: usize,
}

impl ProtocolStats {
    fn new(name: &str, protocol: &str) -> Self {
        ProtocolStats {
            _name: name.to_string(),
            protocol: protocol.to_string(),
            success: 0,
            _failed: 0,
            total_ms: 0,
            min_ms: u128::MAX,
            max_ms: 0,
            requests: 0,
        }
    }

    fn avg_ms(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.total_ms as f64 / self.requests as f64
        }
    }

    fn record(&mut self, duration_ms: u128, success: bool) {
        self.requests += 1;
        self.total_ms += duration_ms;
        self.min_ms = self.min_ms.min(duration_ms);
        self.max_ms = self.max_ms.max(duration_ms);
        if success {
            self.success += 1;
        }
    }
}

#[allow(clippy::type_complexity)]
static STATS: Lazy<Arc<Mutex<HashMap<String, Vec<ProtocolStats>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

// ============ TCP REQUEST ============
fn tcp_request(tool_name: &str, arguments: Value) -> (bool, u128) {
    let start = Instant::now();

    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        },
        "id": 1
    });

    let request_str = match serde_json::to_string(&request) {
        Ok(s) => s,
        Err(_) => return (false, start.elapsed().as_millis()),
    };

    match TcpStream::connect("127.0.0.1:3000") {
        Ok(mut stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

            if stream.write_all(request_str.as_bytes()).is_err() {
                return (false, start.elapsed().as_millis());
            }
            if stream.write_all(b"\n").is_err() {
                return (false, start.elapsed().as_millis());
            }

            let mut buffer = vec![0; 65536];
            match stream.read(&mut buffer) {
                Ok(n) => {
                    let response_str = match String::from_utf8(buffer[..n].to_vec()) {
                        Ok(s) => s,
                        Err(_) => return (false, start.elapsed().as_millis()),
                    };
                    match serde_json::from_str::<Value>(&response_str) {
                        Ok(response) => {
                            let has_error =
                                response.get("error").map(|e| !e.is_null()).unwrap_or(false);
                            (
                                !has_error && response.get("result").is_some(),
                                start.elapsed().as_millis(),
                            )
                        }
                        Err(_) => (false, start.elapsed().as_millis()),
                    }
                }
                Err(_) => (false, start.elapsed().as_millis()),
            }
        }
        Err(_) => (false, start.elapsed().as_millis()),
    }
}

// ============ HTTP REQUEST ============
async fn http_request(tool_name: &str, arguments: Value) -> (bool, u128) {
    let start = Instant::now();

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        },
        "id": 1
    });

    let client = reqwest::Client::new();
    match client
        .post("http://127.0.0.1:3001/rpc")
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            if !resp.status().is_success() {
                return (false, start.elapsed().as_millis());
            }
            match resp.json::<Value>().await {
                Ok(response) => {
                    let has_error = response.get("error").map(|e| !e.is_null()).unwrap_or(false);
                    (
                        !has_error && response.get("result").is_some(),
                        start.elapsed().as_millis(),
                    )
                }
                Err(_) => (false, start.elapsed().as_millis()),
            }
        }
        Err(_) => (false, start.elapsed().as_millis()),
    }
}

// ============ HELPER: Test tool on both protocols ============
fn test_tool_dual_protocol(tool_name: &str, arguments: Value, description: &str) {
    let mut stats_map = STATS.lock().unwrap();
    stats_map.entry(tool_name.to_string()).or_default();

    // TCP Test
    let (tcp_success, tcp_ms) = tcp_request(tool_name, arguments.clone());
    let mut tcp_stat = ProtocolStats::new(tool_name, "TCP");
    tcp_stat.record(tcp_ms, tcp_success);
    stats_map.get_mut(tool_name).unwrap().push(tcp_stat);

    // HTTP Test (async requires special handling in sync test)
    let http_runtime = tokio::runtime::Runtime::new().unwrap();
    let (http_success, http_ms) =
        http_runtime.block_on(async { http_request(tool_name, arguments.clone()).await });
    let mut http_stat = ProtocolStats::new(tool_name, "HTTP");
    http_stat.record(http_ms, http_success);
    stats_map.get_mut(tool_name).unwrap().push(http_stat);

    // Print result
    let tcp_status = if tcp_success { "✓" } else { "✗" };
    let http_status = if http_success { "✓" } else { "✗" };
    println!(
        "{} TCP {:4}ms | {} HTTP {:4}ms | {}",
        tcp_status, tcp_ms, http_status, http_ms, description
    );
}

// ============ TESTS ============

#[test]
fn test_01_list_tables() {
    test_tool_dual_protocol("list_tables", json!({}), "list_tables");
}

#[test]
fn test_02_show_current_user() {
    test_tool_dual_protocol("show_current_user", json!({}), "show_current_user");
}

#[test]
fn test_03_execute_query() {
    test_tool_dual_protocol(
        "execute_query",
        json!({"sql": "SELECT 1 as col"}),
        "execute_query (SELECT 1)",
    );
}

#[test]
fn test_04_list_schemas() {
    test_tool_dual_protocol("list_schemas", json!({}), "list_schemas");
}

#[test]
fn test_05_describe_table() {
    test_tool_dual_protocol(
        "describe_table",
        json!({"table": "pg_tables"}),
        "describe_table",
    );
}

#[test]
fn test_06_list_indexes() {
    test_tool_dual_protocol("list_indexes", json!({}), "list_indexes");
}

#[test]
fn test_07_list_triggers() {
    test_tool_dual_protocol(
        "list_triggers",
        json!({"table": "pg_tables"}),
        "list_triggers",
    );
}

#[test]
fn test_08_show_session_info() {
    test_tool_dual_protocol("show_session_info", json!({}), "show_session_info");
}

#[test]
fn test_09_list_users() {
    test_tool_dual_protocol("list_users", json!({}), "list_users");
}

#[test]
fn test_10_analyze_db_health() {
    test_tool_dual_protocol("analyze_db_health", json!({}), "analyze_db_health");
}

// ============ REPORT AT END ============
#[test]
fn zzz_final_report() {
    std::thread::sleep(Duration::from_millis(500)); // Let other tests finish

    let stats_map = STATS.lock().unwrap();

    if stats_map.is_empty() {
        println!("\n⚠️  No stats collected");
        return;
    }

    println!("\n");
    println!("╔════════════════════════════════════════════════════════════════════╗");
    println!("║               DUAL PROTOCOL TEST REPORT (TCP vs HTTP)              ║");
    println!("╚════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut tcp_total = 0;
    let mut tcp_success = 0;
    let mut http_total = 0;
    let mut http_success = 0;
    let mut tcp_total_ms = 0;
    let mut http_total_ms = 0;

    // Aggregate by protocol
    for (_tool, stats_list) in stats_map.iter() {
        for stat in stats_list {
            if stat.protocol == "TCP" {
                tcp_total += stat.requests;
                tcp_success += stat.success;
                tcp_total_ms += stat.total_ms;
            } else {
                http_total += stat.requests;
                http_success += stat.success;
                http_total_ms += stat.total_ms;
            }
        }
    }

    println!("📊 SUMMARY");
    println!("──────────────────────────────────────────────────────────────────────");
    println!(
        "TCP  │ Success: {}/{} ({:.1}%) │ Avg latency: {:.1}ms",
        tcp_success,
        tcp_total,
        (tcp_success as f64 / tcp_total as f64) * 100.0,
        tcp_total_ms as f64 / tcp_total as f64
    );
    println!(
        "HTTP │ Success: {}/{} ({:.1}%) │ Avg latency: {:.1}ms",
        http_success,
        http_total,
        (http_success as f64 / http_total as f64) * 100.0,
        http_total_ms as f64 / http_total as f64
    );
    println!();

    println!("📈 PER-TOOL COMPARISON");
    println!("──────────────────────────────────────────────────────────────────────");
    println!("{:<25} {:<15} {:<15}", "Tool", "TCP", "HTTP");
    println!("──────────────────────────────────────────────────────────────────────");

    let mut tools: Vec<_> = stats_map.keys().collect();
    tools.sort();

    for tool in tools {
        let stats_list = &stats_map[tool];
        let tcp_stat = stats_list.iter().find(|s| s.protocol == "TCP");
        let http_stat = stats_list.iter().find(|s| s.protocol == "HTTP");

        let tcp_info = tcp_stat
            .map(|s| format!("{:.1}ms ({}/{})", s.avg_ms(), s.success, s.requests))
            .unwrap_or_else(|| "N/A".to_string());
        let http_info = http_stat
            .map(|s| format!("{:.1}ms ({}/{})", s.avg_ms(), s.success, s.requests))
            .unwrap_or_else(|| "N/A".to_string());

        println!("{:<25} {:<15} {:<15}", tool, tcp_info, http_info);
    }

    println!();
    println!("✅ Dual protocol testing complete!");
    println!(
        "   TCP  calls: {} successful out of {} ({:.1}%)",
        tcp_success,
        tcp_total,
        (tcp_success as f64 / tcp_total as f64) * 100.0
    );
    println!(
        "   HTTP calls: {} successful out of {} ({:.1}%)",
        http_success,
        http_total,
        (http_success as f64 / http_total as f64) * 100.0
    );
}
