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
/// Supports EXPLAIN with optional ANALYZE, BUFFERS, and FORMAT options.
/// Only SELECT queries can be explained.
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

    let format_upper = format.to_uppercase();
    if format_upper == "XML" {
        return Err(crate::errors::MCPError::InvalidParams(
            "XML format is not supported — use TEXT, YAML, or JSON".into()
        ));
    }

    let mut opts = Vec::new();
    opts.push(format!("FORMAT {}", format_upper));
    if analyze {
        opts.push("ANALYZE".to_string());
    }
    if buffers {
        opts.push("BUFFERS".to_string());
    }

    let mut explain_sql = String::with_capacity(sql.len() + 64);
    explain_sql.push_str("EXPLAIN (");
    explain_sql.push_str(&opts.join(", "));
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
        let lines: Vec<String> = rows.iter().map(|r| r.get::<_, String>(0)).collect();
        Ok(json!({
            "plan": lines.join("\n"),
            "options": { "analyze": analyze, "buffers": buffers, "format": format }
        }))
    }
}
