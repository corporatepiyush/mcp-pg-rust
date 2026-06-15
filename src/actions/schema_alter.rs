use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

const MAX_IDENTIFIER_LEN: usize = 255;

fn validate_identifier(val: &str, label: &str) -> Result<(), crate::errors::MCPError> {
    if val.is_empty() || val.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'{label}' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }
    Ok(())
}

fn qi(ident: &str) -> String {
    crate::validation::quote_ident(ident)
}

pub async fn add_column(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params
        .as_ref()
        .and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let data_type = params
        .as_ref()
        .and_then(|p| p.get("data_type").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'data_type'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let not_null = params
        .as_ref()
        .and_then(|p| p.get("not_null").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let default = params
        .as_ref()
        .and_then(|p| p.get("default").and_then(|v| v.as_str()));

    validate_identifier(table, "table")?;
    validate_identifier(column, "column")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!(
        "ALTER TABLE {}.{} ADD COLUMN {}",
        qi(schema),
        qi(table),
        qi(column)
    );
    sql.push_str(&format!(" {}", data_type));
    if let Some(d) = default {
        sql.push_str(&format!(" DEFAULT {}", d));
    }
    if not_null {
        sql.push_str(" NOT NULL");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn drop_column(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params
        .as_ref()
        .and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    validate_identifier(table, "table")?;
    validate_identifier(column, "column")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!(
        "ALTER TABLE {}.{} DROP COLUMN {}",
        qi(schema),
        qi(table),
        qi(column)
    );
    if cascade {
        sql.push_str(" CASCADE");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn rename_column(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params
        .as_ref()
        .and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'new_name'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    validate_identifier(table, "table")?;
    validate_identifier(column, "column")?;
    validate_identifier(new_name, "new_name")?;
    validate_identifier(schema, "schema")?;

    let sql = format!(
        "ALTER TABLE {}.{} RENAME COLUMN {} TO {}",
        qi(schema),
        qi(table),
        qi(column),
        qi(new_name)
    );
    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn alter_column_type(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params
        .as_ref()
        .and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let data_type = params
        .as_ref()
        .and_then(|p| p.get("data_type").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'data_type'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let using = params
        .as_ref()
        .and_then(|p| p.get("using").and_then(|v| v.as_str()));

    validate_identifier(table, "table")?;
    validate_identifier(column, "column")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!(
        "ALTER TABLE {}.{} ALTER COLUMN {} TYPE {}",
        qi(schema),
        qi(table),
        qi(column),
        data_type
    );
    if let Some(expr) = using {
        sql.push_str(&format!(" USING {}", expr));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn rename_table(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'new_name'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    validate_identifier(table, "table")?;
    validate_identifier(new_name, "new_name")?;
    validate_identifier(schema, "schema")?;

    let sql = format!(
        "ALTER TABLE {}.{} RENAME TO {}",
        qi(schema),
        qi(table),
        qi(new_name)
    );
    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn rename_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index = params
        .as_ref()
        .and_then(|p| p.get("index").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'index'".into()))?;
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'new_name'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    validate_identifier(index, "index")?;
    validate_identifier(new_name, "new_name")?;
    validate_identifier(schema, "schema")?;

    let sql = format!(
        "ALTER INDEX {}.{} RENAME TO {}",
        qi(schema),
        qi(index),
        qi(new_name)
    );
    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn rename_schema(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'schema'".into()))?;
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'new_name'".into()))?;

    validate_identifier(schema, "schema")?;
    validate_identifier(new_name, "new_name")?;

    let sql = format!("ALTER SCHEMA {} RENAME TO {}", qi(schema), qi(new_name));
    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn add_foreign_key(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let columns = params
        .as_ref()
        .and_then(|p| p.get("columns").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'columns'".into()))?;
    let ref_table = params
        .as_ref()
        .and_then(|p| p.get("ref_table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'ref_table'".into()))?;
    let ref_columns = params
        .as_ref()
        .and_then(|p| p.get("ref_columns").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'ref_columns'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let constraint_name = params
        .as_ref()
        .and_then(|p| p.get("constraint_name").and_then(|v| v.as_str()));
    let on_delete = params
        .as_ref()
        .and_then(|p| p.get("on_delete").and_then(|v| v.as_str()));
    let on_update = params
        .as_ref()
        .and_then(|p| p.get("on_update").and_then(|v| v.as_str()));

    validate_identifier(table, "table")?;
    validate_identifier(ref_table, "ref_table")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!("ALTER TABLE {}.{} ADD", qi(schema), qi(table));
    if let Some(cname) = constraint_name {
        sql.push_str(&format!(" CONSTRAINT {}", qi(cname)));
    }
    sql.push_str(&format!(
        " FOREIGN KEY ({}) REFERENCES {}.{} ({})",
        columns,
        qi(schema),
        qi(ref_table),
        ref_columns
    ));
    if let Some(od) = on_delete {
        sql.push_str(&format!(" ON DELETE {}", od));
    }
    if let Some(ou) = on_update {
        sql.push_str(&format!(" ON UPDATE {}", ou));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn drop_foreign_key(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let constraint = params
        .as_ref()
        .and_then(|p| p.get("constraint").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'constraint'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    validate_identifier(table, "table")?;
    validate_identifier(constraint, "constraint")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!(
        "ALTER TABLE {}.{} DROP CONSTRAINT {}",
        qi(schema),
        qi(table),
        qi(constraint)
    );
    if cascade {
        sql.push_str(" CASCADE");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn add_unique_constraint(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let columns = params
        .as_ref()
        .and_then(|p| p.get("columns").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'columns'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let constraint_name = params
        .as_ref()
        .and_then(|p| p.get("constraint_name").and_then(|v| v.as_str()));

    validate_identifier(table, "table")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!("ALTER TABLE {}.{} ADD", qi(schema), qi(table));
    if let Some(cname) = constraint_name {
        validate_identifier(cname, "constraint_name")?;
        sql.push_str(&format!(" CONSTRAINT {}", qi(cname)));
    }
    sql.push_str(&format!(" UNIQUE ({})", columns));

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn drop_constraint(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params
        .as_ref()
        .and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let constraint = params
        .as_ref()
        .and_then(|p| p.get("constraint").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'constraint'".into()))?;
    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");
    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    validate_identifier(table, "table")?;
    validate_identifier(constraint, "constraint")?;
    validate_identifier(schema, "schema")?;

    let mut sql = format!(
        "ALTER TABLE {}.{} DROP CONSTRAINT {}",
        qi(schema),
        qi(table),
        qi(constraint)
    );
    if cascade {
        sql.push_str(" CASCADE");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}
