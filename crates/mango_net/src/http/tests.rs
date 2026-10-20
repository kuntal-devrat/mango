use super::*;
use crate::url::Url;
use std::collections::HashMap;

#[test]
fn test_http_response_helpers() {
    let mut headers = HashMap::new();
    headers.insert(
        "content-type".to_string(),
        "text/html; charset=UTF-8".to_string(),
    );
    headers.insert(
        "location".to_string(),
        "https://example.com/dest".to_string(),
    );

    let resp = HttpResponse {
        final_url: Url::parse("https://example.com").unwrap(),
        status: 301,
        status_text: "Moved Permanently".to_string(),
        headers,
        version: crate::http2::HttpVersion::Http11,
        body: b"Redirecting...".to_vec(),
    };

    assert!(resp.is_redirect());
    assert!(!resp.is_success());
    assert_eq!(resp.content_type(), Some("text/html"));
    assert_eq!(resp.location(), Some("https://example.com/dest"));
    assert_eq!(resp.body_as_string(), "Redirecting...");
}

#[test]
fn test_cookie_jar_storage_and_header_generation() {
    let mut jar = CookieJar::new();
    let url = Url::parse("https://duckduckgo.com/settings").unwrap();

    let cookie1 =
        CookieJar::parse_set_cookie("p=1; Path=/; Domain=duckduckgo.com; Secure", &url).unwrap();
    let cookie2 =
        CookieJar::parse_set_cookie("theme=dark; Path=/settings; HttpOnly", &url).unwrap();

    jar.store_cookie(cookie1);
    jar.store_cookie(cookie2);

    let hdr = jar.get_cookie_header(&url).unwrap();
    assert!(hdr.contains("p=1"));
    assert!(hdr.contains("theme=dark"));

    // Path mismatch for /other
    let other_url = Url::parse("https://duckduckgo.com/other").unwrap();
    let other_hdr = jar.get_cookie_header(&other_url).unwrap();
    assert_eq!(other_hdr, "p=1");

    // Subdomain matching
    let sub_url = Url::parse("https://lite.duckduckgo.com/").unwrap();
    let sub_hdr = jar.get_cookie_header(&sub_url).unwrap();
    assert_eq!(sub_hdr, "p=1");

    // Deletion via Max-Age=0
    let delete_cookie =
        CookieJar::parse_set_cookie("p=deleted; Max-Age=0; Domain=duckduckgo.com; Path=/", &url)
            .unwrap();
    jar.store_cookie(delete_cookie);

    let after_delete = jar.get_cookie_header(&url).unwrap();
    assert_eq!(after_delete, "theme=dark");
}

#[test]
fn test_read_body_streaming_chunking() {
    let fake_data = vec![42u8; 70000];
    let body = ureq::Body::builder().data(fake_data.clone());
    let mut chunks_received = 0;
    let mut total_bytes = 0;
    let accumulated = read_body_streaming(body, |chunk| {
        chunks_received += 1;
        total_bytes += chunk.len();
        true
    })
    .unwrap();
    assert_eq!(accumulated, fake_data);
    assert_eq!(total_bytes, 70000);
    assert!(chunks_received >= 2);
}

#[test]
fn test_multi_value_header_rfc9110_concatenation() {
    let mut map = HashMap::new();
    insert_response_header(&mut map, "Link", "<https://example.com/a>; rel=\"preload\"");
    insert_response_header(
        &mut map,
        "Link",
        "<https://example.com/b>; rel=\"prefetch\"",
    );
    insert_response_header(&mut map, "Vary", "Accept-Encoding");
    insert_response_header(&mut map, "vary", "User-Agent");

    assert_eq!(
        map.get("link").unwrap(),
        "<https://example.com/a>; rel=\"preload\", <https://example.com/b>; rel=\"prefetch\""
    );
    assert_eq!(map.get("vary").unwrap(), "Accept-Encoding, User-Agent");
}

#[test]
fn test_options_request_method() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("port").port();

    let server_thread = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_text = String::from_utf8_lossy(&buf[..n]);
            let is_options = req_text.starts_with("OPTIONS ");
            let resp = if is_options {
                "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\n\r\n"
            } else {
                "HTTP/1.1 405 Method Not Allowed\r\n\r\n"
            };
            let _ = stream.write_all(resp.as_bytes());
            is_options
        } else {
            false
        }
    });

    let client = HttpClient::new();
    let target_url = Url::parse(&format!("http://127.0.0.1:{}/preflight", port)).unwrap();
    let resp = client
        .request("OPTIONS", &target_url, &[], None)
        .expect("options request");
    let server_saw_options = server_thread.join().unwrap();
    assert!(
        server_saw_options,
        "Server must receive OPTIONS request method"
    );
    assert_eq!(resp.status, 204);
    assert_eq!(
        resp.headers.get("access-control-allow-origin").unwrap(),
        "*"
    );
}
