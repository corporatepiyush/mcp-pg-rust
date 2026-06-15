use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::{MCPError, Result as MCPResult};
use crate::validation::{validate_identifier, quote_ident};

const MAX_BATCH_ROWS: usize = 1000;
const ALLOWED_OPS: &[&str] = &["=", "<", ">", "<=", ">=", "<>", "IN", "LIKE"];

fn format_sql_value(val: &Value) -> String {
    match val {
        Value::String(s) => format!("'{}'", s.replace("'", "''")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Null => "NULL".to_string(),
        Value::Array(_) | Value::Object(_) => format!("'{}'", val.to_string().replace("'", "''")),
    }
}

fn validate_table_columns(table: &str, columns: &[&str]) -> Result<(), MCPError> {
    validate_identifier(table, "table")?;
    for col in columns {
        validate_identifier(col, "column")?;
    }
    Ok(())
}

fn validate_where_clauses(where_clauses: &[Value]) -> Result<Vec<(String, String, &Value)>, MCPError> {
    if where_clauses.is_empty() {
        return Err(MCPError::InvalidParams("'where_clauses' must not be empty".into()));
    }
    let mut parsed = Vec::new();
    for clause in where_clauses {
        let obj = clause.as_object().ok_or_else(|| {
            MCPError::InvalidParams("Each where_clause must be an object with 'column', 'op', and 'value'".into())
        })?;
        let column = obj.get("column").and_then(|v| v.as_str()).ok_or_else(|| {
            MCPError::InvalidParams("Each where_clause must have a string 'column'".into())
        })?;
        let op = obj.get("op").and_then(|v| v.as_str()).ok_or_else(|| {
            MCPError::InvalidParams("Each where_clause must have a string 'op'".into())
        })?;
        let value = obj.get("value").ok_or_else(|| {
            MCPError::InvalidParams("Each where_clause must have a 'value'".into())
        })?;
        validate_identifier(column, "where_clause.column")?;
        if !ALLOWED_OPS.contains(&op) {
            return Err(MCPError::InvalidParams(
                format!("Invalid operator '{op}' — allowed: {}", ALLOWED_OPS.join(", "))
            ));
        }
        parsed.push((column.to_string(), op.to_string(), value));
    }
    Ok(parsed)
}

fn build_where_sql(parsed: &[(String, String, &Value)]) -> String {
    parsed.iter().map(|(col, op, val)| {
        if op == "IN" {
            if let Some(arr) = val.as_array() {
                let items: Vec<String> = arr.iter().map(format_sql_value).collect();
                format!("{} IN ({})", quote_ident(col), items.join(", "))
            } else {
                format!("{} {} {}", quote_ident(col), op, format_sql_value(val))
            }
        } else {
            format!("{} {} {}", quote_ident(col), op, format_sql_value(val))
        }
    }).collect::<Vec<_>>().join(" OR ")
}

/// Batch insert - high performance multi-row insertion
/// Uses SET LOCAL inside a transaction to avoid session-level side effects.
pub async fn async_batch_insert(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table'".into()))?;

    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'columns'".into()))?;

    let rows = params
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'rows'".into()))?;

    if rows.is_empty() {
        return Ok(json!({ "rows_affected": 0 }));
    }

    if rows.len() > MAX_BATCH_ROWS {
        return Err(MCPError::InvalidParams(
            format!("Batch size exceeds maximum of {MAX_BATCH_ROWS} rows (got {})", rows.len())
        ));
    }

    let returning = params.get("returning").and_then(|v| v.as_str());

    let column_count = columns.len();
    let column_names: Vec<&str> = columns.iter().filter_map(|c| c.as_str()).collect();

    if column_names.len() != column_count {
        return Err(MCPError::InvalidParams("All column names must be strings".into()));
    }

    validate_table_columns(table, &column_names)?;

    let quoted_table = quote_ident(table);
    let quoted_cols: Vec<String> = column_names.iter().map(|c| quote_ident(c)).collect();
    let cols = quoted_cols.join(", ");

    let mut sql = String::with_capacity(64 + cols.len() + rows.len() * (column_count * 16 + 4));
    use std::fmt::Write;
    write!(sql, "INSERT INTO {quoted_table} ({cols}) VALUES ").unwrap();

    for (i, row) in rows.iter().enumerate() {
        let row_array = row.as_array().ok_or_else(|| {
            MCPError::InvalidParams("Each row must be an array".into())
        })?;

        if row_array.len() != column_count {
            return Err(MCPError::InvalidParams(
                format!("Row {} has {} columns, expected {}", i, row_array.len(), column_count),
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

    client.execute("BEGIN", &[]).await?;
    client.execute("SET LOCAL synchronous_commit = OFF", &[]).await?;

    let result = if let Some(col) = returning {
        validate_identifier(col, "returning")?;
        let r = format!(" RETURNING {}", quote_ident(col));
        sql.push_str(&r);
        match client.query(&sql, &[]).await {
            Ok(rows) => {
                client.execute("COMMIT", &[]).await?;
                let ids: Vec<Value> = rows.iter().map(|r| {
                    r.try_get::<_, i64>(0).map(|id| json!(id))
                        .or_else(|_| r.try_get::<_, i32>(0).map(|id| json!(id)))
                        .unwrap_or(json!(null))
                }).collect();
                json!({ "rows_affected": ids.len(), "inserted_ids": ids })
            }
            Err(e) => {
                client.execute("ROLLBACK", &[]).await.ok();
                return Err(MCPError::DatabaseError(e));
            }
        }
    } else {
        match client.execute(&sql, &[]).await {
            Ok(rows_affected) => {
                client.execute("COMMIT", &[]).await?;
                json!({ "rows_affected": rows_affected })
            }
            Err(e) => {
                client.execute("ROLLBACK", &[]).await.ok();
                return Err(MCPError::DatabaseError(e));
            }
        }
    };

    Ok(result)
}

/// Batch update - bulk updates with structured WHERE conditions
pub async fn async_batch_update(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table'".into()))?;

    let updates = params
        .get("updates")
        .and_then(|v| v.as_object())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'updates'".into()))?;

    let where_clauses = params
        .get("where_clauses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'where_clauses'".into()))?;

    validate_identifier(table, "table")?;
    let parsed_where = validate_where_clauses(where_clauses)?;

    let quoted_table = quote_ident(table);
    let mut set_clauses = Vec::new();
    for (key, val) in updates {
        validate_identifier(key, "updates key")?;
        set_clauses.push(format!("{} = {}", quote_ident(key), format_sql_value(val)));
    }

    let where_sql = build_where_sql(&parsed_where);
    let sql = format!("UPDATE {quoted_table} SET {} WHERE {where_sql}", set_clauses.join(", "));

    let rows_affected = client.execute(&sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// Batch delete - bulk deletion with structured WHERE conditions
pub async fn async_batch_delete(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table'".into()))?;

    let where_clauses = params
        .get("where_clauses")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'where_clauses'".into()))?;

    validate_identifier(table, "table")?;
    let parsed_where = validate_where_clauses(where_clauses)?;

    let returning = params.get("returning").and_then(|v| v.as_str());

    let quoted_table = quote_ident(table);
    let where_sql = build_where_sql(&parsed_where);
    let mut sql = format!("DELETE FROM {quoted_table} WHERE {where_sql}");

    if let Some(col) = returning {
        validate_identifier(col, "returning")?;
        sql.push_str(&format!(" RETURNING {}", quote_ident(col)));
        let rows = client.query(&sql, &[]).await?;
        let ids: Vec<Value> = rows.iter().map(|r| {
            r.try_get::<_, i64>(0).map(|id| json!(id))
                .or_else(|_| r.try_get::<_, i32>(0).map(|id| json!(id)))
                .unwrap_or(json!(null))
        }).collect();
        Ok(json!({ "rows_affected": ids.len(), "inserted_ids": ids }))
    } else {
        let rows_affected = client.execute(&sql, &[]).await?;
        Ok(json!({ "rows_affected": rows_affected }))
    }
}

/// Batch insert with auto-batching for massive loads
pub async fn async_batch_insert_copy(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let params = params.as_ref().ok_or_else(|| {
        MCPError::InvalidParams("Missing parameters".into())
    })?;

    let table = params
        .get("table")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table'".into()))?;

    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'columns'".into()))?;

    let rows = params
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'rows'".into()))?;

    let batch_size = params
        .get("batch_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000) as usize;

    if rows.is_empty() {
        return Ok(json!({"rows_affected": 0}));
    }

    if rows.len() > MAX_BATCH_ROWS {
        return Err(MCPError::InvalidParams(
            format!("Batch size exceeds maximum of {MAX_BATCH_ROWS} rows (got {})", rows.len())
        ));
    }

    let column_names: Vec<&str> = columns.iter().filter_map(|c| c.as_str()).collect();
    validate_table_columns(table, &column_names)?;

    let quoted_table = quote_ident(table);
    let quoted_cols: Vec<String> = column_names.iter().map(|c| quote_ident(c)).collect();

    let mut total_affected = 0u64;

    for batch in rows.chunks(batch_size) {
        let mut sql = format!("INSERT INTO {quoted_table} ({}) VALUES ", quoted_cols.join(", "));
        let mut value_parts = Vec::new();

        for row in batch {
            let row_array = row.as_array().ok_or_else(|| {
                MCPError::InvalidParams("Each row must be an array".into())
            })?;

            let row_values: Vec<String> = row_array.iter().map(format_sql_value).collect();
            value_parts.push(format!("({})", row_values.join(", ")));
        }

        sql.push_str(&value_parts.join(", "));

        let rows_affected = client.execute(&sql, &[]).await?;
        total_affected += rows_affected;
    }

    #[allow(clippy::cast_precision_loss)]
    let batches = (rows.len() as f64 / batch_size as f64).ceil() as u32;
    Ok(json!({
        "rows_affected": total_affected,
        "batches": batches,
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

    #[test]
    fn test_validate_table_columns_rejects_injection() {
        let result = validate_table_columns("users; DROP TABLE", &["id"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid character"));
    }

    #[test]
    fn test_validate_table_columns_rejects_sql_in_column() {
        let result = validate_table_columns("users", &["id; DROP TABLE users"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_table_columns_accepts_valid() {
        assert!(validate_table_columns("users", &["id", "name"]).is_ok());
    }

    #[test]
    fn test_validate_where_clauses_accepts_structured() {
        let clauses = vec![
            json!({"column": "id", "op": "=", "value": 1}),
            json!({"column": "status", "op": "IN", "value": ["active", "pending"]}),
        ];
        let result = validate_where_clauses(&clauses);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_where_clauses_rejects_invalid_op() {
        let clauses = vec![
            json!({"column": "id", "op": "EXECUTE", "value": "malicious"}),
        ];
        let result = validate_where_clauses(&clauses);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid operator"));
    }

    #[test]
    fn test_validate_where_clauses_rejects_sql_in_column() {
        let clauses = vec![
            json!({"column": "id; DROP TABLE", "op": "=", "value": 1}),
        ];
        let result = validate_where_clauses(&clauses);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_where_sql() {
        let v1 = Value::Number(1.into());
        let v2 = Value::String("active".into());
        let parsed = vec![
            ("id".to_string(), "=".to_string(), &v1),
            ("status".to_string(), "=".to_string(), &v2),
        ];
        let sql = build_where_sql(&parsed);
        assert_eq!(sql, r#""id" = 1 OR "status" = 'active'"#);
    }

    #[test]
    fn test_build_where_sql_in_op() {
        let values = json!(["a", "b"]);
        let parsed = vec![
            ("status".to_string(), "IN".to_string(), &values),
        ];
        let sql = build_where_sql(&parsed);
        assert_eq!(sql, r#""status" IN ('a', 'b')"#);
    }
}
