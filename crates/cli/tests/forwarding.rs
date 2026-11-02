use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::thread;

#[test]
fn look_forwards_to_authenticated_match_sight() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut bytes = [0; 4096];
        let count = socket.read(&mut bytes).unwrap();
        let request = String::from_utf8_lossy(&bytes[..count]);
        assert!(request.starts_with("POST /v1/matches/match-1/look HTTP/1.1\r\n"));
        assert!(request.to_ascii_lowercase().contains("authorization: bearer test-secret"));
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_tankwar"))
        .env("TANKWAR_URL", format!("http://{address}"))
        .env("TANKWAR_ACCESS_TOKEN", "test-secret")
        .env_remove("TANKWAR_ACCESS_CLIENT_ID").env_remove("TANKWAR_ACCESS_CLIENT_SECRET")
        .args(["look", "match-1"]).output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn cli_forwards_command_with_bearer_and_reports_acceptance() {
    verify_forwarding(false);
}

#[test]
fn cli_forwards_cloudflare_access_service_credentials() {
    verify_forwarding(true);
}

#[test]
fn direct_crew_commands_forward_typed_orders() {
    let cases: Vec<(Vec<&str>, serde_json::Value)> = vec![
        (vec!["fire", "match-1", "seat-1", "cmd-fire"], serde_json::json!({"kind":"fire"})),
        (vec!["engine", "match-1", "seat-1", "cmd-engine", "true"], serde_json::json!({"kind":"engine", "running":true})),
        (vec!["drive", "match-1", "seat-1", "cmd-drive", "500", "100", "20"], serde_json::json!({"kind":"drive", "throttle":500, "steering":100, "ttl_ticks":20})),
        (vec!["drive", "match-1", "seat-1", "cmd-reverse", "--", "-500", "-100", "20"], serde_json::json!({"kind":"drive", "throttle":-500, "steering":-100, "ttl_ticks":20})),
        (vec!["load", "match-1", "seat-1", "cmd-load", "ap"], serde_json::json!({"kind":"load", "ammunition":"ap"})),
    ];
    for (args, expected) in cases {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_tankwar"))
            .env("TANKWAR_URL", format!("http://{address}"))
            .env("TANKWAR_ACCESS_TOKEN", "test-secret")
            .env_remove("TANKWAR_ACCESS_CLIENT_ID").env_remove("TANKWAR_ACCESS_CLIENT_SECRET")
            .args(&args).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut socket, _) = loop {
            match listener.accept() {
                Ok(socket) => break socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => {
                    if let Some(status) = child.try_wait().unwrap() { panic!("CLI exited before forwarding {:?}: {status}", args); }
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("CLI did not connect for {:?}: {e}", args),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut bytes=Vec::new();
        let header_end = loop { let mut b=[0;1024]; let n=socket.read(&mut b).unwrap(); assert!(n>0); bytes.extend_from_slice(&b[..n]); if let Some(i)=bytes.windows(4).position(|w|w==b"\r\n\r\n") { break i+4; } };
        let headers=String::from_utf8_lossy(&bytes[..header_end]);
        let length: usize=headers.lines().find_map(|line| { let (k,v)=line.split_once(':')?; k.eq_ignore_ascii_case("content-length").then(||v.trim().parse().unwrap()) }).unwrap();
        while bytes.len()<header_end+length { let mut b=[0;1024]; let n=socket.read(&mut b).unwrap(); assert!(n>0); bytes.extend_from_slice(&b[..n]); }
        let body: serde_json::Value=serde_json::from_slice(&bytes[header_end..header_end+length]).unwrap();
        assert_eq!(body["id"], args[3]);
        assert_eq!(body["order"], expected);
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").unwrap();
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn mcp_drive_forwards_signed_values_and_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let (mut socket, _) = loop {
            match listener.accept() {
                Ok(socket) => break socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && std::time::Instant::now() < deadline => thread::sleep(std::time::Duration::from_millis(10)),
                Err(e) => panic!("MCP did not forward: {e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut bytes = Vec::new();
        let end = loop {
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).unwrap(); assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") { break i + 4; }
        };
        let headers = String::from_utf8_lossy(&bytes[..end]);
        assert!(headers.starts_with("POST /v1/matches/match-1/commands HTTP/1.1\r\n"));
        assert!(headers.lines().any(|l| l.eq_ignore_ascii_case("cf-access-client-id: test-client.access")));
        assert!(headers.lines().any(|l| l.eq_ignore_ascii_case("cf-access-client-secret: test-secret")));
        let length: usize = headers.lines().find_map(|line| { let (k,v) = line.split_once(':')?; k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().unwrap()) }).unwrap();
        while bytes.len() < end + length { let mut buf=[0;1024]; let n=socket.read(&mut buf).unwrap(); assert!(n>0); bytes.extend_from_slice(&buf[..n]); }
        let body: serde_json::Value = serde_json::from_slice(&bytes[end..end+length]).unwrap();
        assert_eq!(body["id"], "mcp-1");
        assert_eq!(body["order"], serde_json::json!({"kind":"drive","throttle":-500,"steering":-100,"ttl_ticks":20}));
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").unwrap();
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_tankwar"))
        .env("TANKWAR_URL", format!("http://{address}"))
        .env("TANKWAR_ACCESS_CLIENT_ID", "test-client.access")
        .env("TANKWAR_ACCESS_CLIENT_SECRET", "test-secret")
        .env_remove("TANKWAR_ACCESS_TOKEN")
        .arg("--mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    for message in [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"call_write_tool","arguments":{"name":"drive","arguments":{"match_id":"match-1","seat_id":"seat-1","command_id":"mcp-1","throttle":-500,"steering":-100,"ttl_ticks":20}}}}),
    ] { writeln!(input, "{message}").unwrap(); }
    drop(input);
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let replies: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout).lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    let reply = replies.iter().find(|value| value["id"] == 2).expect("MCP response");
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("test-secret"));
}

fn verify_forwarding(access: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut bytes = Vec::new();
        let header_end;
        loop {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") { header_end = end + 4; break; }
        }
        let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
        let length: usize = headers.lines().find_map(|line| {
            let (name,value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
        }).unwrap();
        while bytes.len() < header_end + length {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
        }
        assert!(headers.starts_with("POST /v1/matches/match-1/commands HTTP/1.1\r\n"));
        if access {
            assert!(headers.lines().any(|l| l.eq_ignore_ascii_case("cf-access-client-id: test-client.access")));
            assert!(headers.lines().any(|l| l.eq_ignore_ascii_case("cf-access-client-secret: test-secret")));
            assert!(!headers.lines().any(|l| l.to_ascii_lowercase().starts_with("authorization:")));
        } else {
            assert!(headers.lines().any(|l| l.eq_ignore_ascii_case("authorization: Bearer test-secret")));
        }
        let body: serde_json::Value = serde_json::from_slice(&bytes[header_end..header_end+length]).unwrap();
        assert_eq!(body["id"], "cmd-1");
        assert_eq!(body["order"]["kind"], "fire");
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").unwrap();
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_tankwar"));
    command.env("TANKWAR_URL", format!("http://{address}"))
        .env_remove("TANKWAR_ACCESS_TOKEN").env_remove("TANKWAR_ACCESS_CLIENT_ID").env_remove("TANKWAR_ACCESS_CLIENT_SECRET");
    if access { command.env("TANKWAR_ACCESS_CLIENT_ID", "test-client.access").env("TANKWAR_ACCESS_CLIENT_SECRET", "test-secret"); }
    else { command.env("TANKWAR_ACCESS_TOKEN", "test-secret"); }
    let output = command.args(["order", r#"{"protocol_version":1,"id":"cmd-1","match_id":"match-1","seat_id":"seat-1","expected_version":null,"order":{"kind":"fire"}}"#]).output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("test-secret"));
}
