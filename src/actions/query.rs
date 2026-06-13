use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 6. Execute query
pub async fn execute_query(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    // Only allow SELECT queries
    let first_word = sql.trim_start().split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "SELECT" {
        return Err(crate::errors::MCPError::InvalidParams("Only SELECT queries allowed".into()));
    }

    let rows = client.query(sql, &[]).await?;

    let results: Vec<Value> = rows
        .iter()
        .map(|row| {
            let values: Vec<Value> = (0..row.len())
                .map(|i| {
                    // Try type inference: prefer native JSON types over raw strings
                    if let Ok(v) = row.try_get::<_, bool>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, i32>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, i64>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, f32>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, f64>(i) {
                        json!(v)
                    } else if let Ok(v) = row.try_get::<_, String>(i) {
                        Value::String(v)
                    } else if let Ok(v) = row.try_get::<_, Option<String>>(i) {
                        v.map(Value::String).unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                })
                .collect();
            Value::Array(values)
        })
        .collect();

    Ok(json!({ "rows": results }))
}

/// 7. Execute insert
pub async fn execute_insert(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    let first_word = sql.trim_start().split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "INSERT" {
        return Err(crate::errors::MCPError::InvalidParams("Invalid INSERT query".into()));
    }

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 8. Execute update
pub async fn execute_update(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    let first_word = sql.trim_start().split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "UPDATE" {
        return Err(crate::errors::MCPError::InvalidParams("Invalid UPDATE query".into()));
    }

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 9. Execute delete
pub async fn execute_delete(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    let first_word = sql.trim_start().split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "DELETE" {
        return Err(crate::errors::MCPError::InvalidParams("Invalid DELETE query".into()));
    }

    let rows_affected = client.execute(sql, &[]).await?;

    Ok(json!({ "rows_affected": rows_affected }))
}

/// 10. Explain query
///
/// Only SELECT queries can be explained. This guard prevents accidental
/// execution of DDL/DML statements inside EXPLAIN wrappers.
pub async fn explain_query(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let sql = params
        .as_ref()
        .and_then(|p| p.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'sql' parameter".into()))?;

    let first_word = sql.trim_start().split_whitespace().next().unwrap_or("").to_uppercase();
    if first_word != "SELECT" {
        return Err(crate::errors::MCPError::InvalidParams("Only SELECT queries can be explained".into()));
    }

    let mut explain_sql = String::with_capacity(sql.len() + 24);
    explain_sql.push_str("EXPLAIN (FORMAT JSON) ");
    explain_sql.push_str(sql);

    let rows = client.query(&explain_sql, &[]).await?;

    if rows.is_empty() {
        return Ok(json!({ "plan": null }));
    }

    let plan: String = rows[0].get(0);
    Ok(json!({ "plan": serde_json::from_str::<Value>(&plan)? }))
}
