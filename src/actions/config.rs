use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_SETTING_NAME_LEN: usize = 255;

/// 31. Show all settings
pub async fn show_all_settings(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT name, setting, unit, short_desc, context
             FROM pg_settings
             WHERE context NOT LIKE 'internal%'
             ORDER BY name",
            &[],
        )
        .await?;

    let settings: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "value": row.get::<_, Option<String>>(1),
                "unit": row.get::<_, Option<String>>(2),
                "description": row.get::<_, String>(3),
                "context": row.get::<_, String>(4),
            })
        })
        .collect();

    Ok(json!({ "settings": settings }))
}

/// 32. Get setting
pub async fn get_setting(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let setting_name = params
        .as_ref()
        .and_then(|p| p.get("setting").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'setting' parameter".into()))?;

    if setting_name.is_empty() || setting_name.len() > MAX_SETTING_NAME_LEN {
        return Err(crate::errors::MCPError::InvalidParams(
            format!("'setting' must be 1-{MAX_SETTING_NAME_LEN} characters")
        ));
    }

    let rows = client
        .query(
            "SELECT name, setting, unit, short_desc, context, vartype, source
             FROM pg_settings
             WHERE name = $1",
            &[&setting_name],
        )
        .await?;

    if rows.is_empty() {
        return Err(crate::errors::MCPError::InvalidParams(format!("Setting not found: {}", setting_name)));
    }

    let row = &rows[0];

    Ok(json!({
        "name": row.get::<_, String>(0),
        "value": row.get::<_, Option<String>>(1),
        "unit": row.get::<_, Option<String>>(2),
        "description": row.get::<_, String>(3),
        "context": row.get::<_, String>(4),
        "type": row.get::<_, String>(5),
        "source": row.get::<_, String>(6),
    }))
}

/// 33. Show memory settings
pub async fn show_memory_settings(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT name, setting, unit
             FROM pg_settings
             WHERE name IN ('shared_buffers', 'effective_cache_size', 'work_mem',
                           'maintenance_work_mem', 'wal_buffers', 'random_page_cost',
                           'effective_io_concurrency')
             ORDER BY name",
            &[],
        )
        .await?;

    let settings: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "value": row.get::<_, Option<String>>(1),
                "unit": row.get::<_, Option<String>>(2),
            })
        })
        .collect();

    Ok(json!({ "memory_settings": settings }))
}

/// 34. Show performance settings
pub async fn show_performance_settings(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT name, setting, unit
             FROM pg_settings
             WHERE name IN ('max_connections', 'checkpoint_timeout', 'checkpoint_completion_target',
                           'wal_level', 'max_wal_senders', 'wal_keep_size', 'synchronous_commit',
                           'constraint_exclusion')
             ORDER BY name",
            &[],
        )
        .await?;

    let settings: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "value": row.get::<_, Option<String>>(1),
                "unit": row.get::<_, Option<String>>(2),
            })
        })
        .collect();

    Ok(json!({ "performance_settings": settings }))
}

/// 35. Show log settings
pub async fn show_log_settings(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT name, setting, unit
             FROM pg_settings
             WHERE name LIKE 'log%'
             ORDER BY name",
            &[],
        )
        .await?;

    let settings: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "value": row.get::<_, Option<String>>(1),
                "unit": row.get::<_, Option<String>>(2),
            })
        })
        .collect();

    Ok(json!({ "log_settings": settings }))
}
