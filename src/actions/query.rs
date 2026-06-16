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
    // Reject multi-statement: find the first statement-terminating ';' that is
    // not inside a string literal, quoted identifier, dollar-quoted string, or
    // comment. A single trailing ';' is allowed.
    let body = trimmed.strip_suffix(';').unwrap_or(trimmed);
    if let Some(i) = first_unquoted_semicolon(body) {
        let ctx_end = (i + 20).min(body.len());
        let ctx = body.get(i..ctx_end).unwrap_or("");
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "Multi-statement queries are not allowed: {label} contained ';' at position {i} (context: ...{ctx}...)"
        )));
    }
    Ok(())
}

/// Byte index of the first `;` in `sql` that lies outside any string literal,
/// quoted identifier, dollar-quoted string, or comment. Returns `None` if there
/// is no such terminator.
fn first_unquoted_semicolon(sql: &str) -> Option<usize> {
    let b = sql.as_bytes();
    let n = b.len();
    let mut i = 0;
    while i < n {
        match b[i] {
            b'\'' => {
                // single-quoted string literal; '' is an escaped quote
                i += 1;
                while i < n {
                    if b[i] == b'\'' {
                        if i + 1 < n && b[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'"' => {
                // double-quoted identifier; "" is an escaped quote
                i += 1;
                while i < n {
                    if b[i] == b'"' {
                        if i + 1 < n && b[i + 1] == b'"' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'-' if i + 1 < n && b[i + 1] == b'-' => {
                // line comment to end of line
                i += 2;
                while i < n && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < n && b[i + 1] == b'*' => {
                // block comment (PostgreSQL allows nesting)
                i += 2;
                let mut depth = 1usize;
                while i < n && depth > 0 {
                    if i + 1 < n && b[i] == b'/' && b[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if i + 1 < n && b[i] == b'*' && b[i + 1] == b'/' {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            b'$' => {
                // dollar-quoted string: $tag$ ... $tag$ (tag may be empty)
                let mut j = i + 1;
                while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                if j < n && b[j] == b'$' {
                    let tag = &sql[i..=j]; // includes both $ delimiters
                    match sql[j + 1..].find(tag) {
                        Some(off) => i = j + 1 + off + tag.len(),
                        None => i = n, // unterminated — consume the rest
                    }
                } else {
                    i += 1;
                }
            }
            b';' => return Some(i),
            _ => i += 1,
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unquoted_semicolon_detected() {
        assert_eq!(
            first_unquoted_semicolon("SELECT 1; DROP TABLE x"),
            Some(8)
        );
    }

    #[test]
    fn test_semicolon_in_string_ignored() {
        assert_eq!(first_unquoted_semicolon("SELECT ';not a stmt'"), None);
        assert_eq!(first_unquoted_semicolon("SELECT 'a''b; c'"), None);
    }

    #[test]
    fn test_semicolon_in_identifier_ignored() {
        assert_eq!(first_unquoted_semicolon("SELECT \"weird;col\" FROM t"), None);
    }

    #[test]
    fn test_semicolon_in_comments_ignored() {
        assert_eq!(first_unquoted_semicolon("SELECT 1 -- a; b\n"), None);
        assert_eq!(first_unquoted_semicolon("SELECT 1 /* a; b */"), None);
    }

    #[test]
    fn test_semicolon_in_dollar_quote_ignored() {
        assert_eq!(first_unquoted_semicolon("SELECT $$a; b$$"), None);
        assert_eq!(first_unquoted_semicolon("SELECT $tag$a; b$tag$"), None);
    }

    #[test]
    fn test_validate_sql_allows_trailing_semicolon() {
        assert!(validate_sql("SELECT 1;", "SELECT", "SELECT").is_ok());
        assert!(validate_sql("SELECT ';'", "SELECT", "SELECT").is_ok());
    }

    #[test]
    fn test_validate_sql_rejects_stacked() {
        assert!(validate_sql("SELECT 1; DROP TABLE x", "SELECT", "SELECT").is_err());
    }

    #[test]
    fn test_validate_sql_prefix() {
        assert!(validate_sql("DELETE FROM x WHERE id=1", "DELETE", "DELETE").is_ok());
        assert!(validate_sql("SELECT 1", "DELETE", "DELETE").is_err());
    }
}
