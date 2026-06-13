use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_PID: i64 = 4_000_000;

/// 16. List connections
pub async fn list_connections(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pid, usename::text, application_name, state,
                    state_change::text, backend_start::text, query_start::text
             FROM pg_stat_activity
             WHERE pid != pg_backend_pid()
             ORDER BY backend_start DESC",
            &[],
        )
        .await?;

    let connections: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, Option<String>>(1),
                "application": row.get::<_, Option<String>>(2),
                "state": row.get::<_, Option<String>>(3),
                "state_change": row.get::<_, Option<String>>(4),
                "backend_start": row.get::<_, Option<String>>(5),
                "query_start": row.get::<_, Option<String>>(6),
            })
        })
        .collect();

    Ok(json!({ "connections": connections }))
}

/// 17. Kill connection
pub async fn kill_connection(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let pid = params
        .as_ref()
        .and_then(|p| p.get("pid").and_then(|v| v.as_i64()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'pid' parameter".into()))?;

    if pid <= 0 || pid > MAX_PID {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'pid' must be between 1 and {MAX_PID}")
        ));
    }

    let rows = client
        .query("SELECT pg_terminate_backend($1)", &[&(pid as i32)])
        .await?;

    let terminated = rows[0].get::<_, bool>(0);

    Ok(json!({
        "pid": pid,
        "terminated": terminated
    }))
}

/// 18. Show current user
pub async fn show_current_user(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query("SELECT current_user, current_database(), version()", &[])
        .await?;

    let row = &rows[0];

    Ok(json!({
        "user": row.get::<_, String>(0),
        "database": row.get::<_, String>(1),
        "version": row.get::<_, String>(2),
    }))
}

/// 19. Show running queries
pub async fn show_running_queries(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pid, usename, application_name, state, query, query_start
             FROM pg_stat_activity
             WHERE state != 'idle' AND pid != pg_backend_pid()
             ORDER BY query_start DESC",
            &[],
        )
        .await?;

    let queries: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "application": row.get::<_, Option<String>>(2),
                "state": row.get::<_, String>(3),
                "query": row.get::<_, Option<String>>(4),
                "query_start": row.get::<_, Option<String>>(5),
            })
        })
        .collect();

    Ok(json!({ "queries": queries }))
}

/// 20. Show connection summary
pub async fn show_connection_summary(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT state, count(*) as count
             FROM pg_stat_activity
             GROUP BY state
             ORDER BY count DESC",
            &[],
        )
        .await?;

    let summary: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "state": row.get::<_, Option<String>>(0).unwrap_or_else(|| "unknown".to_string()),
                "count": row.get::<_, i64>(1),
            })
        })
        .collect();

    Ok(json!({ "summary": summary }))
}
