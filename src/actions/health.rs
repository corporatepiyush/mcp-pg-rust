use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

pub async fn analyze_db_health(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let hit_ratio = client
        .query_one(
            "SELECT COALESCE(round(sum(blks_hit)::numeric / nullif(sum(blks_hit + blks_read), 0) * 100, 2)::float8, 0)
             FROM pg_stat_database WHERE datname NOT IN ('template0', 'template1')",
            &[],
        )
        .await
        .map(|r| r.get::<_, f64>(0))
        .unwrap_or(0.0);

    let connections = client
        .query_one(
            "SELECT count(*) FROM pg_stat_activity WHERE state IS NOT NULL",
            &[],
        )
        .await
        .map(|r| r.get::<_, i64>(0))
        .unwrap_or(0);
    let max_connections = client
        .query_one(
            "SELECT setting::int FROM pg_settings WHERE name = 'max_connections'",
            &[],
        )
        .await
        .map(|r| r.get::<_, i32>(0))
        .unwrap_or(100);
    #[allow(clippy::cast_precision_loss)]
    let conn_usage_pct = if max_connections > 0 {
        (connections as f64 / f64::from(max_connections)) * 100.0
    } else {
        0.0
    };

    let idle_in_xact = client
        .query_one(
            "SELECT count(*) FROM pg_stat_activity WHERE state = 'idle in transaction'",
            &[],
        )
        .await
        .map(|r| r.get::<_, i64>(0))
        .unwrap_or(0);

    let waiting = client
        .query_one(
            "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type IS NOT NULL AND state IS NOT NULL",
            &[],
        )
        .await
        .map(|r| r.get::<_, i64>(0))
        .unwrap_or(0);

    let unused_indexes = client
        .query(
            "SELECT schemaname || '.' || indexrelname::text AS name, idx_scan, idx_tup_read, idx_tup_fetch
             FROM pg_stat_user_indexes
             WHERE idx_scan = 0 AND idx_tup_read = 0
             ORDER BY schemaname, indexrelname
             LIMIT 20",
            &[],
        )
        .await
        .map(|r| {
            r.iter().map(|row| json!({
                "index": row.get::<_, String>(0),
                "scans": row.get::<_, i64>(1),
            })).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let duplicate_indexes = client
        .query(
            "SELECT tablename::text, array_agg(indexname::text ORDER BY indexname) AS indexes
             FROM pg_indexes
             GROUP BY tablename
             HAVING count(*) > 1
             ORDER BY count(*) DESC, tablename
             LIMIT 20",
            &[],
        )
        .await
        .map(|r| {
            r.iter()
                .map(|row| {
                    json!({
                        "table": row.get::<_, String>(0),
                        "indexes": row.get::<_, Vec<String>>(1),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Note: `max_dead_tuple_bytes` only exists in PG 17+. In PG 16- it's `max_dead_tuples`.
    // We try PG 17+ columns first, fall back gracefully.
    let vacuum_progress = client
        .query(
            "SELECT n.nspname::text, c.relname::text, p.phase::text,
                    p.heap_blks_total, p.heap_blks_scanned, p.heap_blks_vacuumed,
                    p.index_vacuum_count
             FROM pg_stat_progress_vacuum p
             JOIN pg_class c ON p.relid = c.oid
             JOIN pg_namespace n ON c.relnamespace = n.oid
             ORDER BY n.nspname, c.relname",
            &[],
        )
        .await
        .map(|r| {
            r.iter()
                .map(|row| {
                    json!({
                        "schema": row.get::<_, String>(0),
                        "table": row.get::<_, String>(1),
                        "phase": row.get::<_, String>(2),
                        "blocks_total": row.get::<_, i64>(3),
                        "blocks_scanned": row.get::<_, i64>(4),
                        "blocks_vacuumed": row.get::<_, i64>(5),
                        "index_vacuum_count": row.get::<_, i64>(6),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let txn_wraparound = client
        .query(
            "SELECT relname::text, n_dead_tup, n_live_tup,
                    round(100 * n_dead_tup / nullif(n_live_tup + n_dead_tup, 0)::numeric, 2)::float8 AS dead_pct
             FROM pg_stat_user_tables
             WHERE n_dead_tup > 0 AND (n_live_tup + n_dead_tup) > 0
             ORDER BY dead_pct DESC
             LIMIT 10",
            &[],
        )
        .await
        .map(|r| {
            r.iter().map(|row| json!({
                "table": row.get::<_, String>(0),
                "dead_tuples": row.get::<_, i64>(1),
                "live_tuples": row.get::<_, i64>(2),
                "dead_pct": row.get::<_, f64>(3),
            })).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let seq_scan_tables = client
        .query(
            "SELECT schemaname::text, relname::text, seq_scan, seq_tup_read, n_live_tup
             FROM pg_stat_user_tables
             WHERE seq_scan > 1000 AND n_live_tup > 10000
             ORDER BY seq_scan DESC
             LIMIT 10",
            &[],
        )
        .await
        .map(|r| {
            r.iter()
                .map(|row| {
                    json!({
                        "schema": row.get::<_, String>(0),
                        "table": row.get::<_, String>(1),
                        "sequential_scans": row.get::<_, i64>(2),
                        "rows_read": row.get::<_, i64>(3),
                        "estimated_rows": row.get::<_, f64>(4),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(json!({
        "buffer_cache": {
            "hit_ratio_pct": hit_ratio,
            "status": if hit_ratio >= 99.0 { "healthy" } else if hit_ratio >= 95.0 { "fair" } else { "poor" }
        },
        "connections": {
            "active": connections,
            "max": max_connections,
            "utilization_pct": conn_usage_pct,
            "idle_in_transaction": idle_in_xact,
            "waiting": waiting,
            "status": if conn_usage_pct > 80.0 { "high" } else if conn_usage_pct > 50.0 { "moderate" } else { "healthy" }
        },
        "indexes": {
            "unused": unused_indexes,
            "duplicate_candidates": duplicate_indexes,
            "total_unused": unused_indexes.len()
        },
        "vacuum": {
            "in_progress": vacuum_progress,
            "tables_needing_vacuum": txn_wraparound
        },
        "performance": {
            "tables_with_high_seq_scans": seq_scan_tables
        }
    }))
}

pub async fn list_unused_indexes(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname::text, indexrelname::text, relname::text, idx_scan, idx_tup_read, idx_tup_fetch
             FROM pg_stat_user_indexes
             WHERE idx_scan = 0
             ORDER BY schemaname, relname, indexrelname",
            &[],
        )
        .await?;

    let indexes: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "index": row.get::<_, String>(1),
                "table": row.get::<_, String>(2),
                "scans": row.get::<_, i64>(3),
                "tuples_read": row.get::<_, i64>(4),
                "tuples_fetched": row.get::<_, i64>(5),
            })
        })
        .collect();

    Ok(json!({
        "unused_indexes": indexes,
        "count": indexes.len(),
        "warning": "Indexes with 0 scans may be unused — consider dropping them to reduce write overhead"
    }))
}

pub async fn list_duplicate_indexes(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT a.schemaname::text, a.relname::text,
                    a.indexrelname::text AS index_name,
                    b.indexrelname::text AS duplicate_of,
                    pg_size_pretty(pg_relation_size(a.indexrelid)) AS size
             FROM pg_stat_user_indexes a
             JOIN pg_index pai ON a.indexrelid = pai.indexrelid
             JOIN pg_stat_user_indexes b ON a.schemaname = b.schemaname
                 AND a.relname = b.relname
                 AND a.indexrelid <> b.indexrelid
             JOIN pg_index pbi ON b.indexrelid = pbi.indexrelid
                AND pai.indisprimary = pbi.indisprimary
                AND pai.indisunique = pbi.indisunique
             ORDER BY a.schemaname, a.relname, a.indexrelname",
            &[],
        )
        .await?;

    let duplicates: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "index": row.get::<_, String>(2),
                "duplicate_of": row.get::<_, String>(3),
                "size": row.get::<_, String>(4),
            })
        })
        .collect();

    Ok(json!({
        "duplicate_indexes": duplicates,
        "count": duplicates.len(),
    }))
}

pub async fn show_vacuum_progress(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    // `max_dead_tuple_bytes` is PG 17+. On PG 16- the column is `max_dead_tuples`.
    // Try PG 17+ query first; fall back to PG 16- columns.
    let result = client
        .query(
            "SELECT n.nspname::text, c.relname::text, p.phase::text,
                    p.heap_blks_total, p.heap_blks_scanned, p.heap_blks_vacuumed,
                    p.heap_blks_total - p.heap_blks_scanned AS blks_remaining,
                    round(100.0 * p.heap_blks_scanned / nullif(p.heap_blks_total, 0)::numeric, 1)::float8 AS progress_pct,
                    p.index_vacuum_count, p.max_dead_tuple_bytes
             FROM pg_stat_progress_vacuum p
             JOIN pg_class c ON p.relid = c.oid
             JOIN pg_namespace n ON c.relnamespace = n.oid
             ORDER BY n.nspname, c.relname",
            &[],
        )
        .await;

    let rows = match result {
        Ok(rows) => rows,
        Err(_) => {
            // Fallback: PG 16- uses max_dead_tuples instead of max_dead_tuple_bytes
            client
                .query(
                    "SELECT n.nspname::text, c.relname::text, p.phase::text,
                            p.heap_blks_total, p.heap_blks_scanned, p.heap_blks_vacuumed,
                            p.heap_blks_total - p.heap_blks_scanned AS blks_remaining,
                            round(100.0 * p.heap_blks_scanned / nullif(p.heap_blks_total, 0)::numeric, 1)::float8 AS progress_pct,
                            p.index_vacuum_count, p.max_dead_tuples
                     FROM pg_stat_progress_vacuum p
                     JOIN pg_class c ON p.relid = c.oid
                     JOIN pg_namespace n ON c.relnamespace = n.oid
                     ORDER BY n.nspname, c.relname",
                    &[],
                )
                .await?
        }
    };

    if rows.is_empty() {
        return Ok(json!({
            "vacuum_in_progress": false,
            "message": "No VACUUM operations currently in progress"
        }));
    }

    let operations: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "phase": row.get::<_, String>(2),
                "blocks_total": row.get::<_, i64>(3),
                "blocks_scanned": row.get::<_, i64>(4),
                "blocks_vacuumed": row.get::<_, i64>(5),
                "blocks_remaining": row.get::<_, i64>(6),
                "progress_pct": row.get::<_, f64>(7),
                "index_vacuum_count": row.get::<_, i64>(8),
                "max_dead_tuples": row.get::<_, Option<i64>>(9),
            })
        })
        .collect();

    Ok(json!({
        "vacuum_in_progress": true,
        "operations": operations
    }))
}
