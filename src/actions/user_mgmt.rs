use crate::errors::Result as MCPResult;
use serde_json::{Value, json};
use tokio_postgres::Client;

const MAX_IDENTIFIER_LEN: usize = 255;
const MAX_PASSWORD_LEN: usize = 1024;

/// Reject passwords containing control characters (NUL, newline, carriage
/// return). They are escaped for quotes when interpolated into CREATE/ALTER
/// statements, but control characters can still corrupt the statement, so they
/// are disallowed outright.
fn validate_password(pw: &str) -> MCPResult<()> {
    if pw.len() > MAX_PASSWORD_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'password' exceeds maximum length of {MAX_PASSWORD_LEN} characters"
        )));
    }
    if pw.chars().any(|c| c.is_control()) {
        return Err(crate::errors::MCPError::InvalidParams(
            "'password' must not contain control characters".into(),
        ));
    }
    Ok(())
}

pub async fn create_user(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let username = params
        .as_ref()
        .and_then(|p| p.get("username").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'username' parameter".into())
        })?;

    if username.is_empty() || username.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'username' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let password = params
        .as_ref()
        .and_then(|p| p.get("password").and_then(|v| v.as_str()));
    let valid_until = params
        .as_ref()
        .and_then(|p| p.get("valid_until").and_then(|v| v.as_str()));
    let connection_limit = params
        .as_ref()
        .and_then(|p| p.get("connection_limit").and_then(|v| v.as_i64()));
    let can_login = params
        .as_ref()
        .and_then(|p| p.get("can_login").and_then(|v| v.as_bool()));

    let mut sql = format!("CREATE USER {}", quote_ident(username));
    if let Some(pw) = password {
        validate_password(pw)?;
        sql.push_str(&format!(" PASSWORD '{}'", pw.replace('\'', "''")));
    }
    if let Some(limit) = connection_limit {
        sql.push_str(&format!(" CONNECTION LIMIT {}", limit));
    }
    if let Some(login) = can_login {
        if login {
            sql.push_str(" LOGIN");
        } else {
            sql.push_str(" NOLOGIN");
        }
    }
    if let Some(until) = valid_until {
        sql.push_str(&format!(" VALID UNTIL '{}'", until.replace('\'', "''")));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "user": username }))
}

pub async fn alter_user(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let username = params
        .as_ref()
        .and_then(|p| p.get("username").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'username' parameter".into())
        })?;

    if username.is_empty() || username.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'username' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let password = params
        .as_ref()
        .and_then(|p| p.get("password").and_then(|v| v.as_str()));
    let valid_until = params
        .as_ref()
        .and_then(|p| p.get("valid_until").and_then(|v| v.as_str()));
    let connection_limit = params
        .as_ref()
        .and_then(|p| p.get("connection_limit").and_then(|v| v.as_i64()));
    let can_login = params
        .as_ref()
        .and_then(|p| p.get("can_login").and_then(|v| v.as_bool()));
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()));

    if password.is_none()
        && valid_until.is_none()
        && connection_limit.is_none()
        && can_login.is_none()
        && new_name.is_none()
    {
        return Err(crate::errors::MCPError::InvalidParams(
            "No attributes specified to alter".into(),
        ));
    }

    let mut sql = format!("ALTER USER {}", quote_ident(username));
    if let Some(pw) = password {
        validate_password(pw)?;
        sql.push_str(&format!(" PASSWORD '{}'", pw.replace('\'', "''")));
    }
    if let Some(limit) = connection_limit {
        sql.push_str(&format!(" CONNECTION LIMIT {}", limit));
    }
    if let Some(login) = can_login {
        if login {
            sql.push_str(" LOGIN");
        } else {
            sql.push_str(" NOLOGIN");
        }
    }
    if let Some(until) = valid_until {
        sql.push_str(&format!(" VALID UNTIL '{}'", until.replace('\'', "''")));
    }
    if let Some(name) = new_name {
        sql.push_str(&format!(" RENAME TO {}", quote_ident(name)));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "user": username }))
}

pub async fn drop_user(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let username = params
        .as_ref()
        .and_then(|p| p.get("username").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'username' parameter".into())
        })?;

    if username.is_empty() || username.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'username' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let mut sql = "DROP USER".to_string();
    if if_exists {
        sql.push_str(" IF EXISTS");
    }
    sql.push_str(&format!(" {}", quote_ident(username)));

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "user": username }))
}

pub async fn create_role(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let role_name = params
        .as_ref()
        .and_then(|p| p.get("role_name").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'role_name' parameter".into())
        })?;

    if role_name.is_empty() || role_name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'role_name' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let with_login = params
        .as_ref()
        .and_then(|p| p.get("with_login").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let mut sql = format!("CREATE ROLE {}", quote_ident(role_name));
    sql.push_str(if with_login { " LOGIN" } else { " NOLOGIN" });

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "role": role_name }))
}

pub async fn alter_role(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let role_name = params
        .as_ref()
        .and_then(|p| p.get("role_name").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'role_name' parameter".into())
        })?;

    if role_name.is_empty() || role_name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'role_name' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let password = params
        .as_ref()
        .and_then(|p| p.get("password").and_then(|v| v.as_str()));
    let can_login = params
        .as_ref()
        .and_then(|p| p.get("can_login").and_then(|v| v.as_bool()));
    let superuser = params
        .as_ref()
        .and_then(|p| p.get("superuser").and_then(|v| v.as_bool()));
    let createdb = params
        .as_ref()
        .and_then(|p| p.get("createdb").and_then(|v| v.as_bool()));
    let new_name = params
        .as_ref()
        .and_then(|p| p.get("new_name").and_then(|v| v.as_str()));

    let mut sql = format!("ALTER ROLE {}", quote_ident(role_name));
    if let Some(pw) = password {
        validate_password(pw)?;
        sql.push_str(&format!(" PASSWORD '{}'", pw.replace('\'', "''")));
    }
    if let Some(login) = can_login {
        sql.push_str(if login { " LOGIN" } else { " NOLOGIN" });
    }
    if let Some(su) = superuser {
        sql.push_str(if su { " SUPERUSER" } else { " NOSUPERUSER" });
    }
    if let Some(db) = createdb {
        sql.push_str(if db { " CREATEDB" } else { " NOCREATEDB" });
    }
    if let Some(name) = new_name {
        sql.push_str(&format!(" RENAME TO {}", quote_ident(name)));
    }

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "role": role_name }))
}

pub async fn drop_role(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let role_name = params
        .as_ref()
        .and_then(|p| p.get("role_name").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'role_name' parameter".into())
        })?;

    if role_name.is_empty() || role_name.len() > MAX_IDENTIFIER_LEN {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "'role_name' must be 1-{MAX_IDENTIFIER_LEN} characters"
        )));
    }

    let if_exists = params
        .as_ref()
        .and_then(|p| p.get("if_exists").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let mut sql = "DROP ROLE".to_string();
    if if_exists {
        sql.push_str(" IF EXISTS");
    }
    sql.push_str(&format!(" {}", quote_ident(role_name)));

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "role": role_name }))
}

pub async fn grant_privileges(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let privilege = params
        .as_ref()
        .and_then(|p| p.get("privilege").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'privilege' parameter".into())
        })?;
    let object_type = params
        .as_ref()
        .and_then(|p| p.get("object_type").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'object_type' parameter".into())
        })?;
    let object_name = params
        .as_ref()
        .and_then(|p| p.get("object_name").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'object_name' parameter".into())
        })?;
    let grantee = params
        .as_ref()
        .and_then(|p| p.get("grantee").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'grantee' parameter".into())
        })?;

    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    crate::validation::validate_privilege_list(privilege)?;

    let valid_types = [
        "table",
        "sequence",
        "schema",
        "database",
        "all_tables_in_schema",
    ];
    if !valid_types.contains(&object_type) {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "Unsupported object_type '{}'. Use: {:?}",
            object_type, valid_types
        )));
    }

    let sql = match object_type {
        "all_tables_in_schema" => format!(
            "GRANT {} ON ALL TABLES IN SCHEMA {} TO {}",
            privilege,
            quote_ident(schema),
            quote_ident(grantee)
        ),
        _ => format!(
            "GRANT {} ON {} {} TO {}",
            privilege,
            object_type.to_uppercase(),
            quote_ident(object_name),
            quote_ident(grantee)
        ),
    };

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

pub async fn revoke_privileges(client: &Client, params: &Option<&Value>) -> MCPResult<Value> {
    let privilege = params
        .as_ref()
        .and_then(|p| p.get("privilege").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'privilege' parameter".into())
        })?;
    let object_type = params
        .as_ref()
        .and_then(|p| p.get("object_type").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'object_type' parameter".into())
        })?;
    let object_name = params
        .as_ref()
        .and_then(|p| p.get("object_name").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'object_name' parameter".into())
        })?;
    let revokee = params
        .as_ref()
        .and_then(|p| p.get("revokee").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            crate::errors::MCPError::InvalidParams("Missing 'revokee' parameter".into())
        })?;

    let schema = params
        .as_ref()
        .and_then(|p| p.get("schema").and_then(|v| v.as_str()))
        .unwrap_or("public");

    crate::validation::validate_privilege_list(privilege)?;

    let valid_types = [
        "table",
        "sequence",
        "schema",
        "database",
        "all_tables_in_schema",
    ];
    if !valid_types.contains(&object_type) {
        return Err(crate::errors::MCPError::InvalidParams(format!(
            "Unsupported object_type '{}'. Use: {:?}",
            object_type, valid_types
        )));
    }

    let sql = match object_type {
        "all_tables_in_schema" => format!(
            "REVOKE {} ON ALL TABLES IN SCHEMA {} FROM {}",
            privilege,
            quote_ident(schema),
            quote_ident(revokee)
        ),
        _ => format!(
            "REVOKE {} ON {} {} FROM {}",
            privilege,
            object_type.to_uppercase(),
            quote_ident(object_name),
            quote_ident(revokee)
        ),
    };

    client.execute(&sql, &[]).await?;
    Ok(json!({ "success": true, "sql": sql }))
}

fn quote_ident(ident: &str) -> String {
    crate::validation::quote_ident(ident)
}
