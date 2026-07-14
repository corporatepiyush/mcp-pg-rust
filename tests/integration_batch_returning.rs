//! Contract tests for the batch `RETURNING` response shape.
//!
//! These pin the client-facing field names produced by the batch insert/delete
//! tools so they cannot silently drift again:
//!   - `async_batch_insert` with `returning` → `inserted_ids`
//!   - `async_batch_delete` with `returning` → `deleted_ids`
//!
//! Gated on `DATABASE_URL`: skipped (not failed) when no database is available,
//! so the suite stays green in environments without PostgreSQL. Run with:
//!   DATABASE_URL=postgres://user@localhost/db cargo test --test integration_batch_returning

use mcp_postgres::actions::batch::{async_batch_delete, async_batch_insert};
use serde_json::json;
use tokio_postgres::{Client, NoTls};

/// Connect to the database named by `DATABASE_URL`, or return `None` to skip.
async fn connect() -> Option<Client> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let (client, connection) = tokio_postgres::connect(&url, NoTls)
        .await
        .expect("DATABASE_URL is set but connecting failed");
    tokio::spawn(connection);
    Some(client)
}

#[tokio::test]
async fn batch_insert_and_delete_returning_field_names() {
    let Some(client) = connect().await else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };

    let table = "mcp_batch_returning_test";
    client
        .batch_execute(&format!(
            "DROP TABLE IF EXISTS {table}; \
             CREATE TABLE {table} (id SERIAL PRIMARY KEY, name TEXT, val INT)"
        ))
        .await
        .unwrap();

    // ── insert … RETURNING id → "inserted_ids" ──
    let ins_args = json!({
        "table": table,
        "columns": ["name", "val"],
        "rows": [["a", 1], ["b", 2], ["c", 3]],
        "returning": "id",
    });
    let ins = async_batch_insert(&client, &Some(&ins_args))
        .await
        .expect("insert failed");

    assert_eq!(ins["rows_affected"], 3);
    let inserted = ins["inserted_ids"]
        .as_array()
        .expect("insert with RETURNING must expose `inserted_ids`");
    assert_eq!(inserted.len(), 3);
    assert!(
        ins.get("returned_ids").is_none() && ins.get("deleted_ids").is_none(),
        "insert response must use only `inserted_ids`, got: {ins}"
    );

    // ── delete … RETURNING id → "deleted_ids" (regression: was "inserted_ids") ──
    let del_args = json!({
        "table": table,
        "where_clauses": [{ "column": "val", "op": ">=", "value": 2 }],
        "returning": "id",
    });
    let del = async_batch_delete(&client, &Some(&del_args))
        .await
        .expect("delete failed");

    assert_eq!(del["rows_affected"], 2);
    let deleted = del["deleted_ids"]
        .as_array()
        .expect("delete with RETURNING must expose `deleted_ids`");
    assert_eq!(deleted.len(), 2);
    assert!(
        del.get("inserted_ids").is_none(),
        "delete response must NOT mislabel its ids as `inserted_ids`, got: {del}"
    );

    // ── delete without RETURNING → rows_affected only, no id array ──
    let del_plain_args = json!({
        "table": table,
        "where_clauses": [{ "column": "val", "op": "=", "value": 1 }],
    });
    let del_plain = async_batch_delete(&client, &Some(&del_plain_args))
        .await
        .expect("plain delete failed");
    assert_eq!(del_plain["rows_affected"], 1);
    assert!(del_plain.get("deleted_ids").is_none());

    client
        .batch_execute(&format!("DROP TABLE IF EXISTS {table}"))
        .await
        .unwrap();
}
