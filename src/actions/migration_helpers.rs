use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

const MAX_IDENTIFIER_LEN: usize = 255;

pub async fn generate_create_table_ddl(
    client: &Client,
    params: &Option<&Value>,
) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into())
        })?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    if table.is_empty() || table.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'table' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let rows = client
        .query(
            "SELECT column_name, data_type, character_maximum_length,
                    is_nullable, column_default, ordinal_position
             FROM information_schema.columns
             WHERE table_schema = $1 AND table_name = $2
             ORDER BY ordinal_position",
            &[&schema, &table],
        )
        .await?;

    if rows.is_empty() {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "Table {}.{} not found",
            schema, table
        )));
    }

    let mut ddl = format!(
        "CREATE TABLE {}.{} (\n",
        crate::validation::quote_ident(schema),
        crate::validation::quote_ident(table)
    );
    let mut cols = Vec::new();

    for row in &rows {
        let col_name: String = row.get(0);
        let data_type: String = row.get(1);
        let max_len: Option<i32> = row.get(2);
        let nullable: String = row.get(3);
        let default: Option<String> = row.get(4);

        let q_col = crate::validation::quote_ident(&col_name);
        let mut col = format!("    {q_col} {data_type}");
        if let Some(len) = max_len {
            col = format!("    {q_col} {data_type}({len})");
        }
        if let Some(d) = default {
            col.push_str(&format!(" DEFAULT {}", d));
        }
        if nullable == "NO" {
            col.push_str(" NOT NULL");
        }
        cols.push(col);
    }

    // Get primary key
    let pk_rows = client
        .query(
            "SELECT kcu.column_name
             FROM information_schema.table_constraints tc
             JOIN information_schema.key_column_usage kcu ON kcu.constraint_name = tc.constraint_name
             WHERE tc.table_schema = $1 AND tc.table_name = $2 AND tc.constraint_type = 'PRIMARY KEY'
             ORDER BY kcu.ordinal_position",
            &[&schema, &table],
        )
        .await?;

    if !pk_rows.is_empty() {
        let pk_cols: Vec<String> = pk_rows
            .iter()
            .map(|r| crate::validation::quote_ident(&r.get::<_, String>(0)))
            .collect();
        cols.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));
    }

    ddl.push_str(&cols.join(",\n"));
    ddl.push_str("\n);");

    Ok(json!({ "ddl": ddl, "table": table, "schema": schema }))
}

pub async fn generate_create_index_ddl(
    client: &Client,
    params: &Option<&Value>,
) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into())
        })?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let rows = client
        .query(
            "SELECT i.indexrelid::regclass::text AS index_name,
                    pg_get_indexdef(i.indexrelid) AS index_def
             FROM pg_index i
             JOIN pg_class c ON c.oid = i.indrelid
             JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relname = $2
             ORDER BY i.indexrelid::regclass::text",
            &[&schema, &table],
        )
        .await?;

    if rows.is_empty() {
        return Ok(json!({ "indexes": [], "table": table, "message": "No indexes found" }));
    }

    let indexes: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "ddl": row.get::<_, String>(1),
            })
        })
        .collect();

    Ok(json!({ "indexes": indexes, "table": table }))
}

pub async fn table_dependencies(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into())
        })?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    let rows = client
        .query(
            "SELECT
                cl.relname AS dependent_object,
                CASE
                    WHEN cl.relkind = 'r' THEN 'table'
                    WHEN cl.relkind = 'v' THEN 'view'
                    WHEN cl.relkind = 'm' THEN 'materialized_view'
                    WHEN cl.relkind = 'i' THEN 'index'
                    WHEN cl.relkind = 'S' THEN 'sequence'
                    ELSE 'other'
                END AS object_type,
                n.nspname AS object_schema,
                d.deptype::text AS dependency_type
             FROM pg_depend d
             JOIN pg_class cl ON cl.oid = d.objid
             JOIN pg_namespace n ON n.oid = cl.relnamespace
             JOIN pg_class ref_cl ON ref_cl.oid = d.refobjid
             WHERE d.refobjid = (SELECT c.oid FROM pg_class c
                                 JOIN pg_namespace n2 ON n2.oid = c.relnamespace
                                 WHERE n2.nspname = $1 AND c.relname = $2)
               AND d.deptype IN ('n', 'a')
             ORDER BY object_type, dependent_object",
            &[&schema, &table],
        )
        .await?;

    let dependencies: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "object": row.get::<_, String>(0),
                "type": row.get::<_, String>(1),
                "schema": row.get::<_, String>(2),
                "dependency": if row.get::<_, String>(3) == "n" { "normal" } else { "automatic" },
            })
        })
        .collect();

    // Also get what this table depends on
    let dep_on_rows = client
        .query(
            "SELECT cl.relname AS referenced_object,
                    CASE
                        WHEN cl.relkind = 'r' THEN 'table'
                        WHEN cl.relkind = 'v' THEN 'view'
                        WHEN cl.relkind = 'S' THEN 'sequence'
                        ELSE 'other'
                    END AS object_type,
                    n.nspname AS object_schema
             FROM pg_depend d
             JOIN pg_class cl ON cl.oid = d.refobjid
             JOIN pg_namespace n ON n.oid = cl.relnamespace
             WHERE d.objid = (SELECT c.oid FROM pg_class c
                              JOIN pg_namespace n2 ON n2.oid = c.relnamespace
                              WHERE n2.nspname = $1 AND c.relname = $2)
               AND cl.relname != $2
               AND d.deptype = 'n'
             ORDER BY object_type, referenced_object",
            &[&schema, &table],
        )
        .await?;

    let depends_on: Vec<Value> = dep_on_rows
        .iter()
        .map(|row| {
            json!({
                "object": row.get::<_, String>(0),
                "type": row.get::<_, String>(1),
                "schema": row.get::<_, String>(2),
            })
        })
        .collect();

    Ok(json!({
        "table": table,
        "schema": schema,
        "referenced_by": dependencies,
        "depends_on": depends_on,
    }))
}
