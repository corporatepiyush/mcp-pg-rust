use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 36. Show replication status
pub async fn show_replication_status(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pg_is_wal_replay_paused(), pg_last_wal_receive_lsn(),
                    pg_last_wal_replay_lsn(), now() - pg_postmaster_start_time() as uptime",
            &[],
        )
        .await?;

    let row = &rows[0];

    Ok(json!({
        "is_wal_replay_paused": row.get::<_, bool>(0),
        "last_wal_receive_lsn": row.get::<_, Option<String>>(1),
        "last_wal_replay_lsn": row.get::<_, Option<String>>(2),
        "uptime": row.get::<_, Option<String>>(3),
    }))
}

/// 37. List replication slots
pub async fn list_replication_slots(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT slot_name, slot_type, datname, active, restart_lsn, confirmed_flush_lsn
             FROM pg_replication_slots
             ORDER BY slot_name",
            &[],
        )
        .await?;

    let slots: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "slot_name": row.get::<_, String>(0),
                "slot_type": row.get::<_, String>(1),
                "database": row.get::<_, Option<String>>(2),
                "active": row.get::<_, bool>(3),
                "restart_lsn": row.get::<_, Option<String>>(4),
                "confirmed_flush_lsn": row.get::<_, Option<String>>(5),
            })
        })
        .collect();

    Ok(json!({ "replication_slots": slots }))
}

/// 38. List standby servers
pub async fn list_standby_servers(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT client_addr, client_port, state, sync_state, write_lag, flush_lag, replay_lag
             FROM pg_stat_replication
             ORDER BY client_addr, client_port",
            &[],
        )
        .await?;

    let standbys: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "client_address": row.get::<_, Option<String>>(0),
                "client_port": row.get::<_, Option<i32>>(1),
                "state": row.get::<_, String>(2),
                "sync_state": row.get::<_, String>(3),
                "write_lag": row.get::<_, Option<String>>(4),
                "flush_lag": row.get::<_, Option<String>>(5),
                "replay_lag": row.get::<_, Option<String>>(6),
            })
        })
        .collect();

    Ok(json!({ "standbys": standbys }))
}

/// 39. Show WAL info
pub async fn show_wal_info(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT pg_current_wal_lsn(), pg_current_wal_insert_lsn(),
                    pg_is_wal_replay_paused(), pg_wal_lsn_diff(pg_current_wal_lsn(), '0/0') as bytes",
            &[],
        )
        .await?;

    let row = &rows[0];

    Ok(json!({
        "current_wal_lsn": row.get::<_, String>(0),
        "current_wal_insert_lsn": row.get::<_, String>(1),
        "wal_replay_paused": row.get::<_, bool>(2),
        "wal_size_bytes": row.get::<_, Option<f64>>(3),
    }))
}

/// 40. Show base backup progress
pub async fn show_base_backup_progress(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT phase, backup_total, backup_streamed, tablespaces_total, tablespaces_streamed
             FROM pg_stat_basebackup
             WHERE phase IS NOT NULL",
            &[],
        )
        .await;

    match rows {
        Ok(r) => {
            if r.is_empty() {
                return Ok(json!({
                    "status": "no_backup",
                    "message": "No base backup in progress"
                }));
            }

            let row = &r[0];

            Ok(json!({
                "phase": row.get::<_, String>(0),
                "backup_total": row.get::<_, Option<i64>>(1),
                "backup_streamed": row.get::<_, Option<i64>>(2),
                "tablespaces_total": row.get::<_, i32>(3),
                "tablespaces_streamed": row.get::<_, i32>(4),
            }))
        }
        Err(_) => {
            Ok(json!({
                "status": "unavailable",
                "message": "Base backup progress not available on this PostgreSQL version"
            }))
        }
    }
}
