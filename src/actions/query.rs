use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

const MAX_SQL_LEN: usize = 10_000;

fn validate_sql(
    sql: &str,
    allowed_prefix: &str,
    label: &str,
) -> std::result::Result<(), crate::errors::MCPError> {
    if sql.is_empty() {
        return Err(crate::errors::MCPError::InvalidParams(
            "'sql' parameter must not be empty".into(),
        ));
    }
    if sql.len() > MAX_SQL_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "SQL exceeds maximum length of {MAX_SQL_LEN} characters (got {})",
            sql.len()
        )));
    }
    let trimmed = sql.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    if !first_word.eq_ignore_ascii_case(allowed_prefix) {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "Invalid {label} query: expected '{allowed_prefix}'"
        )));
    }
    // Reject multi-statement: find the first unquoted ';' that is not trailing
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed);
    let mut in_string = false;
    for (i, ch) in body.char_indices() {
        if ch == '\'' {
            in_string = !in_string;
        }
        if !in_string && ch == ';' {
            let ctx_end = (i + 20).min(sql.len());
            return Err(crate::errors::MCPError::InvalidParams(format!(
                "Multi-statement queries are not allowed: {label} contained ';' at position {i} (context: ...{}...)",
                &sql[i..ctx_end]
            )));
        }
    }
    Ok(())
}

/// 6. Execute query
pub async fn execute_query(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "SELECT", "SELECT")?;

    let rows = client.query(sql, &[]).await?;

    let results: Vec<Value> = rows
        .iter()
        .map(|row| {
            let values: Vec<Value> = (0..row.len())
                .map(|i| {
                    // Try type inference: prefer native JSON types over raw strings
                    row.try_get::<_, bool>(i)
                        .map(|v| json!(v))
                        .or_else(|_| row.try_get::<_, i32>(i).map(|v| json!(v)))
                        .or_else(|_| row.try_get::<_, i64>(i).map(|v| json!(v)))
                        .or_else(|_| row.try_get::<_, f32>(i).map(|v| json!(v)))
                        .or_else(|_| row.try_get::<_, f64>(i).map(|v| json!(v)))
                        .or_else(|_| row.try_get::<_, String>(i).map(Value::String))
                        .or_else(|_| {
                            row.try_get::<_, Option<String>>(i)
                                .map(|v| v.map(Value::String).unwrap_or(Value::Null))
                        })
                        .unwrap_or(Value::Null)
                })
                .collect();
            Value::Array(values)
        })
        .collect();

    Ok(json!({ "rows": results }))
}

/// 7. Execute insert
pub async fn execute_insert(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "INSERT", "INSERT")?;

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 8. Execute update
pub async fn execute_update(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "UPDATE", "UPDATE")?;

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 9. Execute delete
pub async fn execute_delete(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "DELETE", "DELETE")?;

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 10. Explain query
///
/// Supports EXPLAIN with optional ANALYZE, BUFFERS, and FORMAT options.
/// Only SELECT queries can be explained.
pub async fn explain_query(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "SELECT", "SELECT")?;

    let analyze = params
        .as_ref()
        .and_then(|p| p.get("analyze"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let buffers = params
        .as_ref()
        .and_then(|p| p.get("buffers"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let format = params
        .as_ref()
        .and_then(|p| p.get("format"))
        .and_then(|v| v.as_str())
        .unwrap_or("json");

    if format.eq_ignore_ascii_case("xml") {
        return Err(crate::errors::MCPError::InvalidParams(
            "XML format is not supported — use TEXT, YAML, or JSON".into(),
        ));
    }

    let mut explain_sql = String::with_capacity(sql.len() + 80);
    explain_sql.push_str("EXPLAIN (FORMAT ");
    explain_sql.push_str(&format.to_uppercase());
    if analyze {
        explain_sql.push_str(", ANALYZE");
    }
    if buffers {
        explain_sql.push_str(", BUFFERS");
    }
    explain_sql.push_str(") ");
    explain_sql.push_str(sql);

    let rows = client.query(&explain_sql, &[]).await?;

    if rows.is_empty() {
        return Ok(json!({ "plan": null }));
    }

    if format.eq_ignore_ascii_case("json") {
        let plan: serde_json::Value = rows[0].get(0);
        Ok(json!({
            "plan": plan,
            "options": { "analyze": analyze, "buffers": buffers, "format": format }
        }))
    } else {
        let mut plan = String::new();
        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                plan.push('\n');
            }
            plan.push_str(&row.get::<_, String>(0));
        }
        Ok(json!({
            "plan": plan,
            "options": { "analyze": analyze, "buffers": buffers, "format": format }
        }))
    }
}

/// 26. Async execute insert (with synchronous_commit=off for high-volume operations)
///
/// High-performance insert for WHERE predicate affecting more than 100 rows.
/// Disables synchronous_commit temporarily for maximum throughput.
/// Significant performance benefit when WHERE condition matches > 100 rows.
/// Returns rows affected count.
pub async fn async_execute_insert(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "INSERT", "INSERT")?;

    async_sync_commit_execute(client, sql).await
}

/// 27. Async execute update (with synchronous_commit=off for high-volume operations)
///
/// High-performance update for WHERE predicate affecting more than 100 rows.
/// Disables synchronous_commit temporarily for maximum throughput.
/// Significant performance benefit when WHERE condition matches > 100 rows.
/// Always include WHERE clause to prevent accidental updates.
/// Returns rows affected count.
pub async fn async_execute_update(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "UPDATE", "UPDATE")?;

    async_sync_commit_execute(client, sql).await
}

/// 28. Async execute delete (with synchronous_commit=off for high-volume operations)
///
/// High-performance delete for WHERE predicate affecting more than 100 rows.
/// Disables synchronous_commit temporarily for maximum throughput.
/// Significant performance benefit when WHERE condition matches > 100 rows.
/// Always include WHERE clause - deleting without one removes all rows.
/// Returns rows affected count.
pub async fn async_execute_delete(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    validate_sql(sql, "DELETE", "DELETE")?;

    async_sync_commit_execute(client, sql).await
}

/// Execute a DML statement inside a transaction with SET LOCAL synchronous_commit = OFF.
/// The SET LOCAL is scoped to the transaction, so it auto-resets on COMMIT/ROLLBACK,
/// preventing session-state leakage when the connection returns to the pool.
async fn async_sync_commit_execute(client: &Client, sql: &str) -> MCPResult<Value> {
    client.execute("BEGIN", &[]).await?;
    client
        .execute("SET LOCAL synchronous_commit = OFF", &[])
        .await?;
    match client.execute(sql, &[]).await {
        Ok(rows_affected) => {
            client.execute("COMMIT", &[]).await?;
            Ok(json!({ "rows_affected": rows_affected }))
        }
        Err(e) => {
            client.execute("ROLLBACK", &[]).await.ok();
            Err(crate::errors::MCPError::DatabaseError(e))
        }
    }
}
