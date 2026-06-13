use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;

/// Benchmark 1: &Option<Value> vs cloned() — measures the allocation cost
/// we eliminated by changing action signatures from Option<Value> to &Option<Value>.
fn bench_value_param(c: &mut Criterion) {
    let mut group = c.benchmark_group("action_params");

    // Payload matching a real request: {"sql": "SELECT 1"}
    let payload = serde_json::json!({"sql": "SELECT 1"});

    group.bench_function("&Option<Value> (reference)", |b| {
        b.iter(|| {
            let params: &Option<serde_json::Value> = &Some(payload.clone());
            let _sql = params
                .as_ref()
                .and_then(|p| p.get("sql"))
                .and_then(|v| v.as_str());
            black_box(_sql);
        });
    });

    group.bench_function("Option<Value> (cloned, old pattern)", |b| {
        b.iter(|| {
            let params: Option<serde_json::Value> = Some(payload.clone());
            let _sql = params
                .as_ref()
                .and_then(|p| p.get("sql"))
                .and_then(|v| v.as_str());
            black_box(_sql);
        });
    });

    group.bench_function("owned + ok_or_else (old batch pattern)", |b| {
        b.iter(|| {
            let params: Option<serde_json::Value> = Some(payload.clone());
            let p = params.ok_or_else(|| "missing").unwrap();
            let _sql = p.get("sql").and_then(|v| v.as_str());
            black_box(_sql);
        });
    });

    group.finish();
}

/// Benchmark 2: thread_local! cached shard vs DefaultHasher on each call
/// This measures the optimization in metrics.rs.
fn bench_metric_shard(c: &mut Criterion) {
    let mut group = c.benchmark_group("metric_shard");

    thread_local! {
        static CACHED_SHARD: usize = {
            let tid = thread::current().id();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&tid, &mut hasher);
            (hasher.finish() as usize) & 15
        };
    }

    group.bench_function("thread_local! cached", |b| {
        b.iter(|| {
            let shard = CACHED_SHARD.with(|s| *s);
            black_box(shard);
        });
    });

    group.bench_function("DefaultHasher every call (old)", |b| {
        b.iter(|| {
            const MASK: usize = 15;
            let tid = thread::current().id();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&tid, &mut hasher);
            let shard = hasher.finish() as usize & MASK;
            black_box(shard);
        });
    });

    group.finish();
}

/// Benchmark 3: matches! macro vs HashMap lookup for tool existence
/// This measures the tool existence check before pool.acquire().
fn bench_tool_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_existence_check");

    let _tool_names: Vec<String> = (0..60).map(|i| format!("tool_{}", i)).collect();

    // Build a HashSet for O(1) lookup
    fn bench_match(tool: &str) -> bool {
        matches!(
            tool,
            "list_tables" | "describe_table" | "list_indexes" | "list_schemas"
                | "show_constraints" | "execute_query" | "execute_insert"
                | "execute_update" | "execute_delete" | "explain_query"
                | "batch_insert" | "batch_update" | "batch_delete" | "batch_insert_copy"
                | "get_table_stats" | "get_index_stats" | "show_database_size"
                | "show_table_size" | "get_cache_hit_ratio" | "list_connections"
                | "kill_connection" | "show_current_user" | "show_running_queries"
                | "show_connection_summary" | "vacuum_analyze" | "analyze_table"
                | "reindex_table" | "get_pg_stat_statements" | "reset_statistics"
                | "list_users" | "list_user_privileges" | "list_role_memberships"
                | "list_database_privileges" | "show_session_info" | "show_all_settings"
                | "get_setting" | "show_memory_settings" | "show_performance_settings"
                | "show_log_settings" | "show_replication_status" | "list_replication_slots"
                | "list_standby_servers" | "show_wal_info" | "show_base_backup_progress"
                | "show_active_transactions" | "show_locks" | "show_waiting_locks"
                | "begin_transaction" | "commit_transaction" | "rollback_transaction"
                | "show_transaction_isolation" | "show_deadlocks" | "show_autocommit_status"
                | "show_transaction_timeout" | "analyze_db_health" | "list_unused_indexes"
                | "list_duplicate_indexes" | "show_vacuum_progress" | "get_object_details"
        )
    }

    fn bench_hashset(tool: &str, set: &std::collections::HashSet<&'static str>) -> bool {
        set.contains(tool)
    }

    // Static set built once
    let hashset: std::collections::HashSet<&'static str> = [
        "list_tables", "describe_table", "list_indexes", "list_schemas",
        "show_constraints", "execute_query", "execute_insert", "execute_update",
        "execute_delete", "explain_query", "batch_insert", "batch_update",
        "batch_delete", "batch_insert_copy", "get_table_stats", "get_index_stats",
        "show_database_size", "show_table_size", "get_cache_hit_ratio",
        "list_connections", "kill_connection", "show_current_user",
        "show_running_queries", "show_connection_summary", "vacuum_analyze",
        "analyze_table", "reindex_table", "get_pg_stat_statements",
        "reset_statistics", "list_users", "list_user_privileges",
        "list_role_memberships", "list_database_privileges", "show_session_info",
        "show_all_settings", "get_setting", "show_memory_settings",
        "show_performance_settings", "show_log_settings", "show_replication_status",
        "list_replication_slots", "list_standby_servers", "show_wal_info",
        "show_base_backup_progress", "show_active_transactions", "show_locks",
        "show_waiting_locks", "begin_transaction", "commit_transaction",
        "rollback_transaction", "show_transaction_isolation", "show_deadlocks",
        "show_autocommit_status", "show_transaction_timeout", "analyze_db_health",
        "list_unused_indexes", "list_duplicate_indexes", "show_vacuum_progress",
        "get_object_details",
    ]
    .into_iter()
    .collect();

    group.bench_function("matches! macro", |b| {
        b.iter(|| {
            // Check a valid tool (fast path hit)
            let result = bench_match(black_box("execute_query"));
            black_box(result);
        });
    });

    group.bench_function("matches! macro unknown tool", |b| {
        b.iter(|| {
            let result = bench_match(black_box("nonexistent_tool_xyz"));
            black_box(result);
        });
    });

    group.bench_function("HashSet contains (O(1))", |b| {
        b.iter(|| {
            let result = bench_hashset(black_box("execute_query"), &hashset);
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark 4: crossbeam SegQueue push/pop (pool core primitive)
fn bench_segqueue(c: &mut Criterion) {
    use crossbeam::queue::SegQueue;

    let mut group = c.benchmark_group("segqueue");

    let q = SegQueue::new();
    for i in 0..100 {
        q.push(Arc::new(i));
    }

    group.bench_function("pop (pool fast path)", |b| {
        b.iter(|| {
            // Push one back so queue never empties
            q.push(Arc::new(42));
            let val = q.pop();
            black_box(val);
        });
    });

    group.bench_function("push (pool release)", |b| {
        b.iter(|| {
            q.push(Arc::new(42));
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark 5: AtomicU32 fetch_add/fetch_sub overhead
fn bench_atomics(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomics_pool");

    let counter = AtomicU32::new(0);

    group.bench_function("fetch_add (active_connections)", |b| {
        b.iter(|| {
            counter.fetch_add(1, Ordering::Relaxed);
            black_box(());
        });
    });

    group.bench_function("fetch_sub (active_connections)", |b| {
        b.iter(|| {
            counter.fetch_sub(1, Ordering::Relaxed);
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark 6: JSON parse_request (the #1 app hotspot identified in profile)
fn bench_json_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parse_request");

    // Realistic request payload
    let request = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":0}"#;

    // Using serde_json::from_str (what parse_request does)
    #[derive(serde::Deserialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        method: String,
        #[allow(dead_code)]
        params: Option<serde_json::Value>,
        id: Option<serde_json::Value>,
    }

    group.bench_function("serde_json::from_str (parse_request)", |b| {
        b.iter(|| {
            let line = black_box(request).trim();
            let req: std::result::Result<JsonRpcRequest, _> = serde_json::from_str(line);
            let _ = black_box(req);
        });
    });

    // Benchmark simd-json if available (for comparison)
    group.bench_function("serde_json raw Value parse", |b| {
        b.iter(|| {
            let line = black_box(request).trim();
            let val: std::result::Result<serde_json::Value, _> = serde_json::from_str(line);
            let _ = black_box(val);
        });
    });

    group.finish();
}

/// Benchmark 7: Notify overhead (pool wait primitive)
fn bench_notify(c: &mut Criterion) {
    use tokio::runtime::Runtime;
    use tokio::sync::Notify;

    let mut group = c.benchmark_group("notify_pool");

    let rt = Runtime::new().unwrap();

    group.bench_function("notify_one (pool release)", |b| {
        let notify = Notify::new();
        b.iter(|| {
            notify.notify_one();
            black_box(());
        });
    });

    group.bench_function("notify_one + notified (acquire/release)", |b| {
        b.iter(|| {
            let notify = std::sync::Arc::new(Notify::new());
            let n2 = notify.clone();
            rt.block_on(async {
                notify.notify_one();
                n2.notified().await;
            });
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_value_param,
    bench_metric_shard,
    bench_tool_check,
    bench_segqueue,
    bench_atomics,
    bench_json_parse,
    bench_notify,
);
criterion_main!(benches);
