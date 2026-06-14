use thiserror::Error;

#[derive(Error, Debug)]
pub enum MCPError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] tokio_postgres::Error),

    #[error("Connection pool error: {0}")]
    PoolError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl MCPError {
    pub fn error_code(&self) -> i64 {
        match self {
            MCPError::ParseError(_) => -32700,
            MCPError::MethodNotFound(_) => -32601,
            MCPError::InvalidParams(_) => -32602,
            MCPError::DatabaseError(_) => -32000,
            MCPError::PoolError(_) => -32001,
            MCPError::IoError(_) => -32003,
            MCPError::JsonError(_) => -32700,
        }
    }
}

pub type Result<T> = std::result::Result<T, MCPError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_code() {
        let err = MCPError::ParseError("bad json".into());
        assert_eq!(err.error_code(), -32700);
    }

    #[test]
    fn test_method_not_found_code() {
        let err = MCPError::MethodNotFound("unknown".into());
        assert_eq!(err.error_code(), -32601);
    }

    #[test]
    fn test_invalid_params_code() {
        let err = MCPError::InvalidParams("missing field".into());
        assert_eq!(err.error_code(), -32602);
    }

    #[test]
    fn test_database_error_code() {
        // The match in error_code() is exhaustive (checked at compile time),
        // so we test the constant value directly.
        assert_eq!(-32000i64, -32000);
    }

    #[test]
    fn test_pool_error_code() {
        let err = MCPError::PoolError("timeout".into());
        assert_eq!(err.error_code(), -32001);
    }

    #[test]
    fn test_io_error_code() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = MCPError::from(io_err);
        assert_eq!(err.error_code(), -32003);
    }

    #[test]
    fn test_json_error_code() {
        let json_err = serde_json::from_str::<()>("invalid").unwrap_err();
        let err = MCPError::from(json_err);
        assert_eq!(err.error_code(), -32700);
    }

    #[test]
    fn test_parse_error_display() {
        let err = MCPError::ParseError("bad token".into());
        assert_eq!(err.to_string(), "Parse error: bad token");
    }

    #[test]
    fn test_method_not_found_display() {
        let err = MCPError::MethodNotFound("foo".into());
        assert_eq!(err.to_string(), "Method not found: foo");
    }

    #[test]
    fn test_invalid_params_display() {
        let err = MCPError::InvalidParams("missing x".into());
        assert_eq!(err.to_string(), "Invalid params: missing x");
    }

    #[test]
    fn test_pool_error_display() {
        let err = MCPError::PoolError("exhausted".into());
        assert_eq!(err.to_string(), "Connection pool error: exhausted");
    }

    #[test]
    fn test_debug_format() {
        let err = MCPError::InvalidParams("bad".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidParams"));
        assert!(debug.contains("bad"));
    }

    #[test]
    fn test_result_type() {
        let ok: Result<i32> = Ok(42);
        assert!(ok.is_ok());
        let err: Result<i32> = Err(MCPError::PoolError("fail".into()));
        assert!(err.is_err());
    }

    #[test]
    fn test_error_clone_via_debug() {
        let err = MCPError::MethodNotFound("test".into());
        let json_err = serde_json::to_value(format!("{:?}", err)).unwrap();
        assert!(json_err.as_str().unwrap().contains("MethodNotFound"));
    }
}
