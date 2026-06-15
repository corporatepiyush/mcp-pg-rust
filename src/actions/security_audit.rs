use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

pub async fn security_audit(client: &Client, _params: &Option<&Value>) -> MCPResult<Value> {
    let superusers: Vec<Value> = client.query(
        "SELECT rolname AS role,
                rolsuper AS is_superuser,
                rolcreatedb AS can_create_db,
                rolcreaterole AS can_create_role,
                rolcanlogin AS can_login,
                rolvaliduntil AS valid_until
         FROM pg_catalog.pg_roles
         WHERE rolsuper = true
         ORDER BY rolname",
        &[],
    ).await?.iter().map(|row| {
        json!({
            "role": row.get::<_, String>(0),
            "superuser": row.get::<_, bool>(1),
            "can_create_db": row.get::<_, bool>(2),
            "can_create_role": row.get::<_, bool>(3),
            "can_login": row.get::<_, bool>(4),
            "valid_until": row.get::<_, Option<String>>(5),
        })
    }).collect();

    let world_readable: Vec<Value> = client.query(
        "SELECT schemaname, tablename, tableowner,
                has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'SELECT') AS public_select,
                has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'INSERT') AS public_insert,
                has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'UPDATE') AS public_update,
                has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'DELETE') AS public_delete
         FROM pg_catalog.pg_tables
         WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
           AND (has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'SELECT')
             OR has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'INSERT')
             OR has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'UPDATE')
             OR has_table_privilege('PUBLIC', quote_ident(schemaname)||'.'||quote_ident(tablename), 'DELETE'))
         ORDER BY schemaname, tablename",
        &[],
    ).await?.iter().map(|row| {
        json!({
            "schema": row.get::<_, String>(0),
            "table": row.get::<_, String>(1),
            "owner": row.get::<_, String>(2),
            "public_select": row.get::<_, bool>(3),
            "public_insert": row.get::<_, bool>(4),
            "public_update": row.get::<_, bool>(5),
            "public_delete": row.get::<_, bool>(6),
        })
    }).collect();

    let default_privs: Vec<Value> = client.query(
        "SELECT pg_catalog.pg_get_userbyid(defacluser) AS grantee,
                n.nspname AS schema,
                CASE defaclobjtype
                    WHEN 'r' THEN 'table'
                    WHEN 'f' THEN 'function'
                    WHEN 'S' THEN 'sequence'
                    WHEN 'T' THEN 'type'
                    WHEN 'n' THEN 'schema'
                    ELSE defaclobjtype::text
                END AS object_type,
                pg_catalog.array_to_string(defaclacl, ', ') AS privileges
         FROM pg_catalog.pg_default_acl da
         JOIN pg_catalog.pg_namespace n ON n.oid = da.defaclnamespace
         ORDER BY grantee, schema, object_type",
        &[],
    ).await?.iter().map(|row| {
        json!({
            "role": row.get::<_, String>(0),
            "schema": row.get::<_, String>(1),
            "object_type": row.get::<_, String>(2),
            "privileges": row.get::<_, String>(3),
        })
    }).collect();

    Ok(json!({
        "superusers": superusers,
        "world_readable_tables": world_readable,
        "default_privileges": default_privs,
        "summary": {
            "superuser_count": superusers.len(),
            "world_readable_count": world_readable.len(),
            "has_default_privs": !default_privs.is_empty(),
        }
    }))
}

pub async fn audit_role_usage(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let role_filter = params.as_ref().and_then(|p| p.get("role").and_then(|v| v.as_str()));

    let roles: Vec<Value> = if let Some(role) = role_filter {
        client.query(
            "SELECT oid, rolname, rolsuper, rolinherit, rolcreaterole, rolcreatedb,
                    rolcanlogin, rolconnlimit, rolvaliduntil,
                    pg_catalog.shobj_description(oid, 'pg_authid') AS description
             FROM pg_catalog.pg_roles
             WHERE rolname = $1
             ORDER BY rolname",
            &[&role],
        ).await?.into_iter().map(|row| {
            json!({
                "oid": row.get::<_, i64>(0),
                "role": row.get::<_, String>(1),
                "superuser": row.get::<_, bool>(2),
                "inherit": row.get::<_, bool>(3),
                "can_create_role": row.get::<_, bool>(4),
                "can_create_db": row.get::<_, bool>(5),
                "can_login": row.get::<_, bool>(6),
                "connection_limit": row.get::<_, i32>(7),
                "valid_until": row.get::<_, Option<String>>(8),
                "description": row.get::<_, Option<String>>(9),
            })
        }).collect()
    } else {
        client.query(
            "SELECT oid, rolname, rolsuper, rolinherit, rolcreaterole, rolcreatedb,
                    rolcanlogin, rolconnlimit, rolvaliduntil,
                    pg_catalog.shobj_description(oid, 'pg_authid') AS description
             FROM pg_catalog.pg_roles
             ORDER BY rolname",
            &[],
        ).await?.into_iter().map(|row| {
            json!({
                "oid": row.get::<_, i64>(0),
                "role": row.get::<_, String>(1),
                "superuser": row.get::<_, bool>(2),
                "inherit": row.get::<_, bool>(3),
                "can_create_role": row.get::<_, bool>(4),
                "can_create_db": row.get::<_, bool>(5),
                "can_login": row.get::<_, bool>(6),
                "connection_limit": row.get::<_, i32>(7),
                "valid_until": row.get::<_, Option<String>>(8),
                "description": row.get::<_, Option<String>>(9),
            })
        }).collect()
    };

    Ok(json!({
        "roles": roles,
        "total_roles": roles.len(),
        "login_roles": roles.iter().filter(|r| r.get("can_login").and_then(|v| v.as_bool()).unwrap_or(false)).count(),
        "superuser_roles": roles.iter().filter(|r| r.get("superuser").and_then(|v| v.as_bool()).unwrap_or(false)).count(),
    }))
}
