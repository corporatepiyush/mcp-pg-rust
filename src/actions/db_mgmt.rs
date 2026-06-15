use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_IDENTIFIER_LEN: usize = 255;

pub async fn list_databases(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT d.datname, pg_catalog.pg_get_userbyid(d.datdba) AS owner,
                    pg_catalog.pg_encoding_to_char(d.encoding) AS encoding,
                    d.datcollate, d.datctype,
                    pg_catalog.shobj_description(d.oid, 'pg_database') AS description,
                    d.datistemplate
             FROM pg_catalog.pg_database d
             ORDER BY d.datname",
            &[],
        )
        .await?;

    let databases: Vec<Value> = rows.iter().map(|row| {
        json!({
            "name": row.get::<_, String>(0),
            "owner": row.get::<_, String>(1),
            "encoding": row.get::<_, String>(2),
            "collate": row.get::<_, String>(3),
            "ctype": row.get::<_, String>(4),
            "description": row.get::<_, Option<String>>(5),
            "is_template": row.get::<_, bool>(6),
        })
    }).collect();

    Ok(json!({ "databases": databases }))
}

pub async fn create_database(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let name = params.as_ref().and_then(|p| p.get("name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'name' parameter".into()))?;

    if name.is_empty() || name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!("'name' must be 1-{MAX_IDENTIFIER_LEN} characters")));
    }

    let owner = params.as_ref().and_then(|p| p.get("owner").and_then(|v| v.as_str()));
    let encoding = params.as_ref().and_then(|p| p.get("encoding").and_then(|v| v.as_str()));
    let locale = params.as_ref().and_then(|p| p.get("locale").and_then(|v| v.as_str()));

    let mut sql = format!("CREATE DATABASE {}", crate::validation::quote_ident(name));
    if let Some(o) = owner { sql.push_str(&format!(" OWNER {}", crate::validation::quote_ident(o))); }
    if let Some(e) = encoding { sql.push_str(&format!(" ENCODING '{}'", e.replace('\'', "''"))); }
    if let Some(l) = locale { sql.push_str(&format!(" LC_COLLATE '{}'", l.replace('\'', "''"))); }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "database": name, "sql": sql }))
}
