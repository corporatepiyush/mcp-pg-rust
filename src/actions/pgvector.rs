use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

pub async fn list_vector_columns(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT c.table_schema, c.table_name, c.column_name, c.data_type,
                    e.udt_name
             FROM information_schema.columns c
             JOIN information_schema.element_types e ON (c.table_catalog, c.table_schema, c.table_name, c.column_name, c.dtd_identifier)
             WHERE c.data_type = 'USER-DEFINED'
               AND e.udt_name = 'vector'
             ORDER BY c.table_schema, c.table_name, c.ordinal_position",
            &[],
        )
        .await
        ?;

    let columns: Vec<Value> = rows.iter().map(|row| {
        json!({
            "schema": row.get::<_, String>(0),
            "table": row.get::<_, String>(1),
            "column": row.get::<_, String>(2),
            "type": "vector",
        })
    }).collect();

    Ok(json!({ "vector_columns": columns }))
}

pub async fn vector_search(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params.as_ref().and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let vector = params.as_ref().and_then(|p| p.get("vector").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'vector' parameter (e.g. '[0.1,0.2,0.3]')".into()))?;
    let limit = params.as_ref().and_then(|p| p.get("limit").and_then(|v| v.as_i64())).unwrap_or(10);
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");
    let select_cols = params.as_ref().and_then(|p| p.get("select").and_then(|v| v.as_str())).unwrap_or("*");
    let distance = params.as_ref().and_then(|p| p.get("distance").and_then(|v| v.as_str())).unwrap_or("cosine");

    let operator = match distance {
        "l2" | "euclidean" => "<->",
        "inner" | "ip" => "<#>",
        _ => "<=>",
    };

    let qcol = crate::validation::quote_ident(column);
    let qual = format!("{}.{}", crate::validation::quote_ident(schema), crate::validation::quote_ident(table));
    let sql = format!(
        "SELECT {}, {qcol} {operator} '{vector}' AS distance
         FROM {qual}
         ORDER BY {qcol} {operator} '{vector}'
         LIMIT {}",
        select_cols,
        limit.min(1000)
    );

    let rows = client.query(&sql, &[]).await?;

    let mut results = Vec::new();
    for row in &rows {
        let mut obj = serde_json::Map::new();
        for (i, col) in row.columns().iter().enumerate() {
            let name = col.name();
            if let Ok(v) = row.try_get::<_, Value>(i) {
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
        "distance_metric": distance,
    }))
}

pub async fn create_vector_index(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let table = params.as_ref().and_then(|p| p.get("table").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'table'".into()))?;
    let column = params.as_ref().and_then(|p| p.get("column").and_then(|v| v.as_str()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'column'".into()))?;
    let index_type = params.as_ref().and_then(|p| p.get("index_type").and_then(|v| v.as_str())).unwrap_or("hnsw");
    let distance = params.as_ref().and_then(|p| p.get("distance").and_then(|v| v.as_str())).unwrap_or("cosine");
    let schema = params.as_ref().and_then(|p| p.get("schema").and_then(|v| v.as_str())).unwrap_or("public");

    let distance_op = match distance {
        "l2" | "euclidean" => "vector_l2_ops",
        "inner" | "ip" => "vector_ip_ops",
        _ => "vector_cosine_ops",
    };

    let index_name = format!("idx_{}_{}_{}", table, column, index_type);

    let q_schema = crate::validation::quote_ident(schema);
    let q_table = crate::validation::quote_ident(table);
    let q_column = crate::validation::quote_ident(column);
    let sql = match index_type {
        "ivfflat" => {
            let lists = params.as_ref().and_then(|p| p.get("lists").and_then(|v| v.as_i64())).unwrap_or(100);
            format!(
                "CREATE INDEX \"{index_name}\" ON {q_schema}.{q_table} USING ivfflat ({q_column} {distance_op}) WITH (lists = {lists})"
            )
        }
        _ => {
            format!(
                "CREATE INDEX \"{index_name}\" ON {q_schema}.{q_table} USING hnsw ({q_column} {distance_op})"
            )
        }
    };

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "index": index_name, "sql": sql }))
}
