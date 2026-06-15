#![allow(clippy::cast_precision_loss)]

use reqwest::Client;
use serde_json::json;
use std::collections::BTreeMap;
/// HTTP Server Latency Measurement Tool
/// Measures end-to-end latency for all MCP tools via HTTP/2
/// Run: cargo run --release --bin measure_latency
use std::sync::Arc;
use std::time::Instant;
use tokio::task;

#[derive(Clone)]
#[allow(dead_code)]
struct LatencyStats {
    min_ms: f64,
    max_ms: f64,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    count: usize,
    bytes_received: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 MCP PostgreSQL - HTTP Server Latency Measurement");
    println!("═══════════════════════════════════════════════════\n");

    let client = Client::builder().build()?;

    let base_url = "http://127.0.0.1:3001";

    // Test connection
    println!("Testing connection to {}...", base_url);
    match client.get(format!("{}/health", base_url)).send().await {
        Ok(_) => println!("✅ Server is running\n"),
        Err(_) => {
            println!("❌ Server not running on {}:3001", base_url);
            println!("\nStart the server with:");
            println!("  cargo run --release -- --http-port 3001\n");
            return Ok(());
        }
    }

    // Test cases with different payloads
    let test_cases = vec![
        ("tools/list", json!({}), "List all tools"),
        ("list_tables", json!({}), "List all tables"),
        (
            "describe_table",
            json!({"table": "pg_tables"}),
            "Describe table",
        ),
        ("execute_query", json!({"sql": "SELECT 1"}), "Simple SELECT"),
        (
            "execute_query",
            json!({"sql": "SELECT * FROM pg_tables LIMIT 100"}),
            "Moderate query",
        ),
        (
            "execute_query",
            json!({"sql": "SELECT schemaname, tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) FROM pg_tables ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC LIMIT 50"}),
            "Complex query",
        ),
        ("get_cache_hit_ratio", json!({}), "Cache metrics"),
        ("analyze_db_health", json!({}), "Health check"),
        (
            "get_setting",
            json!({"setting_name": "max_connections"}),
            "Get setting",
        ),
        ("show_current_user", json!({}), "Current user"),
    ];

    println!("📊 Latency Test Cases:\n");

    let mut all_results = BTreeMap::new();

    for (tool_name, args, description) in test_cases {
        println!("Testing: {} - {}", tool_name, description);

        // Warm-up request
        let _ = send_request(&client, base_url, tool_name, &args).await;

        // Measure latencies
        let mut latencies = Vec::new();
        let mut bytes_received = 0;
        let iterations = 50;

        for _ in 0..iterations {
            let start = Instant::now();
            if let Ok(bytes) = send_request(&client, base_url, tool_name, &args).await {
                bytes_received = bytes;
            }
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            latencies.push(elapsed);
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let min = latencies[0];
        let max = latencies[latencies.len() - 1];
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50 = latencies[latencies.len() / 2];
        let p95 = latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)];
        let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];

        let stats = LatencyStats {
            min_ms: min,
            max_ms: max,
            avg_ms: avg,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
            count: iterations,
            bytes_received,
        };

        all_results.insert(tool_name.to_string(), (description, stats));

        println!("  Min:    {:.2}ms", min);
        println!("  Max:    {:.2}ms", max);
        println!("  Avg:    {:.2}ms", avg);
        println!("  P50:    {:.2}ms", p50);
        println!("  P95:    {:.2}ms", p95);
        println!("  P99:    {:.2}ms", p99);
        println!("  Bytes:  {} bytes", bytes_received);
        println!();
    }

    // Concurrent load test
    println!("\n⚡ Concurrent Load Test (20 clients × 10 requests)");
    println!("─────────────────────────────────────────────────");

    let concurrent_client = Arc::new(client.clone());
    let concurrent_latencies = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let mut handles = vec![];

    let start_concurrent = Instant::now();

    for _ in 0..20 {
        let client = concurrent_client.clone();
        let latencies = concurrent_latencies.clone();

        let handle = task::spawn(async move {
            for _ in 0..10 {
                let start = Instant::now();
                let _ = send_request(
                    &client,
                    "http://127.0.0.1:3001",
                    "execute_query",
                    &json!({"sql": "SELECT 1"}),
                )
                .await;
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                latencies.lock().push(elapsed);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    let concurrent_elapsed = start_concurrent.elapsed().as_secs_f64();
    let latencies = concurrent_latencies.lock();

    let mut sorted_latencies: Vec<f64> = latencies.clone();
    sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    println!("Total requests:        200");
    println!("Total time:            {:.2}s", concurrent_elapsed);
    println!("Requests/sec:          {:.0}", 200.0 / concurrent_elapsed);
    println!();

    println!("Min latency:           {:.2}ms", sorted_latencies[0]);
    println!(
        "Max latency:           {:.2}ms",
        sorted_latencies[sorted_latencies.len() - 1]
    );
    println!(
        "Avg latency:           {:.2}ms",
        sorted_latencies.iter().sum::<f64>() / sorted_latencies.len() as f64
    );
    println!(
        "P50 latency:           {:.2}ms",
        sorted_latencies[sorted_latencies.len() / 2]
    );
    println!(
        "P95 latency:           {:.2}ms",
        sorted_latencies[sorted_latencies.len() * 95 / 100]
    );
    println!(
        "P99 latency:           {:.2}ms",
        sorted_latencies[sorted_latencies.len() * 99 / 100]
    );

    // Summary table
    println!("\n📈 Summary Table");
    println!("────────────────────────────────────────────────────────────────────────────────");
    println!(
        "{:<25} {:>10} {:>10} {:>10} {:>10} {:>15}",
        "Tool", "Avg (ms)", "P95 (ms)", "P99 (ms)", "Max (ms)", "Bytes"
    );
    println!("────────────────────────────────────────────────────────────────────────────────");

    for (tool, (_desc, stats)) in &all_results {
        println!(
            "{:<25} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>15}",
            format!("{}", tool),
            stats.avg_ms,
            stats.p95_ms,
            stats.p99_ms,
            stats.max_ms,
            format!("{}", stats.bytes_received)
        );
    }

    println!("────────────────────────────────────────────────────────────────────────────────");

    // Performance classification
    println!("\n🎯 Performance Classification:");
    println!();

    let mut excellent = 0;
    let mut good = 0;
    let mut acceptable = 0;
    let mut slow = 0;

    for (_, stats) in all_results.values() {
        if stats.p95_ms < 10.0 {
            excellent += 1;
        } else if stats.p95_ms < 20.0 {
            good += 1;
        } else if stats.p95_ms < 50.0 {
            acceptable += 1;
        } else {
            slow += 1;
        }
    }

    println!("  ⭐ Excellent  (P95 < 10ms):   {} tools", excellent);
    println!("  ✅ Good       (P95 < 20ms):   {} tools", good);
    println!("  ⚠️  Acceptable (P95 < 50ms):   {} tools", acceptable);
    println!("  ❌ Slow       (P95 ≥ 50ms):   {} tools", slow);

    println!("\n✨ Measurement complete!");

    Ok(())
}

async fn send_request(
    client: &reqwest::Client,
    base_url: &str,
    method: &str,
    params: &serde_json::Value,
) -> Result<usize, Box<dyn std::error::Error>> {
    let url = format!("{}/rpc", base_url);

    let request_body = if method == "tools/list" {
        json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        })
    } else {
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": method,
                "arguments": params
            },
            "id": 1
        })
    };

    let response = client.post(&url).json(&request_body).send().await?;

    let body = response.bytes().await?;
    Ok(body.len())
}
