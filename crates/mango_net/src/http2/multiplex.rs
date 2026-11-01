//! HTTP/2 connection multiplexing, concurrent stream dispatch, and priority scheduling.

use super::session::{Http2ConnectionPool, Http2Session, StreamPriority, SubresourcePriority};
use crate::http::HttpResponse;
use crate::http2::HttpVersion;
use crate::url::Url;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A request to be multiplexed over an HTTP/2 session.
#[derive(Debug, Clone)]
pub struct MultiplexedRequest {
    pub url: Url,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub priority: StreamPriority,
    pub body: Option<Vec<u8>>,
}

impl MultiplexedRequest {
    /// Creates a new GET request for subresource fetching with classified priority.
    pub fn get(url: Url, resource_type: SubresourcePriority) -> Self {
        Self {
            url,
            method: "GET".to_string(),
            headers: Vec::new(),
            priority: StreamPriority::for_resource_type(resource_type),
            body: None,
        }
    }
}

/// Dispatches a single HTTP/2 request on a session with priority, returning the assigned stream ID.
pub fn dispatch_request_on_session(
    session: &mut Http2Session,
    request: &MultiplexedRequest,
) -> u32 {
    let path = request.url.path_and_query();
    let authority = request.url.host.clone().unwrap_or_default();
    let scheme = request.url.scheme.clone();

    let mut hpack_headers: Vec<(&str, &str)> = Vec::with_capacity(4 + request.headers.len());
    hpack_headers.push((":method", request.method.as_str()));
    hpack_headers.push((":path", path.as_str()));
    hpack_headers.push((":scheme", scheme.as_str()));
    hpack_headers.push((":authority", authority.as_str()));

    for (k, v) in &request.headers {
        hpack_headers.push((k.as_str(), v.as_str()));
    }

    let end_stream = request.body.is_none();
    let stream_id =
        session.send_request_with_priority(&hpack_headers, end_stream, request.priority);

    if let Some(ref body_bytes) = request.body {
        session.send_data(stream_id, body_bytes.clone(), true);
    }

    stream_id
}

/// Dispatches a batch of requests multiplexed across origin sessions in priority order.
pub fn dispatch_multiplexed_batch(
    pool: &Arc<Mutex<Http2ConnectionPool>>,
    requests: &[MultiplexedRequest],
) -> HashMap<u32, (Url, StreamPriority)> {
    let mut stream_map = HashMap::new();
    let mut origin_groups: HashMap<String, Vec<&MultiplexedRequest>> = HashMap::new();

    for req in requests {
        let origin = req.url.origin();
        origin_groups.entry(origin).or_default().push(req);
    }

    let mut pool_guard = pool.lock().unwrap();
    for (origin, req_list) in origin_groups {
        let session_arc = pool_guard.get_or_create(&origin);
        let mut session = session_arc.lock().unwrap();

        for req in req_list {
            let stream_id = dispatch_request_on_session(&mut session, req);
            stream_map.insert(stream_id, (req.url.clone(), req.priority));
        }

        // Schedule frames according to RFC 7540 weights
        let _scheduled = session.schedule_outbound_frames();
    }

    stream_map
}

/// Synthesizes an HTTP response from a completed HTTP/2 stream state.
pub fn stream_to_http_response(
    url: &Url,
    response_headers: &HashMap<String, String>,
    body: Vec<u8>,
) -> HttpResponse {
    let status_str = response_headers
        .get(":status")
        .cloned()
        .unwrap_or_else(|| "200".to_string());
    let status = status_str.parse::<u16>().unwrap_or(200);

    let mut headers = HashMap::new();
    for (k, v) in response_headers {
        if !k.starts_with(':') {
            headers.insert(k.to_ascii_lowercase(), v.clone());
        }
    }

    let status_text = match status {
        200 => "OK",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "",
    }
    .to_string();

    HttpResponse {
        final_url: url.clone(),
        status,
        status_text,
        headers,
        version: HttpVersion::Http2,
        body,
    }
}
