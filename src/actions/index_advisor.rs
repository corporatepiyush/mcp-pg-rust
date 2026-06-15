use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

pub async fn suggest_indexes(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str()));
    let min_scan_threshold = params.as_ref().and_then(|p| p.get("min_scan_threshold").and_then(|v| v.as_i64())).unwrap_or(100);
    let min_table_size_mb = params.as_ref().and_then(|p| p.get("min_table_size_mb").and_then(|v| v.as_i64())).unwrap_or(10);

    let schema_filter = match schema {
        Some(s) => format!("AND n.nspname = '{}'", s.replace('\'', "''")),
        None => "AND n.nspname NOT IN ('pg_catalog', 'information_schema')".to_string(),
    };

    let rows = client.query(
        &format!(
            "SELECT s.schemaname, s.tablename, s.seq_scan, s.seq_tup_read,
                    s.n_live_tup,
                    pg_total_relation_size(quote_ident(s.schemaname)||'.'||quote_ident(s.tablename)) AS total_size,
                    COALESCE(
                        (SELECT string_agg(a.attname::text, ',' ORDER BY a.attnum)
                         FROM pg_catalog.pg_attribute a
                         JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
                         JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                         WHERE n.nspname = s.schemaname AND c.relname = s.tablename
                           AND a.attnum > 0 AND NOT a.attisdropped
                        ), ''
                    ) AS columns,
                    (SELECT COUNT(*) FROM pg_catalog.pg_index i
                     JOIN pg_catalog.pg_class c ON c.oid = i.indrelid
                     JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                     WHERE n.nspname = s.schemaname AND c.relname = s.tablename
                    ) AS index_count
             FROM pg_stat_user_tables s
             WHERE s.seq_scan > $1
               AND pg_total_relation_size(quote_ident(s.schemaname)||'.'||quote_ident(s.tablename)) > $2 * 1048576
               {}
             ORDER BY s.seq_tup_read DESC
             LIMIT 20",
            schema_filter,
        ),
        &[&min_scan_threshold, &min_table_size_mb],
    ).await?;

    let mut suggestions = Vec::with_capacity(rows.len());
    for row in &rows {
        let schemaname: String = row.get(0);
        let tablename: String = row.get(1);
        let seq_scan: i64 = row.get(2);
        let seq_tup_read: i64 = row.get(3);
        let n_live_tup: Option<i64> = row.get(4);
        let total_size: i64 = row.get(5);
        let columns_str: String = row.get::<_, Option<String>>(6).unwrap_or_default();
        let existing_count: i64 = row.get::<_, i64>(7);

        let columns: Vec<&str> = if columns_str.is_empty() {
            vec!["(unknown)"]
        } else {
            columns_str.split(',').collect()
        };

        let rationale = if existing_count == 0 {
            format!("No indexes exist ({} seq scans)", seq_scan)
        } else {
            format!("High seq scan count ({} scans, {} rows read) on a {:.1} MB table with {} existing index(es)",
                seq_scan, seq_tup_read, total_size as f64 / 1048576.0, existing_count)
        };

        suggestions.push(json!({
            "schema": schemaname,
            "table": tablename,
            "suggested_columns": columns,
            "suggested_ddl": format!(
                "CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_{}_{}_on_{} ON \"{}\".\"{}\" ({});",
                schemaname, tablename, columns[0], schemaname, tablename, columns.join(", "),
            ),
            "rationale": rationale,
            "seq_scans": seq_scan,
            "seq_tup_read": seq_tup_read,
            "table_size_bytes": total_size,
            "existing_indexes": existing_count,
            "estimated_live_rows": n_live_tup,
        }));
    }

    Ok(json!({
        "suggestions": suggestions,
        "total_suggestions": suggestions.len(),
        "note": "Review each suggestion before applying. Index creation affects write performance.",
    }))
}
