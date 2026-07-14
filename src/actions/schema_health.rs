use crate::errors::Result as MCPResult;
use crate::validation::quote_ident;
use serde_json::{Value, json};
use tokio_postgres::Client;

pub async fn find_tables_without_pk(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()));

    let schema_filter = match schema {
        Some(s) => format!("AND c.table_schema = '{}'", s.replace('\'', "''")),
        None => "AND c.table_schema NOT IN ('pg_catalog', 'information_schema')".to_string(),
    };

    let rows = client.query(
        &format!(
            "SELECT c.table_schema, c.table_name,
                    pg_catalog.pg_size_pretty(pg_total_relation_size(quote_ident(c.table_schema)||'.'||quote_ident(c.table_name))) AS size_pretty,
                    (SELECT COUNT(*) FROM pg_catalog.pg_index i
                     JOIN pg_catalog.pg_class cl ON cl.oid = i.indrelid
                     JOIN pg_catalog.pg_namespace n ON n.oid = cl.relnamespace
                     WHERE n.nspname = c.table_schema AND cl.relname = c.table_name AND i.indisprimary) AS has_pk
             FROM information_schema.tables c
             WHERE c.table_type = 'BASE TABLE'
               {}
               AND NOT EXISTS (
                   SELECT 1 FROM information_schema.table_constraints tc
                   WHERE tc.table_schema = c.table_schema
                     AND tc.table_name = c.table_name
                     AND tc.constraint_type = 'PRIMARY KEY'
               )
             ORDER BY c.table_schema, c.table_name",
            schema_filter,
        ),
        &[],
    ).await?;

    let tables: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "approx_size": row.get::<_, String>(2),
            })
        })
        .collect();

    Ok(json!({
        "tables_without_pk": tables,
        "count": tables.len(),
        "recommendation": "Add a primary key to each table using ALTER TABLE ... ADD PRIMARY KEY (column);"
    }))
}

pub async fn find_missing_fk_indexes(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()));

    let schema_filter = match schema {
        Some(s) => format!("AND ncon.nspname = '{}'", s.replace('\'', "''")),
        None => "AND ncon.nspname NOT IN ('pg_catalog', 'information_schema')".to_string(),
    };

    let rows = client.query(
        &format!(
            "SELECT ncon.nspname AS schema_name,
                    cl.relname AS table_name,
                    a.attname AS fk_column,
                    nref.nspname AS ref_schema,
                    clref.relname AS ref_table,
                    ref_a.attname AS ref_column,
                    con.conname AS constraint_name
             FROM pg_catalog.pg_constraint con
             JOIN pg_catalog.pg_class cl ON cl.oid = con.conrelid
             JOIN pg_catalog.pg_namespace ncon ON ncon.oid = cl.relnamespace
             JOIN pg_catalog.pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = ANY(con.conkey)
             JOIN pg_catalog.pg_class clref ON clref.oid = con.confrelid
             JOIN pg_catalog.pg_namespace nref ON nref.oid = clref.relnamespace
             JOIN pg_catalog.pg_attribute ref_a ON ref_a.attrelid = con.confrelid AND ref_a.attnum = ANY(con.confkey)
             WHERE con.contype = 'f'
               {}
               AND NOT EXISTS (
                   SELECT 1 FROM pg_catalog.pg_index idx
                   WHERE idx.indrelid = con.conrelid
                     AND a.attnum = ANY(idx.indkey)
               )
             ORDER BY ncon.nspname, cl.relname, a.attname",
            schema_filter,
        ),
        &[],
    ).await?;

    let missing: Vec<Value> = rows.iter().map(|row| {
        json!({
            "schema": row.get::<_, String>(0),
            "table": row.get::<_, String>(1),
            "fk_column": row.get::<_, String>(2),
            "references_table": format!("{}.{}", row.get::<_, String>(3), row.get::<_, String>(4)),
            "ref_column": row.get::<_, String>(5),
            "constraint_name": row.get::<_, String>(6),
            "suggestion": format!(
                "CREATE INDEX ON \"{}\".\"{}\" (\"{}\");",
                row.get::<_, String>(0),
                row.get::<_, String>(1),
                row.get::<_, String>(2),
            ),
        })
    }).collect();

    Ok(json!({
        "missing_fk_indexes": missing,
        "count": missing.len(),
        "recommendation": "Foreign key columns without indexes can cause row-level lock contention and slow deletes/updates on the parent table."
    }))
}

pub async fn analyze_table_bloat(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()));
    let threshold = params
        .as_ref()
        .and_then(|p| p.get("threshold").and_then(|v| v.as_f64()))
        .unwrap_or(10.0);

    let schema_filter = match schema {
        Some(s) => format!("AND schemaname = '{}'", s.replace('\'', "''")),
        None => "AND schemaname NOT IN ('pg_catalog', 'information_schema')".to_string(),
    };

    let rows = client.query(
        &format!(
            "SELECT schemaname, tablename,
                    ROUND(CASE
                        WHEN GREATEST(n_live_tup, n_dead_tup) = 0 THEN 0.0
                        ELSE n_dead_tup::numeric / GREATEST(n_live_tup, n_dead_tup) * 100
                    END, 2) AS bloat_pct,
                    n_dead_tup, n_live_tup,
                    pg_size_pretty(pg_total_relation_size(quote_ident(schemaname)||'.'||quote_ident(tablename))) AS total_size,
                    GREATEST(n_dead_tup, 0) * 256 AS estimated_bloat_bytes
             FROM pg_stat_user_tables
             WHERE CASE
                 WHEN GREATEST(n_live_tup, n_dead_tup) = 0 THEN 0.0
                 ELSE n_dead_tup::numeric / GREATEST(n_live_tup, n_dead_tup) * 100
             END > $1
               {}
             ORDER BY bloat_pct DESC
             LIMIT 50",
            schema_filter,
        ),
        &[&threshold],
    ).await?;

    let bloated: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "bloat_percentage": row.get::<_, f64>(2),
                "dead_tuples": row.get::<_, i64>(3),
                "live_tuples": row.get::<_, i64>(4),
                "total_size": row.get::<_, String>(5),
                "estimated_bloat_bytes": row.get::<_, i64>(6),
            })
        })
        .collect();

    Ok(json!({
        "tables": bloated,
        "count": bloated.len(),
        "note": "Consider VACUUM or VACUUM FULL for tables with high bloat."
    }))
}

pub async fn clone_table_schema(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let source_table = params
        .as_ref()
        .and_then(|p| p.get("source_table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'source_table'".into()))?;
    let new_table = params
        .as_ref()
        .and_then(|p| p.get("new_table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'new_table'".into()))?;
    let source_schema = params
        .as_ref()
        .and_then(|p| p.get("source_schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let new_schema = params
        .as_ref()
        .and_then(|p| p.get("new_schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let include_indexes = params
        .as_ref()
        .and_then(|p| p.get("include_indexes").and_then(|v| v.as_bool()))
        .unwrap_or(true);
    let include_defaults = params
        .as_ref()
        .and_then(|p| p.get("include_defaults").and_then(|v| v.as_bool()))
        .unwrap_or(true);
    let include_constraints = params
        .as_ref()
        .and_then(|p| p.get("include_constraints").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let src_ident = format!(
        "{}.{}",
        quote_ident(source_schema),
        quote_ident(source_table)
    );
    let dst_ident = format!("{}.{}", quote_ident(new_schema), quote_ident(new_table));

    let mut parts = vec!["LIKE".to_string(), src_ident];
    if include_indexes {
        parts.push("INCLUDING INDEXES".to_string());
    }
    if include_defaults {
        parts.push("INCLUDING DEFAULTS".to_string());
    }
    if include_constraints {
        parts.push("INCLUDING CONSTRAINTS".to_string());
    }

    let sql = format!("CREATE TABLE {} ({})", dst_ident, parts.join(" "));

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "success": true,
        "source": format!("{}.{}", source_schema, source_table),
        "destination": format!("{}.{}", new_schema, new_table),
        "sql": sql,
        "included": {
            "indexes": include_indexes,
            "defaults": include_defaults,
            "constraints": include_constraints,
        }
    }))
}
