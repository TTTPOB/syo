use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};

// Maximum graph traversal depth accepted from MCP arguments. The graph
// neighborhood traversal already has a 500-node / 1000-edge ceiling, but a
// pathological `depth` value still wastes CPU walking empty levels. 8 hops
// is far beyond any realistic neighborhood query; larger values almost
// always indicate misuse.
pub const MAX_GRAPH_DEPTH: u64 = 8;

// Maximum page size accepted by paginated MCP tools. Mirrors the spirit of
// `MAX_SEARCH_LIMIT` (1000) — a request for more than this defeats the
// pagination contract and risks unbounded payloads.
pub const MAX_PAGE_SIZE: u64 = 1000;

// Convert an anyhow error (from syo-core layer) into an MCP error.
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

// Wrap a response payload with an agent-readable hint explaining post-call expectations.
// Tools that do NOT need a hint should return the bare payload directly.
pub fn with_hint(payload: Value, hint: &str) -> Value {
    serde_json::json!({ "data": payload, "_hint": hint })
}

/// Resolve a user-supplied string (id or name) to a [`NotebookId`].
///
/// Wraps `syo_core::notebook::resolve_notebook_id` and maps errors to MCP
/// `invalid_params` so every handler doesn't repeat the error conversion.
pub async fn resolve_notebook_id(
    client: &SiyuanClient,
    input: &str,
) -> Result<NotebookId, McpError> {
    syo_core::notebook::resolve_notebook_id(client, input)
        .await
        .map_err(|e| McpError::invalid_params(format!("invalid notebook: {:#}", e), None))
}

/// Treat whitespace-only inputs as absent.
pub(crate) fn is_present(s: Option<&str>) -> bool {
    s.is_some_and(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    // The cap constants are part of the MCP tool contract and are referenced
    // verbatim in the registry tool descriptions ("depth is capped at 8",
    // "page_size is capped at 1000"). These sentinel tests fail loudly if a
    // future change moves the cap without updating the description.
    use super::{MAX_GRAPH_DEPTH, MAX_PAGE_SIZE};

    #[test]
    fn graph_depth_cap_is_eight() {
        assert_eq!(MAX_GRAPH_DEPTH, 8);
    }

    #[test]
    fn page_size_cap_is_one_thousand() {
        assert_eq!(MAX_PAGE_SIZE, 1000);
    }
}
