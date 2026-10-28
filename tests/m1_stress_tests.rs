//! Milestone 1 Stress and Boundary Tests.
//!
//! Empirical tests verifying:
//! 1. Headless runner timeout behavior and deadlock avoidance under short/zero timeouts.
//! 2. WptServer concurrency under 80 parallel requests and clean teardown on Drop.
//! 3. BrowserChrome::eval_js exception safety under syntax errors, thrown objects/primitives,
//!    stack recursion, and post-exception runtime stability.

#![allow(clippy::duplicate_mod)]

#[path = "wpt_acid3_runner.rs"]
mod runner;
#[path = "wpt/server.rs"]
mod server;

use mango_browser::browser::BrowserChrome;
use server::WptServer;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_runner_graceful_short_timeout() {
    let server = WptServer::new("tests/wpt").expect("Failed to start server");
    let start = Instant::now();
    let short_timeout = Duration::from_millis(500);

    let res = runner::run_acid3_headless(&server.base_url(), short_timeout)
        .expect("Runner must return Ok result even on timeout");

    let elapsed = start.elapsed();
    assert!(!res.completed, "Must be incomplete on short timeout");
    assert!(
        elapsed >= short_timeout,
        "Elapsed time ({:?}) should be at least timeout ({:?})",
        elapsed,
        short_timeout
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "Runner must not hang on timeout; took {:?}",
        elapsed
    );
}

#[test]
fn test_runner_zero_timeout_boundary() {
    let server = WptServer::new("tests/wpt").expect("Failed to start server");
    let start = Instant::now();
    let zero_timeout = Duration::ZERO;

    let res = runner::run_acid3_headless(&server.base_url(), zero_timeout)
        .expect("Runner must safely handle zero timeout");

    let elapsed = start.elapsed();
    assert!(!res.completed);
    assert!(
        elapsed < Duration::from_secs(3),
        "Zero timeout should return immediately; took {:?}",
        elapsed
    );
}

#[test]
fn test_wpt_server_rapid_concurrent_requests() {
    let server = Arc::new(WptServer::new("tests/wpt").expect("Failed to start server"));
    let mut handles = Vec::new();
    let endpoints = [
        "/acid/acid3/test.html",
        "/acid3/empty.css",
        "/acid3/empty.html",
        "/acid/acid3/support-a.png?pipe=status(404)",
        "/acid/acid3/support-b.png?pipe=header(X-Stress,Active)|status(200)",
        "/acid3/empty.xml",
        "/acid3/xhtml.1",
        "/nonexistent_file_404.txt",
    ];

    for i in 0..80 {
        let s = Arc::clone(&server);
        let endpoint = endpoints[i % endpoints.len()];
        let h = thread::spawn(move || {
            let port = s.port();
            let mut stream =
                TcpStream::connect(("127.0.0.1", port)).expect("Failed to connect to WptServer");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let req = format!(
                "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                endpoint, port
            );
            stream.write_all(req.as_bytes()).unwrap();

            let mut resp = Vec::new();
            stream.read_to_end(&mut resp).unwrap();
            let resp_str = String::from_utf8_lossy(&resp);
            assert!(
                resp_str.starts_with("HTTP/1.1 "),
                "Must receive HTTP response header, got: {}",
                resp_str
            );
            if endpoint.contains("status(404)") || endpoint.contains("nonexistent") {
                assert!(
                    resp_str.contains("404 Not Found"),
                    "Expected 404 for {}, got {}",
                    endpoint,
                    resp_str
                );
            } else {
                assert!(
                    resp_str.contains("200 OK"),
                    "Expected 200 for {}, got {}",
                    endpoint,
                    resp_str
                );
            }
        });
        handles.push(h);
    }

    for h in handles {
        h.join().expect("Concurrent request thread panicked");
    }
}

#[test]
fn test_wpt_server_clean_teardown_and_rapid_drop() {
    // 1: Rapid drop without requests
    for _ in 0..10 {
        let server = WptServer::new("tests/wpt").expect("Failed to create server");
        let start = Instant::now();
        drop(server);
        assert!(
            start.elapsed() < Duration::from_millis(200),
            "Server drop took too long: {:?}",
            start.elapsed()
        );
    }

    // 2: Drop while connection is held open
    {
        let server = WptServer::new("tests/wpt").expect("Failed to create server");
        let port = server.port();
        let _stream = TcpStream::connect(("127.0.0.1", port)).expect("Failed to connect");
        let start = Instant::now();
        drop(server);
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "Server drop with active connection took too long: {:?}",
            start.elapsed()
        );
    }
}

#[test]
fn test_browser_eval_js_exceptions_and_panics() {
    let mut browser = BrowserChrome::new(800, 600);

    // 1. Basic evaluation
    let res = browser.eval_js("1 + 1");
    assert_eq!(res.as_deref(), Ok("2"));

    // 2. Syntax Error
    let res_syntax = browser.eval_js("function ( {");
    assert!(res_syntax.is_err(), "Syntax error must return Err");
    let err_str = res_syntax.unwrap_err();
    assert!(
        err_str.contains("SyntaxError") || err_str.contains("JS Error"),
        "Unexpected syntax error msg: {err_str}"
    );

    // 3. Runtime Error (throw Error)
    let res_throw = browser.eval_js("throw new Error('deliberate error')");
    assert!(res_throw.is_err(), "Thrown error must return Err");
    assert!(res_throw.unwrap_err().contains("deliberate error"));

    // 4. Throwing primitives
    let res_num = browser.eval_js("throw 42");
    assert!(res_num.is_err());
    assert!(res_num.unwrap_err().contains("42"));

    let res_null = browser.eval_js("throw null");
    assert!(res_null.is_err());
    assert!(res_null.unwrap_err().contains("null"));

    let res_undef = browser.eval_js("throw undefined");
    assert!(res_undef.is_err());
    assert!(res_undef.unwrap_err().contains("undefined"));

    // 5. Throwing in toString of returned value
    let res_tostr = browser.eval_js("({ toString() { throw new Error('poisoned toString'); } })");
    // Verify it doesn't panic
    println!("toString throwing result: {:?}", res_tostr);

    // 6. Deep recursion / stack overflow
    let res_rec = browser.eval_js("(function rec() { return rec(); })()");
    assert!(res_rec.is_err(), "Stack overflow must return Err");
    let rec_err = res_rec.unwrap_err();
    assert!(
        rec_err.contains("Maximum call stack size exceeded") || rec_err.contains("JS Error"),
        "Unexpected stack overflow msg: {rec_err}"
    );

    // 7. Post-error evaluation safety (engine must remain functional)
    let res_post = browser.eval_js("10 * 4 + 2");
    assert_eq!(
        res_post.as_deref(),
        Ok("42"),
        "Engine must recover after exceptions"
    );
}

#[test]
fn test_wpt_server_malformed_requests() {
    let server = WptServer::new("tests/wpt").expect("Failed to start server");
    let port = server.port();

    // 1. Completely empty stream / immediate disconnect
    {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        drop(stream);
    }

    // 2. Just whitespace / newlines
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let _ = stream.write_all(b"\r\n\r\n");
    }

    // 3. Partial / single-word request
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let _ = stream.write_all(b"GARBAGE_ONLY\r\n\r\n");
    }

    // 4. Incomplete request line without newline (relies on read_timeout or handle termination)
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let _ = stream.write_all(b"GET /acid/acid3/");
    }

    // 5. Oversized payload (8KB of headers)
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let big = "X-Big: ".to_string() + &"a".repeat(8192) + "\r\n";
        let req = format!(
            "GET /acid/acid3/test.html HTTP/1.1\r\n{}Connection: close\r\n\r\n",
            big
        );
        let _ = stream.write_all(req.as_bytes());
    }

    // Verify server remains responsive after all malformed requests
    {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req = format!(
            "GET /acid/acid3/test.html HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            port
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = Vec::new();
        stream.read_to_end(&mut resp).unwrap();
        assert!(String::from_utf8_lossy(&resp).contains("200 OK"));
    }
}

#[test]
fn test_browser_eval_js_unicode_and_large_strings() {
    let mut browser = BrowserChrome::new(800, 600);

    // Astral plane unicode & null bytes
    let res = browser.eval_js("'\u{1F4A9} \u{0000} \u{200B}'");
    assert!(res.is_ok());
    let s = res.unwrap();
    assert!(s.contains("💩"));

    // Large string generation
    let res_large = browser.eval_js("'A'.repeat(50000)");
    assert!(res_large.is_ok());
    assert_eq!(res_large.unwrap().len(), 50000);
}

#[test]
fn test_runner_unreachable_server_timeout() {
    // Port 1 is reserved and typically closed/unreachable
    let start = Instant::now();
    let res = runner::run_acid3_headless("http://127.0.0.1:1", Duration::from_millis(500));
    let elapsed = start.elapsed();
    println!("Unreachable host result: {:?}, elapsed: {:?}", res, elapsed);
    // The runner should not panic or hang forever
    assert!(elapsed < Duration::from_secs(5));
}
