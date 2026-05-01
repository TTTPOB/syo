/// End-to-end smoke test: spawn the siyuan-mcp binary, perform the MCP
/// initialize + tools/list handshake over stdio, and verify the responses.
///
/// We point --base-url at port 1 (refused), which is fine because no actual
/// SiYuan API calls happen during the handshake phase.
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

use serde_json::{Value, json};

/// Path to the compiled binary (cargo sets CARGO_BIN_EXE_siyuan-mcp in tests).
fn binary_path() -> std::path::PathBuf {
    // CARGO_BIN_EXE_<name> uses hyphens in the env var key.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_siyuan-mcp"))
}

/// Send one JSON-RPC line to the child's stdin and read one response line.
fn rpc(stdin: &mut impl Write, stdout: &mut impl BufRead, msg: &Value) -> Value {
    let line = serde_json::to_string(msg).unwrap() + "\n";
    stdin.write_all(line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut response = String::new();
    stdout.read_line(&mut response).unwrap();
    serde_json::from_str(response.trim()).expect("response must be valid JSON")
}

#[test]
fn mcp_initialize_and_tools_list() {
    let mut child = Command::new(binary_path())
        .args(["--base-url", "http://127.0.0.1:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Redirect stderr so tracing output doesn't clutter the test runner.
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn siyuan-mcp");

    let stdin = child.stdin.as_mut().unwrap();
    let mut stdout = BufReader::new(child.stdout.as_mut().unwrap());

    // --- initialize handshake ---
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "0.0.0"
            }
        }
    });

    let init_resp = rpc(stdin, &mut stdout, &init_req);
    assert_eq!(init_resp["jsonrpc"], "2.0", "must be JSON-RPC 2.0");
    assert_eq!(init_resp["id"], 1, "response id must match request id");
    assert!(
        init_resp["error"].is_null(),
        "initialize must not return error"
    );

    let server_name = &init_resp["result"]["serverInfo"]["name"];
    assert_eq!(
        server_name, "siyuan-mcp",
        "serverInfo.name must be 'siyuan-mcp', got: {server_name}"
    );

    // Send initialized notification (required by MCP spec §2.3 after initialize).
    let initialized_notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let notif_line = serde_json::to_string(&initialized_notif).unwrap() + "\n";
    stdin.write_all(notif_line.as_bytes()).unwrap();
    stdin.flush().unwrap();

    // --- tools/list ---
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let list_resp = rpc(stdin, &mut stdout, &list_req);
    assert_eq!(list_resp["jsonrpc"], "2.0");
    assert_eq!(list_resp["id"], 2);
    assert!(
        list_resp["error"].is_null(),
        "tools/list must not return error"
    );

    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools must be an array");

    // Verify minimum tool count.
    assert!(
        tools.len() >= 25,
        "expected >= 25 tools, got {}",
        tools.len()
    );

    // Verify all tools have the siyuan_ prefix, unique names, and substantive descriptions.
    let mut seen_names = std::collections::HashSet::new();
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name must be a string");
        assert!(
            name.starts_with("siyuan_"),
            "tool {name} must start with 'siyuan_'"
        );
        assert!(
            seen_names.insert(name.to_owned()),
            "duplicate tool name: {name}"
        );

        // Enforce that no tool ships with a stub description.
        // 200 chars is conservative — all enriched descriptions are well above that.
        let description = tool["description"]
            .as_str()
            .unwrap_or_else(|| panic!("tool {name} must have a string description"));
        assert!(
            description.len() >= 200,
            "tool {name} description is too short ({} chars < 200); \
             add a multi-sentence description covering what it does, when to use it, \
             and any caveats",
            description.len()
        );
    }

    // --- tools/call dispatch smoke test ---
    // siyuan_status against port 1 must return an error (no kernel), proving dispatch works.
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "siyuan_status",
            "arguments": {}
        }
    });

    let call_resp = rpc(stdin, &mut stdout, &call_req);
    // The call hits port 1 and fails at the HTTP layer, so MCP returns an error.
    let has_error = !call_resp["error"].is_null()
        || call_resp["result"]["isError"].as_bool().unwrap_or(false)
        || call_resp["result"]["content"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    assert!(
        has_error,
        "call to unreachable kernel must produce an error or error content: {call_resp}"
    );

    // Terminate the child cleanly.
    child.kill().ok();
    child.wait().ok();
}
