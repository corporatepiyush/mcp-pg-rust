use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

pub async fn list_bm25_indexes(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT schemaname, tablename, indexname, indexdef,
                    idx_scan, idx_tup_read, idx_tup_fetch
             FROM pg_stat_user_indexes
             WHERE indexdef LIKE '%USING bm25%'
             ORDER BY schemaname, tablename, indexname",
            &[],
        )
        .await?;

    let indexes: Vec<Value> = rows.iter().map(|row| {
        json!({
            "schema": row.get::<_, String>(0),
            "table": row.get::<_, String>(1),
            "index": row.get::<_, String>(2),
            "definition": row.get::<_, String>(3),
            "scans": row.get::<_, i64>(4),
            "tuples_read": row.get::<_, i64>(5),
            "tuples_fetched": row.get::<_, i64>(6),
        })
    }).collect();

    Ok(json!({ "bm25_indexes": indexes }))
}

pub async fn search_bm25(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let query = params.as_ref().and_then(|p| p.get("query").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'query'".into()))?;
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let index_name = params.as_ref().and_then(|p| p.get("index_name").and_then(|v| v.as_str()));
    let limit = params.as_ref().and_then(|p| p.get("limit").and_then(|v| v.as_i64())).unwrap_or(10);
    let select_cols = params.as_ref().and_then(|p| p.get("select").and_then(|v| v.as_str())).unwrap_or("*");
    let text_column = params.as_ref().and_then(|p| p.get("text_column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'text_column'".into()))?;

    let qualified = format!("{}.{}", crate::validation::quote_ident(schema), crate::validation::quote_ident(table));
    let limit = limit.min(1000);

    let sql = if let Some(idx) = index_name {
        format!(
            "SELECT {}, \"{}\" <@> to_bm25query('{}', '{}') AS bm25_score
             FROM {}
             ORDER BY bm25_score
             LIMIT {}",
            select_cols, text_column, query, idx, qualified, limit
        )
    } else {
        format!(
            "SELECT {}, \"{}\" <@> '{}' AS bm25_score
             FROM {}
             ORDER BY bm25_score
             LIMIT {}",
            select_cols, text_column, query, qualified, limit
        )
    };

    let rows = client.query(&sql, &[]).await?;

    let mut results = Vec::new();
    for row in &rows {
        let mut obj = serde_json::Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let name = col.name();
            if name == "bm25_score" {
                if let Ok(v) = row.try_get::<_, f64>(i) {
                    obj.insert(name.to_string(), json!(-v));
                }
            } else if let Ok(v) = row.try_get::<_, Value>(i) {
                obj.insert(name.to_string(), v);
            } else if let Ok(v) = row.try_get::<_, String>(i) {
                obj.insert(name.to_string(), Value::String(v));
            } else if let Ok(v) = row.try_get::<_, i64>(i) {
                obj.insert(name.to_string(), json!(v));
            } else if let Ok(v) = row.try_get::<_, f64>(i) {
                obj.insert(name.to_string(), json!(v));
            } else if let Ok(v) = row.try_get::<_, bool>(i) {
                obj.insert(name.to_string(), json!(v));
            } else if let Ok(v) = row.try_get::<_, Option<String>>(i) {
                obj.insert(name.to_string(), v.map(Value::String).unwrap_or(Value::Null));
            }
        }
        results.push(Value::Object(obj));
    }

    Ok(json!({
        "results": results,
        "count": results.len(),
        "query": query,
    }))
}

pub async fn create_bm25_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params.as_ref().and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let text_config = params.as_ref().and_then(|p| p.get("text_config").and_then(|v| v.as_str())).unwrap_or("english");
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let index_name = params.as_ref().and_then(|p| p.get("index_name").and_then(|v| v.as_str()));
    let k1 = params.as_ref().and_then(|p| p.get("k1").and_then(|v| v.as_f64()));
    let b = params.as_ref().and_then(|p| p.get("b").and_then(|v| v.as_f64()));
    let where_clause = params.as_ref().and_then(|p| p.get("where").and_then(|v| v.as_str()));
    let concurrently = params.as_ref().and_then(|p| p.get("concurrently").and_then(|v| v.as_bool())).unwrap_or(false);

    let idx_name = match index_name {
        Some(name) => name.to_string(),
        None => format!("idx_{}_{}_bm25", table, column),
    };

    let mut sql = "CREATE INDEX".to_string();
    if concurrently { sql.push_str(" CONCURRENTLY"); }
    sql.push_str(&format!(" {} ON {}.{}", crate::validation::quote_ident(&idx_name), crate::validation::quote_ident(schema), crate::validation::quote_ident(table)));
    sql.push_str(&format!(" USING bm25({}) WITH (text_config='{}'", crate::validation::quote_ident(column), text_config));
    if let Some(k) = k1 { sql.push_str(&format!(", k1={}", k)); }
    if let Some(b_val) = b { sql.push_str(&format!(", b={}", b_val)); }
    sql.push(')');

    if let Some(w) = where_clause {
        sql.push_str(&format!(" WHERE {}", w));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "index": idx_name, "sql": sql }))
}

pub async fn drop_bm25_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params.as_ref().and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'index_name'".into()))?;
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let if_exists = params.as_ref().and_then(|p| p.get("if_exists").and_then(|v| v.as_bool())).unwrap_or(false);
    let concurrently = params.as_ref().and_then(|p| p.get("concurrently").and_then(|v| v.as_bool())).unwrap_or(false);

    let mut sql = "DROP INDEX".to_string();
    if concurrently { sql.push_str(" CONCURRENTLY"); }
    if if_exists { sql.push_str(" IF EXISTS"); }
    sql.push_str(&format!(" {}.{}", crate::validation::quote_ident(schema), crate::validation::quote_ident(index_name)));

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn bm25_force_merge(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params.as_ref().and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'index_name'".into()))?;

    let _rows = client.query("SELECT bm25_force_merge($1)", &[&index_name]).await?;

    Ok(json!({
        "success": true,
        "index": index_name,
        "message": "All segments merged into one",
    }))
}

pub async fn bm25_index_stats(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let index_name = params.as_ref().and_then(|p| p.get("index_name").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'index_name'".into()))?;

    let rows = client.query("SELECT bm25_summarize_index($1)", &[&index_name]).await?;
    let stats: String = rows[0].get(0);

    Ok(json!({
        "index": index_name,
        "statistics": stats,
    }))
}
