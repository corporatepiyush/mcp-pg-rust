use crate::errors::MCPError;

const MAX_IDENTIFIER_LEN: usize = 255;

pub fn validate_identifier(name: &str, label: &str) -> Result<(), MCPError> {
    if name.is_empty() {
        return Err(MCPError::InvalidParams(format!(
            "'{label}' must not be empty"
        )));
    }
    if name.len() > MAX_IDENTIFIER_LEN {
        return Err(MCPError::InvalidParams(format!(
            "'{label}' exceeds maximum length of {MAX_IDENTIFIER_LEN} characters (got {})",
            name.len()
        )));
    }
    for ch in name.chars() {
        if !ch.is_alphanumeric() && ch != '_' {
            return Err(MCPError::InvalidParams(format!(
                "'{label}' contains invalid character '{ch}' — only alphanumeric and underscore allowed"
            )));
        }
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(MCPError::InvalidParams(format!(
            "'{label}' must not start with a digit"
        )));
    }
    Ok(())
}

pub fn quote_identifier(name: &str) -> String {
    quote_ident(name)
}

/// PostgreSQL privilege keywords accepted by GRANT/REVOKE.
const VALID_PRIVILEGES: &[&str] = &[
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "TRUNCATE",
    "REFERENCES",
    "TRIGGER",
    "CREATE",
    "CONNECT",
    "TEMPORARY",
    "TEMP",
    "EXECUTE",
    "USAGE",
    "MAINTAIN",
    "ALL",
    "ALL PRIVILEGES",
];

/// Validate a privilege specification for GRANT/REVOKE. Accepts a
/// comma-separated list of known privilege keywords (case-insensitive), e.g.
/// `"SELECT, INSERT"` or `"ALL PRIVILEGES"`. Rejects anything else so the
/// value can be safely interpolated into the statement.
pub fn validate_privilege_list(privilege: &str) -> Result<(), MCPError> {
    let trimmed = privilege.trim();
    if trimmed.is_empty() {
        return Err(MCPError::InvalidParams(
            "'privilege' must not be empty".into(),
        ));
    }
    for part in trimmed.split(',') {
        let token = part.trim().to_ascii_uppercase();
        if !VALID_PRIVILEGES.contains(&token.as_str()) {
            return Err(MCPError::InvalidParams(format!(
                "Invalid privilege '{}'. Allowed: {}",
                part.trim(),
                VALID_PRIVILEGES.join(", ")
            )));
        }
    }
    Ok(())
}

/// Quote a PostgreSQL identifier, escaping embedded double-quotes.
/// Use this instead of duplicating `format!("\"{}\"", s.replace('"', "\"\""))` in every module.
pub fn quote_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for ch in name.chars() {
        if ch == '"' {
            out.push_str("\"\"");
        } else {
            out.push(ch);
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_identifier() {
        assert!(validate_identifier("users", "table").is_ok());
        assert!(validate_identifier("user_orders_2024", "table").is_ok());
    }

    #[test]
    fn test_empty_identifier() {
        let err = validate_identifier("", "table").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_too_long_identifier() {
        let long = "a".repeat(256);
        let err = validate_identifier(&long, "table").unwrap_err();
        assert!(err.to_string().contains("exceeds maximum length"));
    }

    #[test]
    fn test_invalid_char_identifier() {
        let err = validate_identifier("users; DROP TABLE", "table").unwrap_err();
        assert!(err.to_string().contains("invalid character"));
    }

    #[test]
    fn test_digit_start_identifier() {
        let err = validate_identifier("1users", "table").unwrap_err();
        assert!(err.to_string().contains("must not start with a digit"));
    }

    #[test]
    fn test_quote_identifier() {
        assert_eq!(quote_identifier("users"), "\"users\"");
        assert_eq!(quote_identifier("order_items"), "\"order_items\"");
    }

    #[test]
    fn test_validate_privilege_list_valid() {
        assert!(validate_privilege_list("SELECT").is_ok());
        assert!(validate_privilege_list("select, insert").is_ok());
        assert!(validate_privilege_list("ALL PRIVILEGES").is_ok());
        assert!(validate_privilege_list("SELECT, UPDATE, DELETE").is_ok());
    }

    #[test]
    fn test_validate_privilege_list_rejects_injection() {
        let err = validate_privilege_list("SELECT ON pg_authid TO attacker; --").unwrap_err();
        assert!(err.to_string().contains("Invalid privilege"));
        assert!(validate_privilege_list("").is_err());
        assert!(validate_privilege_list("DROP").is_err());
    }
}
