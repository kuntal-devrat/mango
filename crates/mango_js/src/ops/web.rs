use deno_core::{op2, OpState};
use mango_net::http::HttpClient;
use mango_net::url::Url;

use crate::dom_bindings::CookieStore;
use crate::web_apis::{
    SharedEventLoop, SharedLocalStorage, SharedPendingNav, SharedSessionStorage, SharedStatusText,
};

// ── Timers ───────────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_schedule_timer(
    state: &mut OpState,
    id: u32,
    delay_ms: u32,
    repeats: bool,
    _is_raf: bool,
) {
    if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        let source = format!(
            "if (typeof _mangoRunTimer === 'function') _mangoRunTimer({});",
            id
        );
        if let Ok(mut el) = event_loop.try_borrow_mut() {
            if repeats {
                el.schedule_interval_with_id(id, source, delay_ms as u64);
            } else {
                el.schedule_timeout_with_id(id, source, delay_ms as u64);
            }
        }
    }
}

#[op2(fast)]
pub fn op_mango_cancel_timer(state: &mut OpState, id: u32) {
    if let Some(event_loop) = state.try_borrow::<SharedEventLoop>()
        && let Ok(mut el) = event_loop.try_borrow_mut()
    {
        el.cancel_timer(id);
    }
}

// ── Alert & Navigation ───────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_alert(state: &mut OpState, #[string] msg: String) {
    log::info!("[JS alert] {}", msg);
    if let Some(status_text) = state.try_borrow::<SharedStatusText>() {
        *status_text.borrow_mut() = format!("Alert: {}", msg);
    }
}

#[op2(fast)]
pub fn op_mango_navigate(state: &mut OpState, #[string] url: String) {
    if !url.is_empty() {
        log::info!("[JS navigation] Script requested navigate to: {}", url);
        if let Some(pending_nav) = state.try_borrow::<SharedPendingNav>() {
            *pending_nav.borrow_mut() = Some(url);
        }
    }
}

// ── Crypto ───────────────────────────────────────────────────────────────────

#[op2]
#[serde]
pub fn op_mango_random_bytes(len: u32) -> Vec<u8> {
    let mut buf = vec![0u8; len.min(65536) as usize];
    crate::web_apis::fill_secure_random(&mut buf);
    buf
}

#[derive(Clone)]
pub struct OpLocalStorage(pub SharedLocalStorage);

#[derive(Clone)]
pub struct OpSessionStorage(pub SharedSessionStorage);

fn get_page_origin(state: &OpState) -> String {
    let page_url = state
        .try_borrow::<crate::ops::dom::PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_default();
    match mango_net::Url::parse(&page_url) {
        Ok(u) if u.host.is_some() => {
            let port_part = match (u.scheme.as_str(), u.port) {
                ("http", Some(80)) | ("https", Some(443)) => String::new(),
                (_, Some(p)) => format!(":{p}"),
                (_, None) => String::new(),
            };
            format!(
                "{}://{}{}",
                u.scheme.to_ascii_lowercase(),
                u.host.as_deref().unwrap_or("").to_ascii_lowercase(),
                port_part
            )
        }
        _ => page_url,
    }
}

// ── LocalStorage ─────────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_ls_get(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    let store = state.try_borrow::<OpLocalStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&scoped_key).cloned()
}

#[op2(fast)]
pub fn op_mango_ls_set(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(scoped_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_ls_remove(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&scoped_key);
    }
}

#[op2(fast)]
pub fn op_mango_ls_clear(state: &mut OpState) {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.retain(|k, _| !k.starts_with(&prefix));
    }
}

#[op2]
#[serde]
pub fn op_mango_ls_keys(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].starts_with("__idb__"))
            .map(|k| k[prefix.len()..].to_string())
            .collect()
    } else {
        Vec::new()
    }
}

// ── SessionStorage ───────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_ss_get(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    let store = state.try_borrow::<OpSessionStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&scoped_key).cloned()
}

#[op2(fast)]
pub fn op_mango_ss_set(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(scoped_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_ss_remove(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let scoped_key = format!("{orig}\x1f{key}");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&scoped_key);
    }
}

#[op2(fast)]
pub fn op_mango_ss_clear(state: &mut OpState) {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.retain(|k, _| !k.starts_with(&prefix));
    }
}

#[op2]
#[serde]
pub fn op_mango_ss_keys(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f");
    if let Some(store) = state.try_borrow::<OpSessionStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect()
    } else {
        Vec::new()
    }
}

// ── IndexedDB Stubs ──────────────────────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_idb_load(state: &mut OpState, #[string] key: String) -> Option<String> {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    let store = state.try_borrow::<OpLocalStorage>()?;
    let map = store.0.lock().ok()?;
    map.get(&idb_key).cloned()
}

#[op2(fast)]
pub fn op_mango_idb_save(state: &mut OpState, #[string] key: String, #[string] val: String) {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.insert(idb_key, val);
    }
}

#[op2(fast)]
pub fn op_mango_idb_delete(state: &mut OpState, #[string] key: String) {
    let orig = get_page_origin(state);
    let idb_key = format!("{orig}\x1f__idb__{key}");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(mut map) = store.0.lock()
    {
        map.remove(&idb_key);
    }
}

#[op2]
#[serde]
pub fn op_mango_idb_list(state: &mut OpState) -> Vec<String> {
    let orig = get_page_origin(state);
    let prefix = format!("{orig}\x1f__idb__");
    if let Some(store) = state.try_borrow::<OpLocalStorage>()
        && let Ok(map) = store.0.lock()
    {
        map.keys()
            .filter_map(|k| k.strip_prefix(&prefix).map(|s| s.to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

// ── Web Workers ──────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_worker_create(
    state: &mut OpState,
    id: u32,
    #[string] script_url: String,
) {
    let page_url = state
        .try_borrow::<crate::ops::dom::PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_default();
    let tasks = if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        event_loop.borrow().task_queue()
    } else {
        return;
    };
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.create_worker(id as u64, &script_url, &page_url, tasks);
    }
}

#[op2(fast)]
pub fn op_mango_worker_post_message(state: &mut OpState, worker_id: u32, #[string] message: String) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        let _ = wm.post_message(worker_id as u64, &message);
    }
}

#[op2(fast)]
pub fn op_mango_worker_terminate(state: &mut OpState, worker_id: u32) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        let _ = wm.terminate(worker_id as u64);
    }
}

#[op2(fast)]
pub fn op_mango_blob_register(state: &mut OpState, #[string] blob_id: String, #[string] data: String) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.register_blob(blob_id, data);
    }
}

// ── WebSockets ───────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_ws_connect(
    state: &mut OpState,
    id: u32,
    #[string] url: String,
    #[string] protocols: String,
) {
    let tasks = if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        event_loop.borrow().task_queue()
    } else {
        return;
    };
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let ws = wsm.clone();
        ws.connect(id as u64, &url, &protocols, move |event| {
            let cmd = match event {
                mango_net::WebSocketEvent::Open { protocol } => {
                    let p = protocol.replace('\\', "\\\\").replace('"', "\\\"");
                    format!(
                        "if (typeof _mangoDispatchWsOpen === 'function') _mangoDispatchWsOpen({}, \"{}\");",
                        id, p
                    )
                }
                mango_net::WebSocketEvent::Message { data } => {
                    let escaped = data
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsMessage === 'function') _mangoDispatchWsMessage({}, \"{}\");",
                        id, escaped
                    )
                }
                mango_net::WebSocketEvent::BinaryMessage { data: _ } => {
                    format!(
                        "if (typeof _mangoDispatchWsMessage === 'function') _mangoDispatchWsMessage({}, \"\");",
                        id
                    )
                }
                mango_net::WebSocketEvent::Error { message } => {
                    let escaped = message
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsError === 'function') _mangoDispatchWsError({}, \"{}\");",
                        id, escaped
                    )
                }
                mango_net::WebSocketEvent::Close {
                    code,
                    reason,
                    was_clean,
                } => {
                    let escaped = reason
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsClose === 'function') _mangoDispatchWsClose({}, {}, \"{}\", {});",
                        id, code, escaped, was_clean
                    )
                }
            };
            if let Ok(mut q) = tasks.lock() {
                q.push(cmd);
            }
        });
    }
}

#[op2(fast)]
pub fn op_mango_ws_send(state: &mut OpState, ws_id: u32, #[string] data: String) {
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let _ = wsm.send_text(ws_id as u64, data);
    }
}

#[op2(fast)]
pub fn op_mango_ws_close(state: &mut OpState, ws_id: u32, code: u32, #[string] reason: String) {
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let _ = wsm.close(ws_id as u64, code as u16, reason);
    }
}

// ── Fetch ────────────────────────────────────────────────────────────────────

fn parse_simple_headers_json(json: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let trimmed = json.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return pairs;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    for item in inner.split(',') {
        if let Some((k, v)) = item.split_once(':') {
            let clean_k = k.trim().trim_matches('"').trim_matches('\'').to_string();
            let clean_v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            if !clean_k.is_empty() {
                pairs.push((clean_k, clean_v));
            }
        }
    }
    pairs
}

fn encode_base64_bytes(bytes: &[u8]) -> String {
    crate::web_apis::encode_base64_bytes(bytes)
}

fn format_fetch_response_json(result: Result<mango_net::HttpResponse, String>) -> String {
    match result {
        Ok(resp) => {
            let body_str = resp.body_as_string();
            let body_b64 = encode_base64_bytes(&resp.body);
            let escaped_body = body_str
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            let ct = resp
                .headers
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "text/plain".to_string())
                .replace('"', "\\\"");

            let mut hdrs_entries = Vec::new();
            for (k, v) in &resp.headers {
                let ek = k.replace('\\', "\\\\").replace('"', "\\\"");
                let ev = v.replace('\\', "\\\\").replace('"', "\\\"");
                hdrs_entries.push(format!("\"{}\":\"{}\"", ek, ev));
            }
            let hdrs_json = hdrs_entries.join(",");

            format!(
                "{{\"ok\":{},\"status\":{},\"statusText\":\"{}\",\"contentType\":\"{}\",\"headers\":{{{}}},\"body\":\"{}\",\"bodyBase64\":\"{}\",\"error\":null}}",
                resp.is_success(),
                resp.status,
                resp.status_text.replace('"', "\\\""),
                ct,
                hdrs_json,
                escaped_body,
                body_b64
            )
        }
        Err(e) => {
            let escaped_err = e.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                "{{\"ok\":false,\"status\":0,\"statusText\":\"\",\"contentType\":\"\",\"headers\":{{}},\"body\":\"\",\"bodyBase64\":\"\",\"error\":\"{}\"}}",
                escaped_err
            )
        }
    }
}

#[op2]
#[string]
pub fn op_mango_fetch(
    state: &mut OpState,
    #[string] url_str: String,
    #[string] method_str: String,
    #[string] headers_json: String,
    #[string] body_str: Option<String>,
    #[string] origin_str: String,
    #[string] mode_str: String,
    creds_bool: bool,
) -> String {
    let method_str = method_str.to_uppercase();
    let mode_str = mode_str.to_lowercase();
    let body_str = body_str.unwrap_or_default();

    let cj = state
        .try_borrow::<CookieStore>()
        .cloned()
        .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(mango_net::CookieJar::new())));

    let client = HttpClient::with_cookie_jar(cj);
    let mut custom_headers = parse_simple_headers_json(&headers_json);

    let result = (|| -> Result<mango_net::HttpResponse, String> {
        let url = Url::parse(&url_str).map_err(|e| format!("Invalid URL: {e}"))?;

        let is_cross_origin = !origin_str.is_empty()
            && !mango_net::security::is_same_origin(&origin_str, &url_str);

        if is_cross_origin {
            if mode_str == "same-origin" {
                return Err("Same-Origin Policy: request to cross-origin resource blocked in same-origin mode".to_string());
            }

            if mode_str == "cors" {
                let headers_map: std::collections::HashMap<String, String> =
                    custom_headers.iter().cloned().collect();
                let is_simple =
                    mango_net::security::is_cors_simple_request(&method_str, &headers_map);
                if !is_simple {
                    let mut preflight_headers = Vec::new();
                    preflight_headers.push(("Origin".to_string(), origin_str.clone()));
                    preflight_headers.push((
                        "Access-Control-Request-Method".to_string(),
                        method_str.clone(),
                    ));
                    let req_h_keys: Vec<String> =
                        custom_headers.iter().map(|(k, _)| k.clone()).collect();
                    if !req_h_keys.is_empty() {
                        preflight_headers.push((
                            "Access-Control-Request-Headers".to_string(),
                            req_h_keys.join(", "),
                        ));
                    }
                    let preflight_resp = client
                        .request("OPTIONS", &url, &preflight_headers, None)
                        .map_err(|e| format!("CORS preflight failed: {e}"))?;

                    let header_refs: Vec<&str> =
                        req_h_keys.iter().map(|s| s.as_str()).collect();
                    let preflight_cors = mango_net::security::validate_cors_preflight(
                        preflight_resp.status,
                        &preflight_resp.headers,
                        &origin_str,
                        &method_str,
                        &header_refs,
                    );
                    if let Err(e) = preflight_cors {
                        return Err(format!("CORS preflight validation rejected: {e}"));
                    }
                }
                custom_headers.retain(|(k, _)| !k.eq_ignore_ascii_case("Origin"));
                custom_headers.push(("Origin".to_string(), origin_str.clone()));
            }
        }

        let body_opt = if body_str.is_empty() {
            None
        } else {
            Some(body_str.as_bytes())
        };

        let mut resp = client
            .request(&method_str, &url, &custom_headers, body_opt)
            .map_err(|e| e.to_string())?;

        if is_cross_origin && mode_str == "cors" {
            let exposed = mango_net::security::validate_cors_response(
                &resp.headers,
                &origin_str,
                creds_bool,
            )
            .map_err(|e| format!("CORS response validation failed: {e}"))?;
            resp.headers
                .retain(|k, _| exposed.contains(&k.to_ascii_lowercase()));
        }

        Ok(resp)
    })();

    format_fetch_response_json(result)
}

#[op2(fast)]
pub fn op_mango_start_fetch(
    state: &mut OpState,
    id: u32,
    #[string] url_str: String,
    #[string] method_str: String,
    #[string] headers_json: String,
    #[string] body_str: String,
    #[string] origin_str: String,
    #[string] mode_str: String,
    creds_bool: bool,
) {
    let event_loop = match state.try_borrow::<SharedEventLoop>() {
        Some(el) => el.clone(),
        None => return,
    };
    let cj = state
        .try_borrow::<CookieStore>()
        .cloned()
        .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(mango_net::CookieJar::new())));

    let tasks = event_loop.borrow().task_queue();

    std::thread::spawn(move || {
        let client = HttpClient::with_cookie_jar(cj);
        let mut custom_headers = parse_simple_headers_json(&headers_json);

        let result = (|| -> Result<mango_net::HttpResponse, String> {
            let url = Url::parse(&url_str).map_err(|e| format!("Invalid URL: {e}"))?;

            let is_cross_origin = !origin_str.is_empty()
                && !mango_net::security::is_same_origin(&origin_str, &url_str);

            if is_cross_origin {
                if mode_str == "same-origin" {
                    return Err("Same-Origin Policy: request to cross-origin resource blocked in same-origin mode".to_string());
                }

                if mode_str == "cors" {
                    let headers_map: std::collections::HashMap<String, String> =
                        custom_headers.iter().cloned().collect();
                    let is_simple =
                        mango_net::security::is_cors_simple_request(&method_str, &headers_map);
                    if !is_simple {
                        let mut preflight_headers = Vec::new();
                        preflight_headers.push(("Origin".to_string(), origin_str.clone()));
                        preflight_headers.push((
                            "Access-Control-Request-Method".to_string(),
                            method_str.clone(),
                        ));
                        let req_h_keys: Vec<String> =
                            custom_headers.iter().map(|(k, _)| k.clone()).collect();
                        if !req_h_keys.is_empty() {
                            preflight_headers.push((
                                "Access-Control-Request-Headers".to_string(),
                                req_h_keys.join(", "),
                            ));
                        }
                        let preflight_resp = client
                            .request("OPTIONS", &url, &preflight_headers, None)
                            .map_err(|e| format!("CORS preflight failed: {e}"))?;

                        let header_refs: Vec<&str> =
                            req_h_keys.iter().map(|s| s.as_str()).collect();
                        let preflight_cors = mango_net::security::validate_cors_preflight(
                            preflight_resp.status,
                            &preflight_resp.headers,
                            &origin_str,
                            &method_str,
                            &header_refs,
                        );
                        if let Err(e) = preflight_cors {
                            return Err(format!("CORS preflight validation rejected: {e}"));
                        }
                    }
                    custom_headers.retain(|(k, _)| !k.eq_ignore_ascii_case("Origin"));
                    custom_headers.push(("Origin".to_string(), origin_str.clone()));
                }
            }

            let body_opt = if body_str.is_empty() {
                None
            } else {
                Some(body_str.as_bytes())
            };

            let mut resp = client
                .request(&method_str, &url, &custom_headers, body_opt)
                .map_err(|e| e.to_string())?;

            if is_cross_origin && mode_str == "cors" {
                let exposed = mango_net::security::validate_cors_response(
                    &resp.headers,
                    &origin_str,
                    creds_bool,
                )
                .map_err(|e| format!("CORS response validation failed: {e}"))?;
                resp.headers
                    .retain(|k, _| exposed.contains(&k.to_ascii_lowercase()));
            }

            Ok(resp)
        })();

        let json = format_fetch_response_json(result);
        let js = format!(
            "if (typeof _mangoDispatchFetch === 'function') _mangoDispatchFetch({}, {});",
            id, json
        );
        if let Ok(mut q) = tasks.lock() {
            q.push(js);
        }
    });
}
