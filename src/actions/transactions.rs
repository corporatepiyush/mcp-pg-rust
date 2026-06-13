use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 41. Show active transactions
pub async fn show_active_transactions(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pid, usename, application_name, state, xact_start, query_start, query
             FROM pg_stat_activity
             WHERE xact_start IS NOT NULL AND pid != pg_backend_pid()
             ORDER BY xact_start ASC",
            &[],
        )
        .await?;

    let transactions: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "application": row.get::<_, Option<String>>(2),
                "state": row.get::<_, String>(3),
                "xact_start": row.get::<_, String>(4),
                "query_start": row.get::<_, String>(5),
                "query": row.get::<_, Option<String>>(6),
            })
        })
        .collect();

    Ok(json!({ "transactions": transactions }))
}

/// 42. Show locks
pub async fn show_locks(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT l.pid, a.usename, a.application_name, l.mode, l.granted, l.fastpath,
                    a.query_start, a.query
             FROM pg_locks l
             JOIN pg_stat_activity a ON l.pid = a.pid
             WHERE l.pid != pg_backend_pid()
             ORDER BY l.pid, l.mode",
            &[],
        )
        .await?;

    let locks: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "application": row.get::<_, Option<String>>(2),
                "lock_type": row.get::<_, String>(3),
                "granted": row.get::<_, bool>(4),
                "fastpath": row.get::<_, bool>(5),
                "query_start": row.get::<_, Option<String>>(6),
                "query": row.get::<_, Option<String>>(7),
            })
        })
        .collect();

    Ok(json!({ "locks": locks }))
}

/// 43. Show waiting locks
pub async fn show_waiting_locks(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT l.pid, a.usename, l.mode, a.query_start, a.query
             FROM pg_locks l
             JOIN pg_stat_activity a ON l.pid = a.pid
             WHERE NOT l.granted AND l.pid != pg_backend_pid()
             ORDER BY a.query_start ASC",
            &[],
        )
        .await?;

    let waiting: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "lock_type": row.get::<_, String>(2),
                "query_start": row.get::<_, String>(3),
                "query": row.get::<_, Option<String>>(4),
            })
        })
        .collect();

    Ok(json!({ "waiting_locks": waiting }))
}

/// 44. Begin transaction
pub async fn begin_transaction(client: &Client, params: &Option<Value>) -> MCPResult<Value> {
    let isolation_level = params
        .as_ref()
        .and_then(|p| p.get("isolation_level").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "READ COMMITTED".to_string());

    let valid_levels = vec!["SERIALIZABLE", "REPEATABLE READ", "READ COMMITTED", "READ UNCOMMITTED"];
    if !valid_levels.contains(&isolation_level.as_str()) {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("Invalid isolation level: {}", isolation_level)
        ));
    }

    let sql = format!("BEGIN ISOLATION LEVEL {}", isolation_level);
    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "BEGIN",
        "isolation_level": isolation_level
    }))
}

/// 45. Commit transaction
pub async fn commit_transaction(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    client.execute("COMMIT", &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "COMMIT"
    }))
}

/// 46. Rollback transaction
pub async fn rollback_transaction(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    client.execute("ROLLBACK", &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "ROLLBACK"
    }))
}

/// 47. Show transaction isolation levels
pub async fn show_transaction_isolation(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query("SHOW transaction_isolation", &[])
        .await?;

    let level = rows[0].get::<_, String>(0);

    Ok(json!({
        "isolation_level": level,
        "available_levels": ["serializable", "repeatable read", "read committed", "read uncommitted"]
    }))
}

/// 48. Show deadlocks
pub async fn show_deadlocks(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pid, usename, application_name, state, query_start, query
             FROM pg_stat_activity
             WHERE state = 'disabled' OR wait_event = 'ProcArrayLock'
             ORDER BY query_start ASC",
            &[],
        )
        .await?;

    let deadlocks: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "application": row.get::<_, Option<String>>(2),
                "state": row.get::<_, String>(3),
                "query_start": row.get::<_, String>(4),
                "query": row.get::<_, Option<String>>(5),
            })
        })
        .collect();

    Ok(json!({ "potential_deadlocks": deadlocks }))
}

/// 49. Show auto commit status
///
/// Note: PostgreSQL 17+ removed the `autocommit` GUC.
/// Autocommit is always-on in the wire protocol and cannot be disabled.
/// For PG < 17, we query `SHOW autocommit`; for PG >= 17, we return `true`.
pub async fn show_autocommit_status(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let autocommit = match client.query("SHOW autocommit", &[]).await {
        Ok(rows) => rows[0].get::<_, String>(0) == "on",
        Err(_) => true, // PG 17+ removed the setting; always-on
    };

    Ok(json!({
        "autocommit": autocommit,
        "value": if autocommit { "on" } else { "off" }
    }))
}

/// 50. Show transaction timeout
pub async fn show_transaction_timeout(client: &Client, _params: &Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query("SHOW statement_timeout", &[])
        .await?;

    let timeout = rows[0].get::<_, String>(0);

    Ok(json!({
        "statement_timeout": timeout
    }))
}
