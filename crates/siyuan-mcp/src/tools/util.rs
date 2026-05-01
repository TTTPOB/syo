use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};

use siyuan_types::SiyuanError;

// Convert a SiYuan domain error into a meaningful MCP error.
pub fn siyuan_to_mcp(e: SiyuanError) -> McpError {
    use SiyuanError as E;
    match e {
        E::Auth => McpError::invalid_request("authentication failed", None),
        E::NotFound(msg) => McpError::invalid_params(format!("not found: {msg}"), None),
        E::AmbiguousPath { hpath, candidates } => {
            McpError::invalid_params(format!("ambiguous path {hpath}: {candidates:?}"), None)
        }
        E::Parse(msg) => McpError::invalid_params(format!("parse error: {msg}"), None),
        E::Api { code, msg } => {
            McpError::internal_error(format!("siyuan kernel: code={code}: {msg}"), None)
        }
        E::SqlUnavailable => {
            McpError::internal_error("siyuan SQL endpoint disabled (publish mode?)", None)
        }
        other => McpError::internal_error(other.to_string(), None),
    }
}

// Convert an anyhow error (from model layer) into an MCP error.
pub fn anyhow_to_mcp(e: anyhow::Error) -> McpError {
    McpError::internal_error(format!("{:#}", e), None)
}

// Validate that args is a JSON object; return an empty map for null args.
pub fn ensure_object(args: Value) -> Result<Map<String, Value>, McpError> {
    match args {
        Value::Object(m) => Ok(m),
        Value::Null => Ok(Map::new()),
        _ => Err(McpError::invalid_params(
            "arguments must be a JSON object",
            None,
        )),
    }
}

pub fn required_string(map: &Map<String, Value>, key: &str) -> Result<String, McpError> {
    map.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| McpError::invalid_params(format!("missing or invalid `{key}`"), None))
}

pub fn optional_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

pub fn optional_u64(map: &Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key).and_then(|v| v.as_u64())
}

pub fn string_array(map: &Map<String, Value>, key: &str) -> Result<Vec<String>, McpError> {
    match map.get(key) {
        None => Err(McpError::invalid_params(format!("missing `{key}`"), None)),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|v| {
                v.as_str().map(str::to_owned).ok_or_else(|| {
                    McpError::invalid_params(format!("`{key}` must be array of strings"), None)
                })
            })
            .collect(),
        _ => Err(McpError::invalid_params(
            format!("`{key}` must be an array"),
            None,
        )),
    }
}

pub fn object_field(map: &Map<String, Value>, key: &str) -> Result<Map<String, Value>, McpError> {
    match map.get(key) {
        Some(Value::Object(m)) => Ok(m.clone()),
        Some(_) => Err(McpError::invalid_params(
            format!("`{key}` must be an object"),
            None,
        )),
        None => Err(McpError::invalid_params(format!("missing `{key}`"), None)),
    }
}
