use deno_core::{OpState, op2};
use mango_net::http::HttpClient;
use mango_net::url::Url;

use crate::dom_bindings::CookieStore;
use crate::web_apis::SharedEventLoop;

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
            let ct = resp
                .headers
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "text/plain".to_string());
            let v = serde_json::json!({
                "ok": resp.is_success(),
                "status": resp.status,
                "statusText": resp.status_text,
                "contentType": ct,
                "headers": resp.headers,
                "body": body_str,
                "bodyBase64": body_b64,
                "error": null,
            });
            serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string())
        }
        Err(e) => {
            let v = serde_json::json!({
                "ok": false,
                "status": 0,
                "statusText": "",
                "contentType": "",
                "headers": {},
                "body": "",
                "bodyBase64": "",
                "error": e,
            });
            serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string())
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

    let client =
        HttpClient::with_timeout_and_cookie_jar(std::time::Duration::from_millis(2500), cj);
    let mut custom_headers = parse_simple_headers_json(&headers_json);

    let result = (|| -> Result<mango_net::HttpResponse, String> {
        let url = Url::parse(&url_str).map_err(|e| format!("Invalid URL: {e}"))?;

        let is_cross_origin =
            !origin_str.is_empty() && !mango_net::security::is_same_origin(&origin_str, &url_str);

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

                    let header_refs: Vec<&str> = req_h_keys.iter().map(|s| s.as_str()).collect();
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
            let exposed =
                mango_net::security::validate_cors_response(&resp.headers, &origin_str, creds_bool)
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
            "if (typeof globalThis._mangoResolveFetch === 'function') globalThis._mangoResolveFetch({}, {}); else if (typeof _mangoDispatchFetch === 'function') _mangoDispatchFetch({}, {});",
            id, json, id, json
        );
        if let Ok(mut q) = tasks.lock() {
            q.push(js);
        }
        mango_core::wake_event_loop();
    });
}
