use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
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
