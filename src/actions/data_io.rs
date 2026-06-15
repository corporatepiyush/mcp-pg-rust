use futures::SinkExt;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;
use crate::validation::quote_ident;

pub async fn import_from_url(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let url = params.as_ref().and_then(|p| p.get("url").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'url' parameter".into()))?;
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table' parameter".into()))?;
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let delimiter = params.as_ref().and_then(|p| p.get("delimiter").and_then(|v| v.as_str())).unwrap_or(",");
    let header = params.as_ref().and_then(|p| p.get("header").and_then(|v| v.as_bool())).unwrap_or(true);
    let truncate = params.as_ref().and_then(|p| p.get("truncate").and_then(|v| v.as_bool())).unwrap_or(false);
    let columns = params.as_ref().and_then(|p| p.get("columns").and_then(|v| v.as_str()));

    let qualified = format!("{}.{}", quote_ident(schema), quote_ident(table));

    if truncate {
        client.execute(&format!("TRUNCATE {}", qualified), &[]).await?;
    }

    let resp = reqwest::get(url).await
        .map_err(|e| crate::errors::MCPError::InvalidParams(format!("Failed to fetch URL: {}", e)))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(crate::errors::MCPError::InvalidParams(format!("URL returned HTTP {}", status)));
    }
    let content = resp.bytes().await
        .map_err(|e| crate::errors::MCPError::InvalidParams(format!("Failed to read response body: {}", e)))?;

    let col_clause = columns.map(|c| format!(" ({})", c)).unwrap_or_default();
    let copy_sql = format!(
        "COPY {} FROM STDIN (FORMAT csv, HEADER {}, DELIMITER '{}'){}",
        qualified,
        if header { "true" } else { "false" },
        delimiter.replace('\'', "''"),
        col_clause,
    );

    let mut sink = Box::pin(client.copy_in(&copy_sql).await?);
    sink.as_mut().send(content.clone()).await?;
    sink.as_mut().close().await?;

    let count = 0i64;

    Ok(json!({
        "success": true,
        "table": table,
        "schema": schema,
        "rows_imported": count,
        "source": url,
    }))
}

pub async fn export_csv(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let query = params.as_ref().and_then(|p| p.get("query").and_then(|v| v.as_str()));
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()));
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let header = params.as_ref().and_then(|p| p.get("header").and_then(|v| v.as_bool())).unwrap_or(true);
    let delimiter = params.as_ref().and_then(|p| p.get("delimiter").and_then(|v| v.as_str())).unwrap_or(",");
    let limit = params.as_ref().and_then(|p| p.get("limit").and_then(|v| v.as_i64())).unwrap_or(10000).min(100000);

    let sql = match (query, table) {
        (Some(q), _) => {
            let trimmed = q.trim();
            if trimmed.to_uppercase().starts_with("SELECT") {
                format!("({}) AS _export", trimmed.trim_end_matches(';'))
            } else {
                return Err(crate::errors::MCPError::InvalidParams("Query must be a SELECT statement".into()));
            }
        }
        (None, Some(t)) => format!("{}.{}", quote_ident(schema), quote_ident(t)),
        (None, None) => return Err(crate::errors::MCPError::InvalidParams("Either 'query' or 'table' is required".into())),
    };

    let copy_sql = format!(
        "COPY {} TO STDOUT (FORMAT csv, HEADER {}, DELIMITER '{}', LIMIT {})",
        sql,
        if header { "true" } else { "false" },
        delimiter.replace('\'', "''"),
        limit,
    );

    let stream = client.copy_out(&copy_sql).await?;
    let mut stream = Box::pin(stream);
    let mut output = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        output.extend_from_slice(&chunk);
    }

    let csv_text = String::from_utf8(output)
        .map_err(|e| crate::errors::MCPError::InvalidParams(format!("Output is not valid UTF-8: {}", e)))?;

    Ok(json!({
        "csv": csv_text,
        "row_count": csv_text.lines().count().saturating_sub(if header { 1 } else { 0 }),
        "format": "csv",
    }))
}
