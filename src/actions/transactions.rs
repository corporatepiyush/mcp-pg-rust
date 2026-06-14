use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 41. Show active transactions
pub async fn show_active_transactions(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn show_locks(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn show_waiting_locks(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
/// 45. Show transaction isolation levels
pub async fn show_transaction_isolation(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query("SHOW transaction_isolation", &[])
        .await?;

    let level = rows[0].get::<_, String>(0);

    Ok(json!({
        "isolation_level": level,
        "available_levels": ["serializable", "repeatable read", "read committed", "read uncommitted"]
    }))
}

/// 48. Show blocked processes (potential deadlock situations)
///
/// PostgreSQL's deadlock detector runs every `deadlock_timeout` (default 1s)
/// and automatically cancels one transaction when a deadlock cycle is detected.
/// By the time a deadlock is logged, it has already been resolved.
/// This view shows processes that are currently blocked by other processes,
/// which represents potential deadlock or lock contention situations.
pub async fn show_deadlocks(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    // Show processes blocked by others, with the blocking PID identified
    let rows = client
        .query(
            "SELECT a.pid, a.usename::text, a.application_name, a.state,
                    a.query_start::text, a.query,
                    pg_blocking_pids(a.pid) AS blocked_by,
                    (SELECT count(*) FROM pg_stat_activity
                     WHERE pid = ANY(pg_blocking_pids(a.pid))) AS blockers_count
             FROM pg_stat_activity a
             WHERE cardinality(pg_blocking_pids(a.pid)) > 0
               AND a.pid != pg_backend_pid()
             ORDER BY a.query_start ASC",
            &[],
        )
        .await?;

    let blocked: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "pid": row.get::<_, i32>(0),
                "user": row.get::<_, String>(1),
                "application": row.get::<_, Option<String>>(2),
                "state": row.get::<_, String>(3),
                "query_start": row.get::<_, String>(4),
                "query": row.get::<_, Option<String>>(5),
                "blocked_by": row.get::<_, Vec<i32>>(6),
                "blocker_count": row.get::<_, i64>(7),
                "advisory": "Deadlocks are auto-detected and resolved within deadlock_timeout (default 1s). These are currently blocked processes — potential deadlock situations."
            })
        })
        .collect();

    Ok(json!({ "blocked_processes": blocked, "count": blocked.len() }))
}

/// 49. Show auto commit status
///
/// PostgreSQL's `autocommit` GUC was removed in version 7.4 (2003).
/// Autocommit is always-on in the PostgreSQL wire protocol and cannot be
/// disabled server-side. Client libraries (psycopg2, JDBC, etc.) implement
/// auto-commit-off at the client level by wrapping statements in BEGIN/COMMIT.
/// This tool reports `always_on` because the server always uses autocommit.
pub async fn show_autocommit_status(_client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    Ok(json!({
        "autocommit": true,
        "status": "always_on",
        "detail": "PostgreSQL always operates in autocommit mode at the wire protocol level. Client-side autocommit control is implemented by your database driver, not the server.",
        "note": "The autocommit GUC was removed in PostgreSQL 7.4 (2003) and is not available in any supported version."
    }))
}

/// 50. Show transaction timeout
pub async fn show_transaction_timeout(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query("SHOW statement_timeout", &[])
        .await?;

    let timeout = rows[0].get::<_, String>(0);

    Ok(json!({
        "statement_timeout": timeout
    }))
}
