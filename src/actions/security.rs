use serde_json::{json, Value};
use tokio_postgres::Client;
use crate::errors::Result as MCPResult;

/// 26. List users
pub async fn list_users(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT usename, usesuper, usecreatedb, usecanlogin, valuntil
             FROM pg_user
             ORDER BY usename",
            &[],
        )
        .await?;

    let users: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "username": row.get::<_, String>(0),
                "superuser": row.get::<_, bool>(1),
                "createdb": row.get::<_, bool>(2),
                "canlogin": row.get::<_, bool>(3),
                "valid_until": row.get::<_, Option<String>>(4),
            })
        })
        .collect();

    Ok(json!({ "users": users }))
}

/// 27. List user privileges
pub async fn list_user_privileges(client: &Client, params: Option<Value>) -> MCPResult<Value> {
    let username = params
        .and_then(|p| p.get("username").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or_else(|| crate::errors::MCPError::InvalidParams("Missing 'username' parameter".into()))?;

    let rows = client
        .query(
            "SELECT grantee, table_schema, table_name, privilege_type
             FROM information_schema.role_table_grants
             WHERE grantee = $1
             ORDER BY table_schema, table_name, privilege_type",
            &[&username],
        )
        .await?;

    let privileges: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "grantee": row.get::<_, String>(0),
                "schema": row.get::<_, String>(1),
                "table": row.get::<_, String>(2),
                "privilege": row.get::<_, String>(3),
            })
        })
        .collect();

    Ok(json!({ "privileges": privileges }))
}

/// 28. List role memberships
pub async fn list_role_memberships(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT member.usename as member, role.usename as role, admin_option
             FROM pg_auth_members
             JOIN pg_user member ON member.usesysid = pg_auth_members.member
             JOIN pg_user role ON role.usesysid = pg_auth_members.roleid
             ORDER BY member.usename, role.usename",
            &[],
        )
        .await?;

    let memberships: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "member": row.get::<_, String>(0),
                "role": row.get::<_, String>(1),
                "admin": row.get::<_, bool>(2),
            })
        })
        .collect();

    Ok(json!({ "memberships": memberships }))
}

/// 29. List database privileges
pub async fn list_database_privileges(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT datname, datacl::text
             FROM pg_database
             WHERE datistemplate = false
             ORDER BY datname",
            &[],
        )
        .await?;

    let databases: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "database": row.get::<_, String>(0),
                "acl": row.get::<_, Option<String>>(1),
            })
        })
        .collect();

    Ok(json!({ "databases": databases }))
}

/// 30. Show session info
pub async fn show_session_info(client: &Client, _params: Option<Value>) -> MCPResult<Value> {
    let rows = client
        .query(
            "SELECT current_user, current_database(), inet_client_addr()::text,
                    inet_client_port(), inet_server_addr()::text, inet_server_port()",
            &[],
        )
        .await?;

    let row = &rows[0];

    Ok(json!({
        "current_user": row.get::<_, String>(0),
        "current_database": row.get::<_, String>(1),
        "client_address": row.get::<_, Option<String>>(2),
        "client_port": row.get::<_, Option<i32>>(3),
        "server_address": row.get::<_, Option<String>>(4),
        "server_port": row.get::<_, Option<i32>>(5),
    }))
}
