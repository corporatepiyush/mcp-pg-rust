use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

const MAX_IDENTIFIER_LEN: usize = 255;

pub async fn list_extensions(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT e.extname, e.extversion, n.nspname AS schema,
                    c.description, e.extrelocatable
             FROM pg_extension e
             JOIN pg_namespace n ON n.oid = e.extnamespace
             LEFT JOIN pg_description c ON c.objoid = e.oid
             ORDER BY e.extname",
            &[],
        )
        .await?;

    let extensions: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "name": row.get::<_, String>(0),
                "version": row.get::<_, String>(1),
                "schema": row.get::<_, String>(2),
                "description": row.get::<_, Option<String>>(3),
                "relocatable": row.get::<_, bool>(4),
            })
        })
        .collect();

    Ok(json!({ "extensions": extensions }))
}

pub async fn create_extension(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let name = params
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'name' parameter".into()))?;

    if name.is_empty() || name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'name' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()));
    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let version = params
        .as_ref()
        .and_then(|p| p.get("version").and_then(|v| v.as_str()));
    let if_not_exists = params
        .as_ref()
        .and_then(|p| p.get("if_not_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let mut sql = "CREATE EXTENSION".to_string();
    if if_not_exists {
        sql.push_str(" IF NOT EXISTS");
    }
    sql.push_str(&format!(" {}", crate::validation::quote_ident(name)));
    if let Some(s) = schema {
        sql.push_str(&format!(" SCHEMA {}", crate::validation::quote_ident(s)));
    }
    if let Some(v) = version {
        sql.push_str(&format!(" VERSION '{}'", v.replace('\'', "''")));
    }
    if cascade {
        sql.push_str(" CASCADE");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "extension": name, "sql": sql }))
}

pub async fn drop_extension(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let name = params
        .as_ref()
        .and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'name' parameter".into()))?;

    if name.is_empty() || name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'name' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let cascade = params
        .as_ref()
        .and_then(|p| p.get("cascade").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let mut sql = "DROP EXTENSION".to_string();
    if if_exists {
        sql.push_str(" IF EXISTS");
    }
    sql.push_str(&format!(" {}", crate::validation::quote_ident(name)));
    if cascade {
        sql.push_str(" CASCADE");
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "extension": name, "sql": sql }))
}
