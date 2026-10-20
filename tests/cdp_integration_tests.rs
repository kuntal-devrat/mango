//! Integration tests for the embedded Chrome DevTools Protocol (CDP) server.
//!
//! Validates standard HTTP discovery endpoints and RFC 6455 WebSocket JSON-RPC 2.0
//! control across Target, Page, DOM, Runtime, Network, and Screenshot capture domains.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use mango_browser::cdp::CdpServer;
use serde_json::{Value, json};

#[test]
fn test_cdp_http_discovery_endpoints() {
    let server = CdpServer::start(0).expect("failed to start CDP server");
    let port = server.port();
    assert!(port > 0);

    // 1. Test /json/version
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .write_all(b"GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(
        status_line.contains("200 OK"),
        "Expected 200 OK, got: {status_line}"
    );

    let mut body = String::new();
    let mut reading_body = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if reading_body {
            body.push_str(&line);
        } else if line.is_empty() {
            reading_body = true;
        }
    }

    let ver: Value = serde_json::from_str(&body).expect("parse version JSON");
    assert!(ver.get("Browser").is_some());
    assert_eq!(ver["Protocol-Version"], "1.3");
    assert!(
        ver["webSocketDebuggerUrl"]
            .as_str()
            .unwrap()
            .starts_with("ws://127.0.0.1:")
    );

    // 2. Test /json/list
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .write_all(b"GET /json/list HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"));

    let mut body = String::new();
    let mut reading_body = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if reading_body {
            body.push_str(&line);
        } else if line.is_empty() {
            reading_body = true;
        }
    }

    let targets: Vec<Value> = serde_json::from_str(&body).expect("parse targets JSON");
    assert!(!targets.is_empty());
    assert_eq!(targets[0]["id"], "page-1");

    // 3. Test /json/new
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .write_all(b"GET /json/new?url=https://example.com HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"));

    let mut body = String::new();
    let mut reading_body = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if reading_body {
            body.push_str(&line);
        } else if line.is_empty() {
            reading_body = true;
        }
    }

    let new_target: Value = serde_json::from_str(&body).expect("parse new target JSON");
    assert_eq!(new_target["url"], "https://example.com");
    let new_id = new_target["id"].as_str().unwrap();

    // 4. Test /json/activate/{id}
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let act_req = format!(
        "GET /json/activate/{new_id} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(act_req.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"));

    // 5. Test /json/close/{id}
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let close_req = format!(
        "GET /json/close/{new_id} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(close_req.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).unwrap();
    assert!(status_line.contains("200 OK"));
}

#[test]
fn test_cdp_websocket_json_rpc_session() {
    let server = CdpServer::start(0).expect("failed to start CDP server");
    let port = server.port();

    // Connect and upgrade to WebSocket
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let handshake = format!(
        "GET /devtools/page/page-1 HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(handshake.as_bytes()).unwrap();

    // Read handshake response
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).unwrap();
    let resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));

    // Helper closure to send a JSON-RPC request and decode response frame
    let mut rpc_call = |req: Value| -> Value {
        let req_bytes = serde_json::to_vec(&req).unwrap();
        // Client frame must be masked per RFC 6455 §5.3
        let mask = [0x12, 0x34, 0x56, 0x78];
        let mut frame = Vec::new();
        frame.push(0x81); // FIN + Text opcode
        let len = req_bytes.len();
        if len <= 125 {
            frame.push(0x80 | (len as u8));
        } else if len <= 65535 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        for (i, b) in req_bytes.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        stream.write_all(&frame).unwrap();

        // Read server frame (unmasked)
        let (opcode, payload, _) =
            mango_net::websocket::decode_frame(&mut stream).expect("decode server frame");
        assert_eq!(opcode, mango_net::websocket::Opcode::Text);
        serde_json::from_slice(&payload).expect("parse JSON-RPC response")
    };

    // 1. Browser.getVersion
    let resp = rpc_call(json!({
        "id": 1,
        "method": "Browser.getVersion"
    }));
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["protocolVersion"], "1.3");
    assert!(
        resp["result"]["product"]
            .as_str()
            .unwrap()
            .contains("Mango")
    );

    // 2. Page.navigate
    let resp = rpc_call(json!({
        "id": 2,
        "method": "Page.navigate",
        "params": {
            "url": "data:text/html,<html><head><title>CDP Test Page</title></head><body><h1 id=\"headline\">Mango Title</h1><p class=\"desc\">Test Paragraph</p></body></html>"
        }
    }));
    assert_eq!(resp["id"], 2);
    assert!(resp["result"]["frameId"].is_string());

    // 3. DOM.getDocument
    let resp = rpc_call(json!({
        "id": 3,
        "method": "DOM.getDocument",
        "params": { "depth": -1 }
    }));
    assert_eq!(resp["id"], 3);
    assert!(resp["result"]["root"].is_object());

    // 4. DOM.querySelector for #headline
    let resp = rpc_call(json!({
        "id": 4,
        "method": "DOM.querySelector",
        "params": {
            "nodeId": 1,
            "selector": "#headline"
        }
    }));
    assert_eq!(resp["id"], 4);
    let headline_id = resp["result"]["nodeId"].as_u64().expect("headline nodeId");
    assert!(headline_id > 0);

    // 5. DOM.getOuterHTML for #headline
    let resp = rpc_call(json!({
        "id": 5,
        "method": "DOM.getOuterHTML",
        "params": { "nodeId": headline_id }
    }));
    assert_eq!(resp["id"], 5);
    assert!(
        resp["result"]["outerHTML"]
            .as_str()
            .unwrap()
            .contains("Mango Title")
    );

    // 6. Runtime.evaluate JavaScript
    let resp = rpc_call(json!({
        "id": 6,
        "method": "Runtime.evaluate",
        "params": {
            "expression": "10 + 20 * 2"
        }
    }));
    assert_eq!(resp["id"], 6);
    assert_eq!(resp["result"]["result"]["type"], "number");
    assert_eq!(resp["result"]["result"]["value"], 50.0);

    // 7. Page.captureScreenshot
    let resp = rpc_call(json!({
        "id": 7,
        "method": "Page.captureScreenshot",
        "params": { "format": "png" }
    }));
    assert_eq!(resp["id"], 7);
    let base64_data = resp["result"]["data"]
        .as_str()
        .expect("screenshot base64 string");
    assert!(!base64_data.is_empty());
    // Decode base64 to ensure it's a valid PNG header
    let decoded = mango_render::image_decode::base64_decode(base64_data).expect("base64 decode");
    assert!(
        decoded.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Valid PNG signature"
    );
}
