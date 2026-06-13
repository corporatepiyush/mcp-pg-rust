use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcRequest {}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_serialize_deserialize() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test_method".to_string(),
            params: Some(json!({"key": "value"})),
            id: Some(Value::Number(1.into())),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.method, "test_method");
        assert_eq!(deserialized.params, Some(json!({"key": "value"})));
        assert_eq!(deserialized.id, Some(Value::Number(1.into())));
    }

    #[test]
    fn test_request_without_params() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert!(deserialized.params.is_none());
    }

    #[test]
    fn test_request_with_string_id() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: Some(Value::String("req-1".into())),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, Some(Value::String("req-1".into())));
    }

    #[test]
    fn test_response_success() {
        let resp = JsonRpcResponse::success(Some(Value::Number(1.into())), json!({"result": "ok"}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.result, Some(json!({"result": "ok"})));
        assert!(resp.error.is_none());
        assert_eq!(resp.id, Some(Value::Number(1.into())));
    }

    #[test]
    fn test_response_success_null_id() {
        let resp = JsonRpcResponse::success(None, json!("ok"));
        assert!(resp.id.is_none());
    }

    #[test]
    fn test_response_error() {
        let resp = JsonRpcResponse::error(Some(Value::Number(1.into())), -32700, "Parse error".into());
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32700);
        assert_eq!(err.message, "Parse error");
        assert!(err.data.is_none());
    }

    #[test]
    fn test_response_error_with_null_id() {
        let resp = JsonRpcResponse::error(None, -32601, "Not found".into());
        assert!(resp.id.is_none());
    }

    #[test]
    fn test_response_serde_roundtrip() {
        let resp = JsonRpcResponse::success(Some(Value::Number(42.into())), json!([1, 2, 3]));
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.result, Some(json!([1, 2, 3])));
        assert_eq!(deserialized.id, Some(Value::Number(42.into())));
    }

    #[test]
    fn test_error_serde_roundtrip() {
        let resp = JsonRpcResponse::error(Some(json!("abc")), -32000, "DB error".into());
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        let err = deserialized.error.unwrap();
        assert_eq!(err.code, -32000);
        assert_eq!(err.message, "DB error");
    }

    #[test]
    fn test_minimal_request() {
        let json = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "ping");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_with_null_params() {
        let json = r#"{"jsonrpc":"2.0","method":"ping","params":null,"id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_with_num_id_zero() {
        let json = r#"{"jsonrpc":"2.0","method":"initialize","id":0}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(Value::Number(0.into())));
    }

    #[test]
    fn test_response_with_error_data() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32000,
                message: "custom error".into(),
                data: Some(json!({"detail": "something broke"})),
            }),
            id: Some(Value::Number(1.into())),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        let err = deserialized.error.unwrap();
        assert_eq!(err.data, Some(json!({"detail": "something broke"})));
    }
}

