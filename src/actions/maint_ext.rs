use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

const MAX_IDENTIFIER_LEN: usize = 255;

pub async fn vacuum(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()));
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let full = params.as_ref().and_then(|p| p.get("full").and_then(|v| v.as_bool())).unwrap_or(false);
    let freeze = params.as_ref().and_then(|p| p.get("freeze").and_then(|v| v.as_bool())).unwrap_or(false);
    let verbose = params.as_ref().and_then(|p| p.get("verbose").and_then(|v| v.as_bool())).unwrap_or(false);

    let mut sql = "VACUUM".to_string();
    let opts: Vec<&str> = match (full, freeze, verbose) {
        (true, _, _) => vec!["FULL", if verbose { "VERBOSE" } else { "" }],
        (_, true, _) => vec!["FREEZE", if verbose { "VERBOSE" } else { "" }],
        (_, _, true) => vec!["VERBOSE"],
        _ => vec![],
    };
    let opts_str = opts.iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ");
    if !opts_str.is_empty() {
        sql.push_str(&format!(" {}", opts_str));
    }

    if let Some(t) = table {
        sql.push_str(&format!(" {}.{}", quote_ident(schema), quote_ident(t)));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn vacuum_full(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()));
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let verbose = params.as_ref().and_then(|p| p.get("verbose").and_then(|v| v.as_bool())).unwrap_or(false);
    let analyze = params.as_ref().and_then(|p| p.get("analyze").and_then(|v| v.as_bool())).unwrap_or(false);
    let freeze = params.as_ref().and_then(|p| p.get("freeze").and_then(|v| v.as_bool())).unwrap_or(false);

    let mut sql = "VACUUM FULL".to_string();
    let mut opts = Vec::new();
    if freeze { opts.push("FREEZE"); }
    if verbose { opts.push("VERBOSE"); }
    if analyze { opts.push("ANALYZE"); }
    if !opts.is_empty() {
        sql.push_str(&format!(" {}", opts.join(" ")));
    }

    if let Some(t) = table {
        sql.push_str(&format!(" {}.{}", quote_ident(schema), quote_ident(t)));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn reindex_database(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let database = params.as_ref().and_then(|p| p.get("database").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'database' parameter".into()))?;

    if database.is_empty() || database.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!("'database' must be 1-{MAX_IDENTIFIER_LEN} characters")));
    }

    let concurrent = params.as_ref().and_then(|p| p.get("concurrent").and_then(|v| v.as_bool())).unwrap_or(false);
    let verbose = params.as_ref().and_then(|p| p.get("verbose").and_then(|v| v.as_bool())).unwrap_or(false);
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str()));

    let mut sql = "REINDEX".to_string();
    if concurrent { sql.push_str(" (CONCURRENTLY)"); }
    sql.push_str(" DATABASE ");
    if verbose { sql.push_str("VERBOSE "); }
    sql.push_str(&quote_ident(database));

    if let Some(s) = schema
        && !s.is_empty() && s.len() <= MAX_IDENTIFIER_LEN {
            sql.push_str(&format!(" SCHEMA {}", quote_ident(s)));

            let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()));
            if let Some(t) = table {
                sql.push_str(&format!(" TABLE {}", quote_ident(t)));
            }

            let index = params.as_ref().and_then(|p| p.get("index").and_then(|v| v.as_str()));
            if let Some(i) = index {
                sql = format!("REINDEX{} INDEX {} {}",
                    if concurrent { " (CONCURRENTLY)" } else { "" },
                    if verbose { "VERBOSE " } else { "" },
                    quote_ident(i));
            }
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

fn quote_ident(ident: &str) -> String {
    crate::validation::quote_ident(ident)
}
