use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

pub async fn create_hypertable(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let time_column = params
        .as_ref()
        .and_then(|p| p.get("time_column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'time_column'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let chunk_time = params
        .as_ref()
        .and_then(|p| p.get("chunk_time_interval").and_then(|v| v.as_str()));

    let mut sql = format!(
        "SELECT create_hypertable('{}.{}', '{}'",
        crate::validation::quote_ident(schema),
        crate::validation::quote_ident(table),
        time_column
    );
    if let Some(ct) = chunk_time {
        sql.push_str(&format!(", chunk_time_interval => INTERVAL '{}'", ct));
    }
    sql.push(')');

    let rows = client.query(&sql, &[]).await?;
    let created: bool = rows[0].get(0);

    Ok(json!({ "success": created, "hypertable": format!("{}.{}", schema, table), "sql": sql }))
}

pub async fn show_hypertable_details(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let rows = client
        .query(
            "SELECT hypertable_name, hypertable_schema, owner,
                    num_dimensions, chunk_target_size,
                    compression_state, tablespaces
             FROM timescaledb_information.hypertables
             WHERE hypertable_name = $1 AND hypertable_schema = $2",
            &[&table, &schema],
        )
        .await?;

    if rows.is_empty() {
        return Ok(json!({ "table": format!("{}.{}", schema, table), "is_hypertable": false }));
    }

    let row = &rows[0];
    Ok(json!({
        "table": row.get::<_, String>(0),
        "schema": row.get::<_, String>(1),
        "owner": row.get::<_, Option<String>>(2),
        "dimensions": row.get::<_, Option<i32>>(3),
        "chunk_target_size": row.get::<_, Option<String>>(4),
        "compression": row.get::<_, Option<String>>(5),
        "tablespaces": row.get::<_, Option<String>>(6),
        "is_hypertable": true,
    }))
}

pub async fn show_chunks(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let rows = client
        .query(
            "SELECT chunk_name, chunk_schema, table_name, table_schema,
                    range_start::text, range_end::text, is_compressed::text,
                    disk_size::text
             FROM timescaledb_information.chunks
             WHERE table_name = $1 AND table_schema = $2
             ORDER BY range_start",
            &[&table, &schema],
        )
        .await?;

    let chunks: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "chunk_name": row.get::<_, String>(0),
                "chunk_schema": row.get::<_, String>(1),
                "range_start": row.get::<_, String>(3),
                "range_end": row.get::<_, String>(4),
                "compressed": row.get::<_, String>(5),
                "disk_size": row.get::<_, Option<String>>(6),
            })
        })
        .collect();

    Ok(json!({ "chunks": chunks, "count": chunks.len() }))
}

pub async fn add_retention_policy(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let drop_after = params
        .as_ref()
        .and_then(|p| p.get("drop_after").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'drop_after' (e.g. '90 days')".into())
        })?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let sql = format!(
        "SELECT add_retention_policy('{}.{}', INTERVAL '{}')",
        crate::validation::quote_ident(schema),
        crate::validation::quote_ident(table),
        drop_after
    );

    let rows = client.query(&sql, &[]).await?;
    let job_id: i32 = rows[0].get(0);

    Ok(json!({ "success": true, "job_id": job_id, "sql": sql }))
}

pub async fn add_compression_policy(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let compress_after = params
        .as_ref()
        .and_then(|p| p.get("compress_after").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams(
                "Missing 'compress_after' (e.g. '7 days')".into(),
            )
        })?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let sql = format!(
        "SELECT add_compression_policy('{}.{}', INTERVAL '{}')",
        crate::validation::quote_ident(schema),
        crate::validation::quote_ident(table),
        compress_after
    );

    let rows = client.query(&sql, &[]).await?;
    let job_id: i32 = rows[0].get(0);

    Ok(json!({ "success": true, "job_id": job_id, "sql": sql }))
}

pub async fn compress_chunk(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let chunk_name = params
        .as_ref()
        .and_then(|p| p.get("chunk_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'chunk_name'".into()))?;
    let chunk_schema = params
        .as_ref()
        .and_then(|p| p.get("chunk_schema").and_then(|v| v.as_str()))
        .unwrap_or("_hyper");

    let sql = format!(
        "SELECT compress_chunk('{}.{}')",
        crate::validation::quote_ident(chunk_schema),
        crate::validation::quote_ident(chunk_name)
    );
    let rows = client.query(&sql, &[]).await?;
    let result: String = rows[0].get(0);

    Ok(
        json!({ "success": true, "chunk": format!("{}.{}", chunk_schema, chunk_name), "result": result }),
    )
}

pub async fn add_continuous_aggregate(
    client: &Client,
    params: &Option<&Value>,
) -> MCPResult<Value> {
    let name = params
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'name'".into()))?;
    let query = params
        .as_ref()
        .and_then(|p| p.get("query").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'query'".into()))?;
    let refresh_interval = params
        .as_ref()
        .and_then(|p| p.get("refresh_interval").and_then(|v| v.as_str()));

    let q_name = crate::validation::quote_ident(name);
    let mut sql = format!("CREATE MATERIALIZED VIEW {q_name}");
    sql.push_str("\nWITH (timescaledb.continuous) AS\n");
    sql.push_str(query);
    if let Some(ri) = refresh_interval {
        sql.push_str(&format!("\nWITH DATA;\nSELECT add_continuous_aggregate_policy('{q_name}', INTERVAL '{ri}', INTERVAL '{ri}')"));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "name": name, "sql": sql }))
}
