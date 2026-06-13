use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_IDENTIFIER_LEN: usize = 255;

fn validate_table_name(name: &str) -> std::result::Result<(), crate::errors::MCPError> {
    if name.is_empty() {
        return Err(crate::errors::MCPError::InvalidParams("'table' must not be empty".into()));
    }
    if name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'table' exceeds maximum length of {MAX_IDENTIFIER_LEN} characters (got {})", name.len())
        ));
    }
    Ok(())
}

/// 21. Vacuum analyze
pub async fn vacuum_analyze(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()).map(|s| s.to_string()));

    if let Some(ref table) = table_name {
        validate_table_name(table)?;
    }

    let sql = if let Some(ref table) = table_name {
        format!("VACUUM ANALYZE {}", table)
    } else {
        "VACUUM ANALYZE".to_string()
    };

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "VACUUM ANALYZE",
        "table": table_name
    }))
}

/// 22. Analyze table
pub async fn analyze_table(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_table_name(&table_name)?;

    client.execute(&format!("ANALYZE {}", table_name), &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "ANALYZE",
        "table": table_name
    }))
}

/// 23. Reindex table
pub async fn reindex_table(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_table_name(&table_name)?;

    client.execute(&format!("REINDEX TABLE {}", table_name), &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "REINDEX",
        "table": table_name
    }))
}

/// 24. Get pg stat statements
pub async fn get_pg_stat_statements(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT query, calls, mean_time, max_time, total_time
             FROM pg_stat_statements
             ORDER BY total_time DESC
             LIMIT 50",
            &[],
        )
        .await;

    match rows {
        Ok(r) => {
            let statements: Vec<Value> = r
                .iter()
                .map(|row| {
                    json!({
                        "query": row.get::<_, String>(0),
                        "calls": row.get::<_, i64>(1),
                        "mean_time_ms": row.get::<_, f64>(2),
                        "max_time_ms": row.get::<_, f64>(3),
                        "total_time_ms": row.get::<_, f64>(4),
                    })
                })
                .collect();

            Ok(json!({ "statements": statements }))
        }
        Err(_) => {
            Ok(json!({
                "error": "pg_stat_statements extension not installed",
                "statements": []
            }))
        }
    }
}

/// 25. Reset statistics
pub async fn reset_statistics(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    client.execute("SELECT pg_stat_reset()", &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "reset_statistics",
        "message": "All statistics counters have been reset"
    }))
}
