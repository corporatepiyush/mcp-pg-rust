use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

pub async fn cancel_query(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let pid = params
        .as_ref()
        .and_then(|p| p.get("pid").and_then(|v| v.as_i64()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'pid' parameter".into()))?;

    let rows = client
        .query(
            "SELECT pg_cancel_backend($1) AS cancelled",
            &[&(pid as i32)],
        )
        .await?;
    let cancelled: bool = rows[0].get(0);

    Ok(json!({
        "pid": pid,
        "cancelled": cancelled,
        "message": if cancelled { "Query cancellation sent" } else { "No active query found for this PID" }
    }))
}

pub async fn terminate_connection(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let pid = params
        .as_ref()
        .and_then(|p| p.get("pid").and_then(|v| v.as_i64()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'pid' parameter".into()))?;

    let rows = client
        .query(
            "SELECT pg_terminate_backend($1) AS terminated",
            &[&(pid as i32)],
        )
        .await?;
    let terminated: bool = rows[0].get(0);

    Ok(json!({
        "pid": pid,
        "terminated": terminated,
        "message": if terminated { "Connection terminated" } else { "No connection found with this PID" }
    }))
}

pub async fn show_blocked_queries(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT
                blocked.pid AS blocked_pid,
                blocked.usename AS blocked_user,
                blocked.query AS blocked_query,
                blocked.query_start AS blocked_start,
                blocking.pid AS blocking_pid,
                blocking.usename AS blocking_user,
                blocking.query AS blocking_query,
                blocking.query_start AS blocking_start,
                pg_blocking_pids(blocked.pid) AS blocking_pids
             FROM pg_stat_activity blocked
             JOIN pg_stat_activity blocking ON blocking.pid = ANY(pg_blocking_pids(blocked.pid))
             WHERE blocked.state != 'idle'
             ORDER BY blocked.query_start",
            &[],
        )
        .await?;

    let blocks: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "blocked_pid": row.get::<_, i32>(0),
                "blocked_user": row.get::<_, Option<String>>(1),
                "blocked_query": row.get::<_, Option<String>>(2),
                "blocked_start": row.get::<_, Option<String>>(3),
                "blocking_pid": row.get::<_, i32>(4),
                "blocking_user": row.get::<_, Option<String>>(5),
                "blocking_query": row.get::<_, Option<String>>(6),
                "blocking_start": row.get::<_, Option<String>>(7),
            })
        })
        .collect();

    Ok(json!({ "blocked_queries": blocks }))
}
