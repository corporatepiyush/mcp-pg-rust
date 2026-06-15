use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;
use crate::validation::quote_ident;

pub async fn sample_data(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let limit = params.as_ref().and_then(|p| p.get("limit").and_then(|v| v.as_i64())).unwrap_or(100).min(10000);
    let where_clause = params.as_ref().and_then(|p| p.get("where").and_then(|v| v.as_str()));
    let order_by = params.as_ref().and_then(|p| p.get("order_by").and_then(|v| v.as_str()));
    let anonymize = params.as_ref().and_then(|p| p.get("anonymize_columns"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());

    let qualified = format!("{}.{}", quote_ident(schema), quote_ident(table));

    let order = match order_by {
        Some(col) => quote_ident(col),
        None => "RANDOM()".to_string(),
    };

    let where_sql = where_clause.map(|w| format!(" WHERE {}", w)).unwrap_or_default();

    let sql = format!(
        "SELECT * FROM {} {} ORDER BY {} LIMIT {}",
        qualified, where_sql, order, limit,
    );

    let rows = client.query(&sql, &[]).await?;

    let has_anonymize = anonymize.as_ref().is_some_and(|a| !a.is_empty());
    let mut results = Vec::new();
    for row in &rows {
        let mut obj = serde_json::Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let name = col.name();
            let masked = has_anonymize && anonymize.as_ref().is_some_and(|a| a.iter().any(|c| c == name));

            if masked {
                let val: Option<String> = row.try_get::<_, Option<String>>(i).ok().flatten();
                match val {
                    Some(v) if v.contains('@') => {
                        let parts: Vec<&str> = v.splitn(2, '@').collect();
                        let _ = obj.insert(name.to_string(), json!(format!("****@{}", parts[1])));
                    }
                    Some(v) if v.len() > 2 => {
                        let first = &v[..1];
                        let last = &v[v.len()-1..];
                        let _ = obj.insert(name.to_string(), json!(format!("{}...{}", first, last)));
                    }
                    Some(_v) => { let _ = obj.insert(name.to_string(), json!("***")); }
                    None => { let _ = obj.insert(name.to_string(), Value::Null); }
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
        "sample": results,
        "count": results.len(),
        "randomized": order_by.is_none(),
        "anonymized_columns": anonymize.unwrap_or_default(),
    }))
}
