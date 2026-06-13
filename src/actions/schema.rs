use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::{MCPError, Result as MCPResult};

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

/// 5b. Get detailed object info (columns, constraints, indexes, FKs, descriptions)
pub async fn get_object_details(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let schema_name = params
        .as_ref()
        .and_then(|p| p.get("schema"))
        .and_then(|v| v.as_str())
        .unwrap_or("public");

    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let columns = client
        .query(
            "SELECT c.column_name::text, c.data_type::text, c.is_nullable::text,
                    c.column_default::text, c.ordinal_position,
                    COALESCE(pgd.description, '')::text AS column_description,
                    CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END AS is_pk,
                    CASE WHEN uc.column_name IS NOT NULL THEN true ELSE false END AS is_unique
             FROM information_schema.columns c
             LEFT JOIN pg_catalog.pg_statio_all_tables st
                 ON st.relname = c.table_name AND st.schemaname = c.table_schema
             LEFT JOIN pg_catalog.pg_description pgd
                 ON pgd.objoid = st.relid AND pgd.objsubid = c.ordinal_position
             LEFT JOIN (
                 SELECT ku.column_name, tc.table_schema, tc.table_name
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage ku
                     ON tc.constraint_name = ku.constraint_name
                     AND tc.table_schema = ku.table_schema
                 WHERE tc.constraint_type = 'PRIMARY KEY'
             ) pk ON pk.column_name = c.column_name
                 AND pk.table_schema = c.table_schema
                 AND pk.table_name = c.table_name
             LEFT JOIN (
                 SELECT ku.column_name, tc.table_schema, tc.table_name
                 FROM information_schema.table_constraints tc
                 JOIN information_schema.key_column_usage ku
                     ON tc.constraint_name = ku.constraint_name
                     AND tc.table_schema = ku.table_schema
                 WHERE tc.constraint_type = 'UNIQUE'
             ) uc ON uc.column_name = c.column_name
                 AND uc.table_schema = c.table_schema
                 AND uc.table_name = c.table_name
             WHERE c.table_schema = $1 AND c.table_name = $2
             ORDER BY c.ordinal_position",
            &[&schema_name, &table_name],
        )
        .await?;

    let cols: Vec<Value> = columns.iter().map(|row| {
        json!({
            "name": row.get::<_, String>(0),
            "type": row.get::<_, String>(1),
            "nullable": row.get::<_, String>(2) == "YES",
            "default": row.get::<_, Option<String>>(3),
            "position": row.get::<_, i32>(4),
            "description": row.get::<_, String>(5),
            "is_primary_key": row.get::<_, bool>(6),
            "is_unique": row.get::<_, bool>(7),
        })
    }).collect();

    let indexes = client
        .query(
            "SELECT indexname::text, indexdef::text
             FROM pg_indexes
             WHERE schemaname = $1 AND tablename = $2
             ORDER BY indexname",
            &[&schema_name, &table_name],
        )
        .await?;

    let idxs: Vec<Value> = indexes.iter().map(|row| {
        json!({
            "name": row.get::<_, String>(0),
            "definition": row.get::<_, String>(1),
        })
    }).collect();

    let foreign_keys = client
        .query(
            "SELECT kcu.column_name::text,
                    ccu.table_schema::text AS foreign_schema,
                    ccu.table_name::text AS foreign_table,
                    ccu.column_name::text AS foreign_column,
                    rc.update_rule::text, rc.delete_rule::text
             FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage kcu
                 ON tc.constraint_name = kcu.constraint_name
                 AND tc.table_schema = kcu.table_schema
             JOIN information_schema.constraint_column_usage ccu
                 ON tc.constraint_name = ccu.constraint_name
                 AND tc.table_schema = ccu.table_schema
             JOIN information_schema.referential_constraints rc
                 ON tc.constraint_name = rc.constraint_name
                 AND tc.table_schema = rc.constraint_schema
             WHERE tc.constraint_type = 'FOREIGN KEY'
                 AND tc.table_schema = $1 AND tc.table_name = $2
             ORDER BY kcu.ordinal_position",
            &[&schema_name, &table_name],
        )
        .await?;

    let fks: Vec<Value> = foreign_keys.iter().map(|row| {
        json!({
            "column": row.get::<_, String>(0),
            "references_schema": row.get::<_, String>(1),
            "references_table": row.get::<_, String>(2),
            "references_column": row.get::<_, String>(3),
            "on_update": row.get::<_, String>(4),
            "on_delete": row.get::<_, String>(5),
        })
    }).collect();

    let constraints = client
        .query(
            "SELECT constraint_name::text, constraint_type::text
             FROM information_schema.table_constraints
             WHERE table_schema = $1 AND table_name = $2
             ORDER BY constraint_name",
            &[&schema_name, &table_name],
        )
        .await?;

    let cons: Vec<Value> = constraints.iter().map(|row| {
        json!({
            "name": row.get::<_, String>(0),
            "type": row.get::<_, String>(1),
        })
    }).collect();

    let row_estimate = client
        .query_one(
            "SELECT n_live_tup FROM pg_stat_user_tables
             WHERE schemaname = $1 AND relname = $2",
            &[&schema_name, &table_name],
        )
        .await
        .map(|r| r.get::<_, Option<f64>>(0))
        .unwrap_or(None);

    let table_size = client
        .query_one(
            "SELECT pg_size_pretty(pg_total_relation_size($1::regclass))",
            &[&format!("{}.{}", schema_name, table_name)],
        )
        .await
        .map(|r| r.get::<_, Option<String>>(0))
        .unwrap_or(None);

    Ok(json!({
        "table": table_name,
        "schema": schema_name,
        "columns": cols,
        "indexes": idxs,
        "foreign_keys": fks,
        "constraints": cons,
        "estimated_rows": row_estimate,
        "total_size": table_size,
    }))
}
