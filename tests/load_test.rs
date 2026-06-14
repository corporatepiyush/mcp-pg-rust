/// HTTP Black Box Load Test for MCP PostgreSQL
/// Measures real throughput with concurrent requests and connection pooling
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::task::JoinSet;

#[tokio::test]
#[ignore]  // Run with: cargo test --test load_test -- --ignored --nocapture
async fn load_test_concurrent_requests() {
    let base_url = "http://127.0.0.1:3001/rpc";

    // Create HTTP client with connection pooling
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(20)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .build()
        .expect("Failed to create HTTP client");

    let client = Arc::new(client);
    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));

    println!("🚀 HTTP Black Box Load Test");
    println!("============================");
    println!("Target: {}", base_url);
    println!("Duration: 10 seconds");
    println!("Concurrent connections: 20");
    println!();

    let start = Instant::now();
    let mut tasks = JoinSet::new();

    // Spawn 20 concurrent worker tasks
    for _worker_id in 0..20 {
        let client = Arc::clone(&client);
        let success = Arc::clone(&success_count);
        let errors = Arc::clone(&error_count);
        let url = base_url.to_string();

        tasks.spawn(async move {
            let worker_start = Instant::now();
            let mut count = 0;

            // Each worker sends requests for 10 seconds
            while worker_start.elapsed().as_secs() < 10 {
                let payload = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "tools/call",
                    "params": {
                        "name": "show_current_user",
                        "arguments": {}
                    },
                    "id": count
                });

                match client
                    .post(&url)
                    .json(&payload)
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            let _ = success.fetch_add(1, Ordering::Relaxed);
                        } else {
                            let _ = errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        let _ = errors.fetch_add(1, Ordering::Relaxed);
                    }
                }

                count += 1;
            }

            count
        });
    }

    // Wait for all workers to complete
    let mut total_requests = 0;
    while let Some(result) = tasks.join_next().await {
        if let Ok(count) = result {
            total_requests += count;
        }
    }

    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let success = success_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let throughput = success as f64 / elapsed_secs;

    println!("📊 Results:");
    println!("===========");
    println!("Duration: {:.2}s", elapsed_secs);
    println!("Total requests: {}", total_requests);
    println!("Successful: {} ({:.1}%)", success, (success as f64 / total_requests as f64) * 100.0);
    println!("Failed: {} ({:.1}%)", errors, (errors as f64 / total_requests as f64) * 100.0);
    println!();
    println!("Throughput: {:.0} req/sec", throughput);
    println!("Baseline: 17,713 req/sec");
    println!();

    // Performance assessment
    if throughput > 15000.0 {
        println!("✅ EXCELLENT (>15K req/sec)");
    } else if throughput > 10000.0 {
        println!("✅ GOOD (10-15K req/sec)");
    } else if throughput > 5000.0 {
        println!("✅ ACCEPTABLE (5-10K req/sec)");
    } else if throughput > 1000.0 {
        println!("⚠️  DEGRADED (1-5K req/sec)");
    } else {
        println!("❌ CRITICAL (<1K req/sec)");
    }

    println!();
    println!("Run with: cargo test --test load_test -- --ignored --nocapture");
}

#[tokio::test]
#[ignore]  // Run with: cargo test --test load_test -- --ignored --nocapture
async fn load_test_sequential_baseline() {
    let base_url = "http://127.0.0.1:3001/rpc";
    let client = reqwest::Client::new();

    println!("🚀 Sequential Baseline Test (for reference)");
    println!("==========================================");
    println!("100 sequential requests");
    println!();

    let start = Instant::now();
    let mut success = 0;
    let mut errors = 0;

    for i in 0..100 {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "show_current_user",
                "arguments": {}
            },
            "id": i
        });

        match client
            .post(base_url)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    success += 1;
                } else {
                    errors += 1;
                }
            }
            Err(_) => {
                errors += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    println!("📊 Results:");
    println!("Successful: {}/100", success);
    println!("Failed: {}/100", errors);
    println!("Time: {}ms", elapsed_ms);
}

#[tokio::test]
#[ignore]
async fn load_test_tool_variations() {
    let base_url = "http://127.0.0.1:3001/rpc";
    let client = Arc::new(reqwest::Client::builder()
        .pool_max_idle_per_host(20)
        .build()
        .expect("Failed to create client"));

    let tools = vec![
        ("show_current_user", serde_json::json!({})),
        ("list_tables", serde_json::json!({})),
        ("execute_query", serde_json::json!({"query": "SELECT 1"})),
    ];

    println!("🚀 Tool Variation Load Test");
    println!("===========================");
    println!("Testing different tools with concurrent load");
    println!();

    for (tool_name, args) in tools {
        let success = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));

        let start = Instant::now();
        let mut tasks = JoinSet::new();

        // 10 concurrent requests for 5 seconds per tool
        for _worker in 0..10 {
            let client = Arc::clone(&client);
            let success = Arc::clone(&success);
            let errors = Arc::clone(&errors);
            let tool = tool_name.to_string();
            let args = args.clone();
            let url = base_url.to_string();

            tasks.spawn(async move {
                let worker_start = Instant::now();
                let mut count = 0;

                while worker_start.elapsed().as_secs() < 5 {
                    let payload = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "tools/call",
                        "params": {
                            "name": tool,
                            "arguments": args
                        },
                        "id": count
                    });

                    match client.post(&url).json(&payload).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                let _ = success.fetch_add(1, Ordering::Relaxed);
                            } else {
                                let _ = errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            let _ = errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    count += 1;
                }
                count
            });
        }

        while tasks.join_next().await.is_some() {}

        let elapsed = start.elapsed().as_secs_f64();
        let total = success.load(Ordering::Relaxed);
        let throughput = total as f64 / elapsed;

        println!("{}: {:.0} req/sec", tool_name, throughput);
    }
}
