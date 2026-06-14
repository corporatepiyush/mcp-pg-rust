use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::{MCPError, Result as MCPResult};
use crate::validation::validate_identifier;

/// 1. List all tables
pub async fn list_tables(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn describe_table(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_identifier(table_name, "table")?;

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
pub async fn list_indexes(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn list_schemas(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn show_constraints(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
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
pub async fn get_object_details(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema_name = params
        .as_ref()
        .and_then(|p| p.get("schema"))
        .and_then(|v| v.as_str())
        .unwrap_or("public");

    validate_identifier(schema_name, "schema")?;

    let table_name = params
        .as_ref()
        .and_then(|p| p.get("table"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_identifier(table_name, "table")?;

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

/// 6. List triggers
pub async fn list_triggers(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let limit = params
        .as_ref()
        .and_then(|p| p.get("limit").and_then(|v| v.as_i64()))
        .unwrap_or(1000);

    validate_identifier(table, "table")?;
    validate_identifier(schema, "schema")?;

    if !((1..=10000).contains(&limit)) {
        return Err(MCPError::InvalidParams(
            format!("'limit' must be between 1 and 10000 (got {})", limit)
        ));
    }

    let rows = client
        .query(
            "SELECT trigger_name, event_object_table, event_manipulation,
                    action_timing, action_statement, trigger_schema
             FROM information_schema.triggers
             WHERE event_object_table = $1 AND trigger_schema = $2
             ORDER BY trigger_name
             LIMIT $3",
            &[&table, &schema, &limit],
        )
        .await?;

    let triggers: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "table": row.get::<_, String>(1),
                "event": row.get::<_, String>(2),
                "timing": row.get::<_, String>(3),
                "statement": row.get::<_, String>(4),
                "schema": row.get::<_, String>(5),
            })
        })
        .collect();

    Ok(json!({
        "table": table,
        "schema": schema,
        "trigger_count": triggers.len(),
        "triggers": triggers
    }))
}

/// 7. Create index
pub async fn create_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params
        .as_ref()
        .and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'index_name' parameter".into()))?;

    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let columns = params
        .as_ref()
        .and_then(|p| p.get("columns").and_then(|v| v.as_array()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'columns' parameter (array)".into()))?;

    if columns.is_empty() {
        return Err(MCPError::InvalidParams("'columns' array must not be empty".into()));
    }

    validate_identifier(index_name, "index_name")?;
    validate_identifier(table, "table")?;

    let mut column_list = Vec::new();
    for col in columns {
        let col_name = col.as_str()
            .ok_or_else(|| MCPError::InvalidParams("Column names must be strings".into()))?;
        validate_identifier(col_name, "column")?;
        column_list.push(col_name.to_string());
    }

    let unique = params
        .as_ref()
        .and_then(|p| p.get("unique").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let concurrent = params
        .as_ref()
        .and_then(|p| p.get("concurrent").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let unique_str = if unique { "UNIQUE " } else { "" };
    let concurrent_str = if concurrent { "CONCURRENTLY " } else { "" };
    let columns_str = column_list.join(", ");

    let sql = format!(
        "CREATE {}INDEX {} {} ON {}({})",
        unique_str,
        concurrent_str,
        index_name,
        table,
        columns_str
    );

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE INDEX",
        "index_name": index_name,
        "table": table,
        "columns": column_list,
        "unique": unique,
        "concurrent": concurrent
    }))
}

/// 8. Drop index
pub async fn drop_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params
        .as_ref()
        .and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'index_name' parameter".into()))?;

    validate_identifier(index_name, "index_name")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let concurrent = params
        .as_ref()
        .and_then(|p| p.get("concurrent").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };
    let concurrent_str = if concurrent { "CONCURRENTLY " } else { "" };

    let sql = format!(
        "DROP INDEX {}{}{}",
        if_exists_str,
        concurrent_str,
        index_name
    );

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP INDEX",
        "index_name": index_name,
        "if_exists": if_exists,
        "concurrent": concurrent
    }))
}

/// 9. Create partition
pub async fn create_partition(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let partition_name = params
        .as_ref()
        .and_then(|p| p.get("partition_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'partition_name' parameter".into()))?;

    let partition_type = params
        .as_ref()
        .and_then(|p| p.get("partition_type").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'partition_type' parameter (RANGE/LIST/HASH)".into()))?;

    validate_identifier(table, "table")?;
    validate_identifier(partition_name, "partition_name")?;

    let partition_type_upper = partition_type.to_uppercase();
    if !["RANGE", "LIST", "HASH"].contains(&partition_type_upper.as_str()) {
        return Err(MCPError::InvalidParams(
            format!("'partition_type' must be RANGE, LIST, or HASH (got {})", partition_type)
        ));
    }

    let column = params
        .as_ref()
        .and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'column' parameter".into()))?;

    validate_identifier(column, "column")?;

    let values = params
        .as_ref()
        .and_then(|p| p.get("values").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams(
            "Missing 'values' parameter (FOR RANGE: 'FROM (x) TO (y)' or FOR LIST: 'IN (values)' or FOR HASH: 'MODULUS n REMAINDER r')".into()
        ))?;

    if values.contains(';') || values.contains("--") {
        return Err(MCPError::InvalidParams(
            "Invalid 'values' parameter: semicolons and SQL comments not allowed".into()
        ));
    }

    let sql = format!(
        "CREATE TABLE {} PARTITION OF {} FOR VALUES {}",
        partition_name,
        table,
        values
    );

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE TABLE PARTITION",
        "table": table,
        "partition_name": partition_name,
        "partition_type": partition_type,
        "column": column
    }))
}

/// 10. Drop partition
pub async fn drop_partition(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let partition_name = params
        .as_ref()
        .and_then(|p| p.get("partition_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'partition_name' parameter".into()))?;

    validate_identifier(partition_name, "partition_name")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };

    let sql = format!(
        "DROP TABLE {}{}",
        if_exists_str,
        partition_name
    );

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP TABLE PARTITION",
        "table": table,
        "partition_name": partition_name,
        "if_exists": if_exists
    }))
}

/// 11. Create table
pub async fn create_table(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    let columns = params
        .as_ref()
        .and_then(|p| p.get("columns").and_then(|v| v.as_array()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'columns' parameter (array)".into()))?;

    if columns.is_empty() {
        return Err(MCPError::InvalidParams("'columns' array must not be empty".into()));
    }

    validate_identifier(table, "table")?;

    let mut column_defs = Vec::new();
    for (idx, col) in columns.iter().enumerate() {
        let col_def = col.as_str()
            .ok_or_else(|| MCPError::InvalidParams(format!("Column {} must be a string with format: 'name TYPE [constraints]'", idx)))?;

        if col_def.is_empty() {
            return Err(MCPError::InvalidParams(format!("Column {} definition cannot be empty", idx)));
        }

        if col_def.contains(';') || col_def.contains("--") {
            return Err(MCPError::InvalidParams(
                format!("Column {} definition contains dangerous SQL patterns", idx)
            ));
        }

        column_defs.push(col_def.to_string());
    }

    let columns_str = column_defs.join(", ");
    let sql = format!("CREATE TABLE {} ({})", table, columns_str);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE TABLE",
        "table": table,
        "column_count": columns.len()
    }))
}

/// 12. Drop table
pub async fn drop_table(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_identifier(table, "table")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };
    let cascade_str = if cascade { " CASCADE" } else { "" };

    let sql = format!("DROP TABLE {}{}{}", if_exists_str, table, cascade_str);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP TABLE",
        "table": table,
        "if_exists": if_exists,
        "cascade": cascade
    }))
}

/// 13. Create view
pub async fn create_view(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let view_name = params
        .as_ref()
        .and_then(|p| p.get("view_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'view_name' parameter".into()))?;

    let query = params
        .as_ref()
        .and_then(|p| p.get("query").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'query' parameter".into()))?;

    validate_identifier(view_name, "view_name")?;

    if query.trim().is_empty() {
        return Err(MCPError::InvalidParams("'query' cannot be empty".into()));
    }

    let materialized = params
        .as_ref()
        .and_then(|p| p.get("materialized").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let or_replace = params
        .as_ref()
        .and_then(|p| p.get("or_replace").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let materialized_str = if materialized { "MATERIALIZED " } else { "" };
    let or_replace_str = if or_replace { "OR REPLACE " } else { "" };

    let sql = format!("CREATE {}{}VIEW {} AS {}", or_replace_str, materialized_str, view_name, query);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE VIEW",
        "view_name": view_name,
        "materialized": materialized,
        "or_replace": or_replace
    }))
}

/// 14. Drop view
pub async fn drop_view(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let view_name = params
        .as_ref()
        .and_then(|p| p.get("view_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'view_name' parameter".into()))?;

    validate_identifier(view_name, "view_name")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };
    let cascade_str = if cascade { " CASCADE" } else { "" };

    let sql = format!("DROP VIEW {}{}{}", if_exists_str, view_name, cascade_str);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP VIEW",
        "view_name": view_name,
        "if_exists": if_exists,
        "cascade": cascade
    }))
}

/// 15. Alter view
pub async fn alter_view(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let view_name = params
        .as_ref()
        .and_then(|p| p.get("view_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'view_name' parameter".into()))?;

    validate_identifier(view_name, "view_name")?;

    let rename_to = params
        .as_ref()
        .and_then(|p| p.get("rename_to").and_then(|v| v.as_str()));

    let set_schema = params
        .as_ref()
        .and_then(|p| p.get("set_schema").and_then(|v| v.as_str()));

    if rename_to.is_none() && set_schema.is_none() {
        return Err(MCPError::InvalidParams(
            "Must provide either 'rename_to' or 'set_schema' parameter".into()
        ));
    }

    let mut action_desc = Vec::new();

    if let Some(new_name) = rename_to {
        validate_identifier(new_name, "rename_to")?;
        let sql = format!("ALTER VIEW {} RENAME TO {}", view_name, new_name);
        client.execute(&sql, &[]).await?;
        action_desc.push(format!("renamed to {}", new_name));
    }

    if let Some(schema) = set_schema {
        validate_identifier(schema, "set_schema")?;
        let sql = format!("ALTER VIEW {} SET SCHEMA {}", view_name, schema);
        client.execute(&sql, &[]).await?;
        action_desc.push(format!("moved to schema {}", schema));
    }

    Ok(json!({
        "status": "success",
        "action": "ALTER VIEW",
        "view_name": view_name,
        "changes": action_desc
    }))
}

/// 16. Create schema
pub async fn create_schema(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema_name = params
        .as_ref()
        .and_then(|p| p.get("schema_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'schema_name' parameter".into()))?;

    validate_identifier(schema_name, "schema_name")?;

    let if_not_exists = params
        .as_ref()
        .and_then(|p| p.get("if_not_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_not_exists_str = if if_not_exists { "IF NOT EXISTS " } else { "" };

    let sql = format!("CREATE SCHEMA {}{}", if_not_exists_str, schema_name);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE SCHEMA",
        "schema_name": schema_name,
        "if_not_exists": if_not_exists
    }))
}

/// 17. Drop schema
pub async fn drop_schema(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema_name = params
        .as_ref()
        .and_then(|p| p.get("schema_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'schema_name' parameter".into()))?;

    validate_identifier(schema_name, "schema_name")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };
    let cascade_str = if cascade { " CASCADE" } else { "" };

    let sql = format!("DROP SCHEMA {}{}{}", if_exists_str, schema_name, cascade_str);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP SCHEMA",
        "schema_name": schema_name,
        "if_exists": if_exists,
        "cascade": cascade
    }))
}

/// 18. Create sequence
pub async fn create_sequence(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sequence_name = params
        .as_ref()
        .and_then(|p| p.get("sequence_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'sequence_name' parameter".into()))?;

    validate_identifier(sequence_name, "sequence_name")?;

    let if_not_exists = params
        .as_ref()
        .and_then(|p| p.get("if_not_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let start = params
        .as_ref()
        .and_then(|p| p.get("start").and_then(|v| v.as_i64()))
        .unwrap_or(1);

    let increment = params
        .as_ref()
        .and_then(|p| p.get("increment").and_then(|v| v.as_i64()))
        .unwrap_or(1);

    let if_not_exists_str = if if_not_exists { "IF NOT EXISTS " } else { "" };

    let sql = format!(
        "CREATE SEQUENCE {}{} START {} INCREMENT {}",
        if_not_exists_str,
        sequence_name,
        start,
        increment
    );

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "CREATE SEQUENCE",
        "sequence_name": sequence_name,
        "start": start,
        "increment": increment,
        "if_not_exists": if_not_exists
    }))
}

/// 19. Drop sequence
pub async fn drop_sequence(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let sequence_name = params
        .as_ref()
        .and_then(|p| p.get("sequence_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'sequence_name' parameter".into()))?;

    validate_identifier(sequence_name, "sequence_name")?;

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let if_exists_str = if if_exists { "IF EXISTS " } else { "" };

    let sql = format!("DROP SEQUENCE {}{}", if_exists_str, sequence_name);

    client.execute(&sql, &[]).await?;

    Ok(json!({
        "status": "success",
        "action": "DROP SEQUENCE",
        "sequence_name": sequence_name,
        "if_exists": if_exists
    }))
}

/// 20. Alter index
pub async fn alter_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params
        .as_ref()
        .and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'index_name' parameter".into()))?;

    validate_identifier(index_name, "index_name")?;

    let rename_to = params
        .as_ref()
        .and_then(|p| p.get("rename_to").and_then(|v| v.as_str()));

    let set_schema = params
        .as_ref()
        .and_then(|p| p.get("set_schema").and_then(|v| v.as_str()));

    if rename_to.is_none() && set_schema.is_none() {
        return Err(MCPError::InvalidParams(
            "Must provide either 'rename_to' or 'set_schema' parameter".into()
        ));
    }

    let mut action_desc = Vec::new();

    if let Some(new_name) = rename_to {
        validate_identifier(new_name, "rename_to")?;
        let sql = format!("ALTER INDEX {} RENAME TO {}", index_name, new_name);
        client.execute(&sql, &[]).await?;
        action_desc.push(format!("renamed to {}", new_name));
    }

    if let Some(schema) = set_schema {
        validate_identifier(schema, "set_schema")?;
        let sql = format!("ALTER INDEX {} SET SCHEMA {}", index_name, schema);
        client.execute(&sql, &[]).await?;
        action_desc.push(format!("moved to schema {}", schema));
    }

    Ok(json!({
        "status": "success",
        "action": "ALTER INDEX",
        "index_name": index_name,
        "changes": action_desc
    }))
}

/// 21. List partitions
pub async fn list_partitions(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_identifier(table, "table")?;

    let rows = client
        .query(
            "SELECT child.relname as partition_name, pg_size_pretty(pg_relation_size(child.oid)) as size
             FROM pg_inherits
             JOIN pg_class parent ON pg_inherits.inhparent = parent.oid
             JOIN pg_class child ON pg_inherits.inhrelid = child.oid
             JOIN pg_namespace ON child.relnamespace = pg_namespace.oid
             WHERE parent.relname = $1
             ORDER BY child.relname",
            &[&table],
        )
        .await?;

    let partitions: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "partition_name": row.get::<_, String>(0),
                "size": row.get::<_, String>(1),
            })
        })
        .collect();

    Ok(json!({
        "table": table,
        "partition_count": partitions.len(),
        "partitions": partitions
    }))
}

/// 22. Backup table
pub async fn backup_table(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| MCPError::InvalidParams("Missing 'table' parameter".into()))?;

    validate_identifier(table, "table")?;

    let backup_name = format!("backup_{}", table);
    validate_identifier(&backup_name, "backup_name")?;

    // Verify source table exists
    let _: (i32,) = client
        .query_one(
            "SELECT 1 FROM information_schema.tables
             WHERE table_name = $1 AND table_schema NOT IN ('pg_catalog', 'information_schema')",
            &[&table],
        )
        .await
        .map_err(|_| MCPError::InvalidParams(format!("Table '{}' does not exist", table)))?
        .try_get(0)
        .map(|val| (val,))
        .map_err(|_| MCPError::InvalidParams("Could not verify table existence".into()))?;

    // Check if backup already exists
    let backup_exists = client
        .query_opt(
            "SELECT 1 FROM information_schema.tables WHERE table_name = $1",
            &[&backup_name],
        )
        .await?
        .is_some();

    if backup_exists {
        return Err(MCPError::InvalidParams(
            format!("Backup table '{}' already exists. Drop it first or use a different table.", backup_name)
        ));
    }

    // Create backup table with all columns and structure
    let create_backup_sql = format!("CREATE TABLE {} AS SELECT * FROM {}", backup_name, table);
    client.execute(&create_backup_sql, &[]).await?;

    // Get column info for the original table
    let columns: Vec<(String, String, String)> = client
        .query(
            "SELECT column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_name = $1
             ORDER BY ordinal_position",
            &[&table],
        )
        .await?
        .iter()
        .map(|row| {
            (
                row.get::<_, String>(0),
                row.get::<_, String>(1),
                row.get::<_, String>(2),
            )
        })
        .collect();

    // Get and copy indexes
    let indexes: Vec<String> = client
        .query(
            "SELECT indexname FROM pg_indexes WHERE tablename = $1 AND schemaname NOT IN ('pg_catalog')",
            &[&table],
        )
        .await?
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect();

    let mut indexes_created = 0;
    for idx_name in indexes {
        let new_idx_name = format!("{}_on_{}", idx_name, backup_name);
        let idx_def: String = client
            .query_one(
                "SELECT indexdef FROM pg_indexes WHERE indexname = $1",
                &[&idx_name],
            )
            .await?
            .get(0);

        let updated_def = idx_def
            .replace(&format!("ON {}", table), &format!("ON {}", backup_name))
            .replace(&idx_name, &new_idx_name);

        if client.execute(&updated_def, &[]).await.is_ok() {
            indexes_created += 1;
        }
    }

    // Get row count
    let row_count: (i64,) = client
        .query_one(&format!("SELECT COUNT(*) FROM {}", backup_name), &[])
        .await?
        .try_get(0)
        .map(|val| (val,))
        .map_err(|_| MCPError::InvalidParams("Could not count rows".into()))?;

    Ok(json!({
        "status": "success",
        "action": "BACKUP TABLE",
        "original_table": table,
        "backup_table": backup_name,
        "rows_copied": row_count.0,
        "columns_copied": columns.len(),
        "indexes_created": indexes_created,
        "message": format!("Table '{}' backed up to '{}' with {} rows", table, backup_name, row_count.0)
    }))
}
