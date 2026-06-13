use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_BATCH_ROWS: usize = 1000;
const MAX_IDENTIFIER_LEN: usize = 255;

/// Format JSON value as SQL-safe string
fn format_sql_value(val: &Value) -> String {
    match val {
        Value::String(s) => format!("'{}'", s.replace("'", "''")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Null => "NULL".to_string(),
        Value::Array(_) | Value::Object(_) => format!("'{}'", val.to_string().replace("'", "''")),
    }
}

/// Batch insert - high performance multi-row insertion
/// Applies synchronous_commit = OFF at query level for maximum throughput during bulk loads
pub async fn batch_insert(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        crate::errors::MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;

    if table.is_empty() || table.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'table' must be 1-{MAX_IDENTIFIER_LEN} characters")
        ));
    }

    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'columns'".into()))?;

    let rows = params
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'rows'".into()))?;

    if rows.is_empty() {
        return Ok(json!({ "rows_affected": 0 }));
    }

    if rows.len() > MAX_BATCH_ROWS {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("Batch size exceeds maximum of {MAX_BATCH_ROWS} rows (got {})", rows.len())
        ));
    }

    let returning = params.get("returning").and_then(|v| v.as_str());

    let column_count = columns.len();
    let column_names: Vec<&str> = columns
        .iter()
        .filter_map(|c| c.as_str())
        .collect();

    if column_names.len() != column_count {
        return Err(crate::errors::MCPError::InvalidParams(
            "All column names must be strings".into(),
        ));
    }

    // Build VALUES clause
    let cols = column_names.join(", ");
    let total_capacity = 64 + cols.len() + rows.len() * (column_count * 16 + 4);
    let mut sql = String::with_capacity(total_capacity);
    use std::fmt::Write;
    write!(sql, "INSERT INTO {table} ({cols}) VALUES ").unwrap();

    for (i, row) in rows.iter().enumerate() {
        let row_array = row.as_array().ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Each row must be an array".into())
        })?;

        if row_array.len() != column_count {
            return Err(crate::errors::MCPError::InvalidParams(
                format!("Row has {} columns, expected {}", row_array.len(), column_count),
            ));
        }

        if i > 0 {
            sql.push(',');
        }
        sql.push('(');
        for (j, val) in row_array.iter().enumerate() {
            if j > 0 {
                sql.push_str(", ");
            }
            match val {
                Value::String(s) => {
                    sql.push('\'');
                    for ch in s.chars() {
                        if ch == '\'' {
                            sql.push_str("''");
                        } else {
                            sql.push(ch);
                        }
                    }
                    sql.push('\'');
                }
                Value::Number(n) => {
                    write!(sql, "{n}").unwrap();
                }
                Value::Bool(b) => {
                    sql.push_str(if *b { "true" } else { "false" });
                }
                Value::Null => {
                    sql.push_str("NULL");
                }
                Value::Array(_) | Value::Object(_) => {
                    let s = val.to_string();
                    sql.push('\'');
                    for ch in s.chars() {
                        if ch == '\'' {
                            sql.push_str("''");
                        } else {
                            sql.push(ch);
                        }
                    }
                    sql.push('\'');
                }
            }
        }
        sql.push(')');
    }

    // Temporarily disable synchronous commit for bulk insert throughput,
    // then restore the original setting to avoid session-level side effects.
    let orig_sync = client
        .query_one("SHOW synchronous_commit", &[])
        .await
        .map(|r| r.get::<_, String>(0))
        .unwrap_or_else(|_| "on".to_string());
    client.execute("SET synchronous_commit = OFF", &[]).await?;

    let result = if let Some(col) = returning {
        let r = format!(" RETURNING {}", col);
        sql.push_str(&r);
        let rows = client.query(&sql, &[]).await;
        client
            .execute(&format!("SET synchronous_commit = {}", orig_sync), &[])
            .await
            .ok();
        let rows = rows?;
        let ids: Vec<Value> = rows.iter().map(|r| {
            if let Ok(id) = r.try_get::<_, i64>(0) {
                json!(id)
            } else if let Ok(id) = r.try_get::<_, i32>(0) {
                json!(id)
            } else {
                json!(null)
            }
        }).collect();
        json!({
            "rows_affected": ids.len(),
            "inserted_ids": ids
        })
    } else {
        let rows_affected = client.execute(&sql, &[]).await;
        client
            .execute(&format!("SET synchronous_commit = {}", orig_sync), &[])
            .await
            .ok();
        json!({
            "rows_affected": rows_affected?
        })
    };

    Ok(result)
}

/// Batch update - bulk updates with WHERE conditions
pub async fn batch_update(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        crate::errors::MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;

    if table.is_empty() || table.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'table' must be 1-{MAX_IDENTIFIER_LEN} characters")
        ));
    }

    let updates = params
        .get("updates")
        .and_then(|v| v.as_object())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'updates'".into()))?;

    let where_clauses = params
        .get("where_clauses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'where_clauses'".into()))?;

    if where_clauses.is_empty() {
        return Ok(json!({ "rows_affected": 0 }));
    }

    let mut total_affected = 0u64;

    for where_clause in where_clauses {
        let where_str = where_clause
            .as_str()
            .ok_or_else(|| crate::errors::MCPError::InvalidParams("Where clause must be string".into()))?;

        let mut set_clauses = Vec::new();
        for (key, val) in updates {
            let val_str = format_sql_value(val);
            set_clauses.push(format!("{} = {}", key, val_str));
        }

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            table,
            set_clauses.join(", "),
            where_str
        );

        let rows_affected = client.execute(&sql, &[]).await?;
        total_affected += rows_affected;
    }

    Ok(json!({
        "rows_affected": total_affected
    }))
}

/// Batch delete - bulk deletion with combined WHERE clauses
pub async fn batch_delete(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        crate::errors::MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;

    if table.is_empty() || table.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'table' must be 1-{MAX_IDENTIFIER_LEN} characters")
        ));
    }

    let where_clauses = params
        .get("where_clauses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'where_clauses'".into()))?;

    if where_clauses.is_empty() {
        return Ok(json!({ "rows_affected": 0 }));
    }

    let returning = params.get("returning").and_then(|v| v.as_str());

    let where_conditions: Vec<String> = where_clauses
        .iter()
        .filter_map(|c| c.as_str().map(|s| format!("({})", s)))
        .collect();

    let mut sql = format!(
        "DELETE FROM {} WHERE {}",
        table,
        where_conditions.join(" OR ")
    );

    if let Some(col) = returning {
        sql.push_str(&format!(" RETURNING {}", col));
        let rows = client.query(&sql, &[]).await?;
        let ids: Vec<Value> = rows.iter().map(|r| {
            if let Ok(id) = r.try_get::<_, i64>(0) {
                json!(id)
            } else if let Ok(id) = r.try_get::<_, i32>(0) {
                json!(id)
            } else {
                json!(null)
            }
        }).collect();
        Ok(json!({
            "rows_affected": ids.len(),
            "inserted_ids": ids
        }))
    } else {
        let rows_affected = client.execute(&sql, &[]).await?;
        Ok(json!({
            "rows_affected": rows_affected
        }))
    }
}

/// Batch insert with auto-batching for massive loads
pub async fn batch_insert_copy(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        crate::errors::MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;

    if table.is_empty() || table.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'table' must be 1-{MAX_IDENTIFIER_LEN} characters")
        ));
    }

    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'columns'".into()))?;

    let rows = params
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'rows'".into()))?;

    let batch_size = params
        .get("batch_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as usize;

    if rows.is_empty() {
        return Ok(json!({"rows_affected": 0}));
    }

    if rows.len() > MAX_BATCH_ROWS {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("Batch size exceeds maximum of {MAX_BATCH_ROWS} rows (got {})", rows.len())
        ));
    }

    let column_names: Vec<&str> = columns
        .iter()
        .filter_map(|c| c.as_str())
        .collect();

    let mut total_affected = 0u64;

    // Process in batches
    for batch in rows.chunks(batch_size) {
        let mut sql = format!("INSERT INTO {} ({}) VALUES ", table, column_names.join(", "));
        let mut value_parts = Vec::new();

        for row in batch {
            let row_array = row.as_array().ok_or_else(|| {
                crate::errors::MCPError::InvalidParams("Each row must be an array".into())
            })?;

            let row_values: Vec<String> = row_array
                .iter()
                .map(|v| format_sql_value(v))
                .collect();

            value_parts.push(format!("({})", row_values.join(", ")));
        }

        sql.push_str(&value_parts.join(", "));

        let rows_affected = client.execute(&sql, &[]).await?;
        total_affected += rows_affected;
    }

    Ok(json!({
        "rows_affected": total_affected,
        "batches": (rows.len() as f64 / batch_size as f64).ceil() as u32
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sql_value() {
        assert_eq!(format_sql_value(&Value::String("test".into())), "'test'");
        assert_eq!(format_sql_value(&Value::Number(123.into())), "123");
        assert_eq!(format_sql_value(&Value::Bool(true)), "true");
        assert_eq!(format_sql_value(&Value::Null), "NULL");
    }

    #[test]
    fn test_sql_injection_prevention() {
        let malicious = Value::String("'; DROP TABLE users; --".into());
        let result = format_sql_value(&malicious);
        assert_eq!(result, "'''; DROP TABLE users; --'");
    }
}
