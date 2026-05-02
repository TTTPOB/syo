use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};

use siyuan_types::SiyuanError;

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
        E::Api { code, msg } => classify_api_msg(&msg).unwrap_or_else(|| {
            McpError::internal_error(format!("siyuan kernel: code={code}: {msg}"), None)
        }),
        E::SqlUnavailable => {
            McpError::internal_error("siyuan SQL endpoint disabled (publish mode?)", None)
        }
        other => McpError::internal_error(other.to_string(), None),
    }
}

// Recognise well-known kernel error message shapes and remap them to typed
// MCP error kinds. Returning `None` preserves the generic internal_error
// fallback in `siyuan_to_mcp`. The patterns below are documented in the
// SiYuan kernel source (Lang 0/15/34 and a handful of hard-coded strings)
// and were verified during the Round 3 research pass.
fn classify_api_msg(msg: &str) -> Option<McpError> {
    // Auth — kernel emits HTTP 401 with code=-1 and msg starting with
    // "Auth failed"; 401 short-circuits to SiyuanError::Auth in client.rs,
    // but a non-401 response carrying the same prefix is still surfaced.
    if msg.starts_with("Auth failed") {
        return Some(McpError::invalid_request("authentication failed", None));
    }

    // Not-found family: block / tree / Lang(15) "Content block with id [..]".
    if msg == "block not found"
        || msg == "tree not found"
        || msg.contains("Content block with id [")
    {
        return Some(McpError::invalid_params(format!("not found: {msg}"), None));
    }

    // Not-found family: notebook variants — Lang(0) localised string and
    // two literal forms emitted by the kernel.
    if msg == "Query notebook failed"
        || msg == "notebook not found"
        || (msg.starts_with("opened notebook [") && msg.ends_with("] not found"))
    {
        return Some(McpError::invalid_params(format!("not found: {msg}"), None));
    }

    // Caller-side input rejected by the kernel: invalid id, missing/typed
    // field, or generic request parse failure.
    if msg == "invalid ID argument"
        || msg.starts_with("Field [")
        || msg.starts_with("Parses request [")
    {
        return Some(McpError::invalid_params(
            format!("kernel rejected request: {msg}"),
            None,
        ));
    }

    None
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

// Wrap a response payload with an agent-readable hint explaining post-call expectations.
// Tools that do NOT need a hint should return the bare payload directly.
pub fn with_hint(payload: Value, hint: &str) -> Value {
    serde_json::json!({ "data": payload, "_hint": hint })
}

#[cfg(test)]
mod tests {
    // The cap constants are part of the MCP tool contract and are referenced
    // verbatim in the registry tool descriptions ("depth is capped at 8",
    // "page_size is capped at 1000"). These sentinel tests fail loudly if a
    // future change moves the cap without updating the description.
    use super::{MAX_GRAPH_DEPTH, MAX_PAGE_SIZE, classify_api_msg};

    #[test]
    fn graph_depth_cap_is_eight() {
        assert_eq!(MAX_GRAPH_DEPTH, 8);
    }

    #[test]
    fn page_size_cap_is_one_thousand() {
        assert_eq!(MAX_PAGE_SIZE, 1000);
    }

    // The classification helper is the single point at which the SiYuan
    // kernel's free-form error text gets pinned down to typed MCP error
    // kinds. The cases below cover one positive example per family plus a
    // negative passthrough. We assert on `.message` so the test does not
    // depend on internal MCP error code numbers.

    #[test]
    fn classify_auth_prefix() {
        let err = classify_api_msg("Auth failed [abcdef]").expect("auth pattern matches");
        assert!(err.message.contains("authentication failed"));
    }

    #[test]
    fn classify_block_not_found_literal() {
        let err = classify_api_msg("block not found").expect("block-not-found matches");
        assert!(err.message.contains("not found"));
    }

    #[test]
    fn classify_content_block_with_id_lang15() {
        let err = classify_api_msg("Content block with id [20260501-xxxxxxx] not found")
            .expect("Lang(15) matches");
        assert!(err.message.contains("not found"));
    }

    #[test]
    fn classify_notebook_not_found_variants() {
        assert!(classify_api_msg("Query notebook failed").is_some());
        assert!(classify_api_msg("notebook not found").is_some());
        assert!(classify_api_msg("opened notebook [nb-xyz] not found").is_some());
    }

    #[test]
    fn classify_caller_side_field_required() {
        let err = classify_api_msg("Field [stmt] is required").expect("field-required matches");
        assert!(err.message.contains("kernel rejected request"));
    }

    #[test]
    fn classify_caller_side_parses_request() {
        let err = classify_api_msg("Parses request [/api/foo] failed: unexpected end of JSON")
            .expect("parses-request matches");
        assert!(err.message.contains("kernel rejected request"));
    }

    #[test]
    fn classify_invalid_id_argument() {
        let err = classify_api_msg("invalid ID argument").expect("invalid-id matches");
        assert!(err.message.contains("kernel rejected request"));
    }

    #[test]
    fn classify_unknown_message_passes_through() {
        assert!(classify_api_msg("some unrelated kernel hiccup").is_none());
    }
}
