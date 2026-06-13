use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 1. List all tables
pub async fn list_tables(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT table_schema, table_name, table_type
             FROM information_schema.tables
             WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
             ORDER BY table_schema, table_name",
            &[],
        )
        .await?;

    let tables: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "name": row.get::<_, String>(1),
                "type": row.get::<_, String>(2),
            })
        })
        .collect();

    Ok(json!({ "tables": tables }))
}

/// 2. Describe table structure
pub async fn describe_table(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let rows = client
        .query(
            "SELECT column_name, data_type, is_nullable, column_default, ordinal_position
             FROM information_schema.columns
             WHERE table_name = $1
             ORDER BY ordinal_position",
            &[&table_name],
        )
        .await?;

    let columns: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "type": row.get::<_, String>(1),
                "nullable": row.get::<_, String>(2),
                "default": row.get::<_, Option<String>>(3),
                "position": row.get::<_, i32>(4),
            })
        })
        .collect();

    Ok(json!({ "columns": columns }))
}

/// 3. List all indexes
pub async fn list_indexes(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname, tablename, indexname, indexdef
             FROM pg_indexes
             WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
             ORDER BY schemaname, tablename, indexname",
            &[],
        )
        .await?;

    let indexes: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "name": row.get::<_, String>(2),
                "definition": row.get::<_, String>(3),
            })
        })
        .collect();

    Ok(json!({ "indexes": indexes }))
}

/// 4. List schemas
pub async fn list_schemas(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schema_name, schema_owner
             FROM information_schema.schemata
             WHERE schema_name NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
             ORDER BY schema_name",
            &[],
        )
        .await?;

    let schemas: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "owner": row.get::<_, String>(1),
            })
        })
        .collect();

    Ok(json!({ "schemas": schemas }))
}

/// 5. Show constraints
pub async fn show_constraints(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT table_schema, table_name, constraint_name, constraint_type
             FROM information_schema.table_constraints
             WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
             ORDER BY table_schema, table_name, constraint_name",
            &[],
        )
        .await?;

    let constraints: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "schema": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "name": row.get::<_, String>(2),
                "type": row.get::<_, String>(3),
            })
        })
        .collect();

    Ok(json!({ "constraints": constraints }))
}
