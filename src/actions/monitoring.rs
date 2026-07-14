use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

/// 11. Get table stats
pub async fn get_table_stats(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname, relname, n_live_tup, n_dead_tup,
                    last_vacuum::text, last_autovacuum::text
             FROM pg_stat_user_tables
             ORDER BY schemaname, relname",
            &[],
        )
        .await?;

    let stats: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "live_tuples": row.get::<_, i64>(2),
                "dead_tuples": row.get::<_, i64>(3),
                "last_vacuum": row.get::<_, Option<String>>(4),
                "last_autovacuum": row.get::<_, Option<String>>(5),
            })
        })
        .collect();

    Ok(json!({ "tables": stats }))
}

/// 12. Get index stats
pub async fn get_index_stats(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname, relname, indexrelname, idx_scan, idx_tup_read, idx_tup_fetch
             FROM pg_stat_user_indexes
             ORDER BY schemaname, relname, indexrelname",
            &[],
        )
        .await?;

    let stats: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "index": row.get::<_, String>(2),
                "scans": row.get::<_, i64>(3),
                "tuples_read": row.get::<_, i64>(4),
                "tuples_fetched": row.get::<_, i64>(5),
            })
        })
        .collect();

    Ok(json!({ "indexes": stats }))
}

/// 13. Show database size
pub async fn show_database_size(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT datname, pg_size_pretty(pg_database_size(datname)) as size_pretty,
                    pg_database_size(datname) as size_bytes
             FROM pg_database
             WHERE datistemplate = false
             ORDER BY pg_database_size(datname) DESC",
            &[],
        )
        .await?;

    let databases: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "size": row.get::<_, String>(1),
                "size_bytes": row.get::<_, i64>(2),
            })
        })
        .collect();

    Ok(json!({ "databases": databases }))
}

/// 14. Show table size
pub async fn show_table_size(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname, relname,
                    pg_size_pretty(pg_total_relation_size(schemaname||'.'||relname)) as total_size,
                    pg_total_relation_size(schemaname||'.'||relname) as total_bytes
             FROM pg_stat_user_tables
             ORDER BY pg_total_relation_size(schemaname||'.'||relname) DESC",
            &[],
        )
        .await?;

    let tables: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "size": row.get::<_, String>(2),
                "size_bytes": row.get::<_, i64>(3),
            })
        })
        .collect();

    Ok(json!({ "tables": tables }))
}

/// 15. Get cache hit ratio
pub async fn get_cache_hit_ratio(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT CASE
                WHEN (sum(heap_blks_hit) + sum(heap_blks_read)) > 0
                THEN sum(heap_blks_hit)::float / NULLIF(sum(heap_blks_hit) + sum(heap_blks_read), 0)
                ELSE NULL
             END as ratio
             FROM pg_statio_user_tables",
            &[],
        )
        .await?;

    let ratio = if rows.is_empty() {
        None
    } else {
        rows[0].get::<_, Option<f64>>(0)
    };

    Ok(json!({
        "cache_hit_ratio": ratio,
        "percentage": ratio.map(|r| r * 100.0)
    }))
}
