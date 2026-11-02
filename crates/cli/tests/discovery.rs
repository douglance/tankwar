use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn drive_schema_describes_controls_and_idempotency() {
    let output = Command::new(env!("CARGO_BIN_EXE_tankwar"))
        .args(["drive", "--schema", "--format", "json"]).output().unwrap();
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let props = &schema["args"]["properties"];
    assert!(props["throttle"]["description"].as_str().unwrap().contains("-1000 to 1000"));
    assert!(props["ttl-ticks"]["description"].as_str().unwrap().contains("1 to 200"));
    assert!(props["command-id"]["description"].as_str().unwrap().contains("identical"));
}

#[test]
fn mcp_discovery_exposes_drive_schema_and_read_only_observation() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tankwar"))
        .arg("--mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for message in [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_tool_details","arguments":{"name":"drive"}}}),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_tool_details","arguments":{"name":"look"}}}),
    ] { writeln!(input, "{message}").unwrap(); }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let replies: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    let drive = &replies.iter().find(|reply| reply["id"] == 2).unwrap()["result"]["structuredContent"];
    assert!(drive["inputSchema"]["properties"]["ttl_ticks"]["description"].as_str().unwrap().contains("1 to 200"));
    assert_ne!(drive["annotations"]["readOnlyHint"], true);
    let look = &replies.iter().find(|reply| reply["id"] == 3).unwrap()["result"]["structuredContent"];
    assert_eq!(look["annotations"]["readOnlyHint"], true);
}
