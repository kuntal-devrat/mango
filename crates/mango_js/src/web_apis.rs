//! Non-DOM Web API bindings for the Mango JS runtime.
//!
//! Registers native Rust implementations of:
//!   - `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval`
//!   - `alert`
//!   - `_mangoNavigate` (internal navigation trigger)
//!   - `_mangoScheduleTimer` / `_mangoCancelTimer` (closure-preserving timer helpers)
//!   - `_mangoFetch` (synchronous HTTP bridge for the JS `fetch()` polyfill)
//!   - `_mangoLocalStorage*` (persistent localStorage native bridge)
//!
//! Then installs a large JS shim that builds the full browser global environment:
//! `window`, `location`, `navigator`, `screen`, `history`, `performance`,
//! `localStorage`/`sessionStorage`, `requestAnimationFrame`, `matchMedia`,
//! `getComputedStyle`, the DOM constructor hierarchy, event classes, observers,
//! `fetch()` with a real HTTP implementation, and `WebSocket` with graceful degradation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use boa_engine::{Context, JsArgs, JsValue, NativeFunction};
use mango_net::http::HttpClient;

use crate::event_loop::EventLoop;

// ── Shared handle types ───────────────────────────────────────────────────────

/// Shared mutable event loop accessible from both JS callbacks and the runtime.
pub type SharedEventLoop = Rc<RefCell<EventLoop>>;

/// Shared status text for `alert()` display.
pub type SharedStatusText = Rc<RefCell<String>>;

/// Shared pending navigation URL triggered by `location.href` assignment.
pub type SharedPendingNav = Rc<RefCell<Option<String>>>;

/// Thread-safe localStorage store.  Keyed by string; values are UTF-8 strings.
/// `Arc<Mutex<_>>` so the browser can persist it to disk after each page unload.
pub type SharedLocalStorage = Arc<Mutex<HashMap<String, String>>>;

/// Thread-safe sessionStorage store (tab-scoped, in-memory). Keyed by string (`{origin}\x1f{key}`).
pub type SharedSessionStorage = Arc<Mutex<HashMap<String, String>>>;

/// Escapes a profile-field value so one entry always occupies one line.
pub fn escape_profile_field(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Unescapes a profile-field value.
pub fn unescape_profile_field(s: &str) -> String {
    s.replace("%0D", "\r")
        .replace("%0A", "\n")
        .replace("%09", "\t")
        .replace("%25", "%")
}

/// Restores a `localStorage` profile (`<origin>\t<key>\t<value>` per line) into the store (GAP-017 / PRD 7.6).
pub fn load_local_storage_from_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<usize> {
    let body = std::fs::read_to_string(path)?;
    let mut count = 0usize;
    if let Ok(mut map) = store.lock() {
        for line in body.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let origin = unescape_profile_field(parts[0]);
                let k = unescape_profile_field(parts[1]);
                let value = unescape_profile_field(parts[2]);
                map.insert(format!("{origin}\x1f{k}"), value);
                count += 1;
            } else if parts.len() == 2 {
                map.insert(
                    unescape_profile_field(parts[0]),
                    unescape_profile_field(parts[1]),
                );
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Writes the `localStorage` store to its profile file (GAP-017 / PRD 7.6).
pub fn save_local_storage_to_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<()> {
    let map = match store.lock() {
        Ok(m) => m,
        Err(_) => {
            return Err(std::io::Error::other("Lock poisoned"));
        }
    };
    let mut body = String::from("# Mango localStorage v1\n");
    for (key, value) in map.iter() {
        if let Some((origin, k)) = key.split_once('\x1f') {
            body.push_str(&format!(
                "{}\t{}\t{}\n",
                escape_profile_field(origin),
                escape_profile_field(k),
                escape_profile_field(value)
            ));
        } else {
            body.push_str(&format!(
                "{}\t{}\n",
                escape_profile_field(key),
                escape_profile_field(value)
            ));
        }
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, body)?;
    if std::fs::rename(&tmp_path, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path)?;
    }
    Ok(())
}

/// Thread-safe cookie jar handle.
pub type SharedCookieStore = Arc<Mutex<mango_net::CookieJar>>;

#[cfg(windows)]
fn fill_secure_random(buf: &mut [u8]) {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    if !buf.is_empty() {
        unsafe {
            SystemFunction036(buf.as_mut_ptr(), buf.len() as u32);
        }
    }
}

#[cfg(not(windows))]
fn fill_secure_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
}

fn encode_base64_bytes(input: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };

        out.push(B64_CHARS[b0 >> 2] as char);
        out.push(B64_CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((b1 & 0xF) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[b2 & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

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

// ── Helper macro ──────────────────────────────────────────────────────────────

/// Creates an unsafe NativeFunction from a closure.
/// SAFETY: Mango is single-threaded; closures capture only `Rc<RefCell<_>>`
/// or `Arc<Mutex<_>>` values, never raw pointers or `!Send` references.
macro_rules! native_fn {
    ($closure:expr) => {
        unsafe { NativeFunction::from_closure($closure) }
    };
}

// ── Public registration entry point ──────────────────────────────────────────

/// Registers all browser Web APIs into the Boa context.
///
/// `local_storage` is a shared, externally-owned map so the browser can
/// load it from disk before page execution and flush it afterwards.
/// Pass `Arc::new(Mutex::new(HashMap::new()))` for a fresh in-memory store.
#[allow(clippy::too_many_arguments)]
pub fn register_web_apis(
    context: &mut Context,
    event_loop: SharedEventLoop,
    status_text: SharedStatusText,
    viewport_width: Rc<RefCell<f32>>,
    viewport_height: Rc<RefCell<f32>>,
    pending_nav: SharedPendingNav,
    page_url: &str,
    local_storage: SharedLocalStorage,
    session_storage: SharedSessionStorage,
    cookie_jar: SharedCookieStore,
) {
    register_native_fns(
        context,
        event_loop,
        status_text,
        viewport_width,
        viewport_height,
        pending_nav,
        page_url,
        local_storage,
        session_storage,
        cookie_jar,
    );
    install_js_shim(context, page_url);
}

// ── Native Rust function registrations ───────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn register_native_fns(
    context: &mut Context,
    event_loop: SharedEventLoop,
    status_text: SharedStatusText,
    viewport_width: Rc<RefCell<f32>>,
    viewport_height: Rc<RefCell<f32>>,
    pending_nav: SharedPendingNav,
    page_url: &str,
    local_storage: SharedLocalStorage,
    session_storage: SharedSessionStorage,
    cookie_jar: SharedCookieStore,
) {
    // ── Timers ────────────────────────────────────────────────────────────────

    let el = event_loop.clone();
    let set_timeout_fn = native_fn!(move |_this, args, ctx| {
        let callback = args.get_or_undefined(0);
        let delay = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0) as u64;
        let source = if callback.is_callable() {
            format!("({})()", callback.to_string(ctx)?.to_std_string_escaped())
        } else {
            callback.to_string(ctx)?.to_std_string_escaped()
        };
        let id = el.borrow_mut().schedule_timeout(source, delay);
        Ok(JsValue::from(id))
    });

    let el = event_loop.clone();
    let set_interval_fn = native_fn!(move |_this, args, ctx| {
        let callback = args.get_or_undefined(0);
        let delay = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0) as u64;
        let source = if callback.is_callable() {
            format!("({})()", callback.to_string(ctx)?.to_std_string_escaped())
        } else {
            callback.to_string(ctx)?.to_std_string_escaped()
        };
        let id = el.borrow_mut().schedule_interval(source, delay);
        Ok(JsValue::from(id))
    });

    let el = event_loop.clone();
    let clear_timeout_fn = native_fn!(move |_this, args, ctx| {
        let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        el.borrow_mut().cancel_timer(id);
        Ok(JsValue::undefined())
    });

    let el = event_loop.clone();
    let clear_interval_fn = native_fn!(move |_this, args, ctx| {
        let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        el.borrow_mut().cancel_timer(id);
        Ok(JsValue::undefined())
    });

    // ── Alert ─────────────────────────────────────────────────────────────────

    let st = status_text.clone();
    let alert_fn = native_fn!(move |_this, args, ctx| {
        let msg = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        log::info!("[JS alert] {}", msg);
        *st.borrow_mut() = format!("Alert: {}", msg);
        Ok(JsValue::undefined())
    });

    // ── Navigation ────────────────────────────────────────────────────────────

    let nav_slot = pending_nav.clone();
    let navigate_fn = native_fn!(move |_this, args, ctx| {
        let target = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        if !target.is_empty() {
            log::info!("[JS navigation] Script requested navigate to: {}", target);
            *nav_slot.borrow_mut() = Some(target);
        }
        Ok(JsValue::undefined())
    });

    // ── Closure-preserving timer helpers ─────────────────────────────────────

    let el = event_loop.clone();
    let schedule_timer_fn = native_fn!(move |_this, args, ctx| {
        let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let delay = args.get_or_undefined(1).to_u32(ctx).unwrap_or(0) as u64;
        let is_interval = args.get_or_undefined(2).to_boolean();
        let source = format!(
            "if (typeof _mangoRunTimer === 'function') _mangoRunTimer({});",
            id
        );
        if is_interval {
            el.borrow_mut().schedule_interval_with_id(id, source, delay);
        } else {
            el.borrow_mut().schedule_timeout_with_id(id, source, delay);
        }
        Ok(JsValue::from(id))
    });

    let el = event_loop.clone();
    let cancel_timer_fn = native_fn!(move |_this, args, ctx| {
        let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        el.borrow_mut().cancel_timer(id);
        Ok(JsValue::undefined())
    });

    // ── fetch() — synchronous HTTP bridge ─────────────────────────────────────
    //
    // JS calls `_mangoFetch(url, method, headersJson, body)` synchronously on
    // the Rust side.  We perform a blocking HTTP request and return a JSON
    // string that the JS `fetch()` polyfill deserialises into a Response object.
    //
    // NOTE: This is intentionally synchronous because Boa does not yet integrate
    // with an async executor.  It blocks the JS microtask queue while the request
    // is in-flight.  For script compatibility this is acceptable — nearly all
    // real-world `fetch()` usage wraps the call in `async/await` or `.then()`,
    // which are both driven by Promise callbacks that execute after _mangoFetch
    // returns.  The practical latency overhead is no different from a blocking
    // XMLHttpRequest call, which browsers historically allowed in workers.

    let random_bytes_fn = native_fn!(move |_this, args, ctx| {
        let count = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0) as usize;
        let count = count.min(65536);
        let mut bytes = vec![0u8; count];
        fill_secure_random(&mut bytes);
        let vals: Vec<JsValue> = bytes.into_iter().map(JsValue::from).collect();
        let arr = boa_engine::object::builtins::JsArray::from_iter(vals, ctx);
        Ok(JsValue::from(arr))
    });

    let el_fetch = event_loop.clone();
    let cj_fetch = cookie_jar.clone();
    let start_fetch_fn = native_fn!(move |_this, args, ctx| {
        let id = args.get_or_undefined(0).to_u32(ctx).unwrap_or(0);
        let url_str = args
            .get_or_undefined(1)
            .to_string(ctx)?
            .to_std_string_escaped();
        let method_str = args
            .get_or_undefined(2)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "GET".to_string())
            .to_uppercase();
        let headers_json = args
            .get_or_undefined(3)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let body_str = args
            .get_or_undefined(4)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let origin_str = args
            .get_or_undefined(5)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let mode_str = args
            .get_or_undefined(6)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "cors".to_string())
            .to_lowercase();
        let creds_bool = args.get_or_undefined(7).to_boolean();

        let tasks = el_fetch.borrow().task_queue();
        let jar = cj_fetch.clone();

        std::thread::spawn(move || {
            let client = HttpClient::with_cookie_jar(jar);
            let mut custom_headers = parse_simple_headers_json(&headers_json);

            let result = (|| -> Result<mango_net::HttpResponse, String> {
                let url = mango_net::url::Url::parse(&url_str)
                    .map_err(|e| format!("Invalid URL: {e}"))?;

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
                            // Perform OPTIONS preflight
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
                                .map_err(|e| format!("CORS preflight request failed: {e}"))?;
                            let header_refs: Vec<&str> =
                                req_h_keys.iter().map(|s| s.as_str()).collect();
                            mango_net::security::validate_cors_preflight(
                                preflight_resp.status,
                                &preflight_resp.headers,
                                &origin_str,
                                &method_str,
                                &header_refs,
                            )
                            .map_err(|e| format!("CORS preflight validation failed: {e}"))?;
                        }

                        // Attach Origin header to actual request
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

            let json_result = format_fetch_response_json(result);
            let escaped_for_js = json_result
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            let task_source = format!(
                "if (typeof _mangoResolveFetch === 'function') _mangoResolveFetch({}, \"{}\");",
                id, escaped_for_js
            );
            if let Ok(mut q) = tasks.lock() {
                q.push(task_source);
            }
        });

        Ok(JsValue::undefined())
    });

    let cj = cookie_jar.clone();
    let fetch_fn = native_fn!(move |_this, args, ctx| {
        let url_str = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let method_str = args
            .get_or_undefined(1)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "GET".to_string())
            .to_uppercase();
        let headers_json = args
            .get_or_undefined(2)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let body_str = args
            .get_or_undefined(3)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let origin_str = args
            .get_or_undefined(4)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let mode_str = args
            .get_or_undefined(5)
            .to_string(ctx)
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|_| "cors".to_string())
            .to_lowercase();
        let creds_bool = args.get_or_undefined(6).to_boolean();

        log::debug!("[fetch] {} {}", method_str, url_str);

        let client = HttpClient::with_cookie_jar(cj.clone());
        let mut custom_headers = parse_simple_headers_json(&headers_json);

        let result = (|| -> Result<mango_net::HttpResponse, String> {
            let url =
                mango_net::url::Url::parse(&url_str).map_err(|e| format!("Invalid URL: {e}"))?;

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
                            .map_err(|e| format!("CORS preflight request failed: {e}"))?;
                        let header_refs: Vec<&str> =
                            req_h_keys.iter().map(|s| s.as_str()).collect();
                        mango_net::security::validate_cors_preflight(
                            preflight_resp.status,
                            &preflight_resp.headers,
                            &origin_str,
                            &method_str,
                            &header_refs,
                        )
                        .map_err(|e| format!("CORS preflight validation failed: {e}"))?;
                    }

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

        let json_result = format_fetch_response_json(result);
        Ok(JsValue::from(boa_engine::js_string!(json_result.as_str())))
    });

    // ── Origin computation for storage partitioning ──────────────────────────
    let page_origin = match mango_net::Url::parse(page_url) {
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
        _ => page_url.to_string(),
    };

    // ── localStorage native bridge ────────────────────────────────────────────
    //
    // These native functions are called by the JS `localStorage` implementation.
    // Keys are partitioned per origin (RFC 6454 / W3C Web Storage spec).

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let ls_get_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        let val = ls.lock().ok().and_then(|map| map.get(&scoped_key).cloned());
        match val {
            Some(v) => Ok(JsValue::from(boa_engine::js_string!(v.as_str()))),
            None => Ok(JsValue::null()),
        }
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let ls_set_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let val = args
            .get_or_undefined(1)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        if let Ok(mut map) = ls.lock() {
            map.insert(scoped_key, val);
        }
        Ok(JsValue::undefined())
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let ls_remove_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        if let Ok(mut map) = ls.lock() {
            map.remove(&scoped_key);
        }
        Ok(JsValue::undefined())
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let prefix = format!("{orig}\x1f");
    let ls_clear_fn = native_fn!(move |_this, _args, _ctx| {
        if let Ok(mut map) = ls.lock() {
            map.retain(|k, _| !k.starts_with(&prefix));
        }
        Ok(JsValue::undefined())
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let prefix = format!("{orig}\x1f");
    let ls_keys_fn = native_fn!(move |_this, _args, ctx| {
        let keys: Vec<JsValue> = ls
            .lock()
            .map(|map| {
                map.keys()
                    .filter(|k| k.starts_with(&prefix) && !k[prefix.len()..].starts_with("__idb__"))
                    .map(|k| JsValue::from(boa_engine::js_string!(&k[prefix.len()..])))
                    .collect()
            })
            .unwrap_or_default();
        let arr = boa_engine::object::builtins::JsArray::from_iter(keys, ctx);
        Ok(JsValue::from(arr))
    });

    // ── sessionStorage native bridge ──────────────────────────────────────────
    //
    // These native functions are called by the JS `sessionStorage` implementation.
    // Keys are partitioned per origin and scoped to the tab browsing context.

    let ss = session_storage.clone();
    let orig = page_origin.clone();
    let ss_get_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        let val = ss.lock().ok().and_then(|map| map.get(&scoped_key).cloned());
        match val {
            Some(v) => Ok(JsValue::from(boa_engine::js_string!(v.as_str()))),
            None => Ok(JsValue::null()),
        }
    });

    let ss = session_storage.clone();
    let orig = page_origin.clone();
    let ss_set_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let val = args
            .get_or_undefined(1)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        if let Ok(mut map) = ss.lock() {
            map.insert(scoped_key, val);
        }
        Ok(JsValue::undefined())
    });

    let ss = session_storage.clone();
    let orig = page_origin.clone();
    let ss_remove_fn = native_fn!(move |_this, args, ctx| {
        let key = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let scoped_key = format!("{orig}\x1f{key}");
        if let Ok(mut map) = ss.lock() {
            map.remove(&scoped_key);
        }
        Ok(JsValue::undefined())
    });

    let ss = session_storage.clone();
    let orig = page_origin.clone();
    let prefix = format!("{orig}\x1f");
    let ss_clear_fn = native_fn!(move |_this, _args, _ctx| {
        if let Ok(mut map) = ss.lock() {
            map.retain(|k, _| !k.starts_with(&prefix));
        }
        Ok(JsValue::undefined())
    });

    let ss = session_storage.clone();
    let orig = page_origin.clone();
    let prefix = format!("{orig}\x1f");
    let ss_keys_fn = native_fn!(move |_this, _args, ctx| {
        let keys: Vec<JsValue> = ss
            .lock()
            .map(|map| {
                map.keys()
                    .filter(|k| k.starts_with(&prefix))
                    .map(|k| JsValue::from(boa_engine::js_string!(&k[prefix.len()..])))
                    .collect()
            })
            .unwrap_or_default();
        let arr = boa_engine::object::builtins::JsArray::from_iter(keys, ctx);
        Ok(JsValue::from(arr))
    });

    // ── IndexedDB native bridge ───────────────────────────────────────────────
    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let idb_load_fn = native_fn!(move |_this, args, ctx| {
        let name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let key = format!("{orig}\x1f__idb__{name}");
        let val = ls.lock().ok().and_then(|map| map.get(&key).cloned());
        match val {
            Some(v) => Ok(JsValue::from(boa_engine::js_string!(v.as_str()))),
            None => Ok(JsValue::null()),
        }
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let idb_save_fn = native_fn!(move |_this, args, ctx| {
        let name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let json = args
            .get_or_undefined(1)
            .to_string(ctx)?
            .to_std_string_escaped();
        let key = format!("{orig}\x1f__idb__{name}");
        if let Ok(mut map) = ls.lock() {
            map.insert(key, json);
        }
        Ok(JsValue::undefined())
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let idb_delete_fn = native_fn!(move |_this, args, ctx| {
        let name = args
            .get_or_undefined(0)
            .to_string(ctx)?
            .to_std_string_escaped();
        let key = format!("{orig}\x1f__idb__{name}");
        if let Ok(mut map) = ls.lock() {
            map.remove(&key);
        }
        Ok(JsValue::undefined())
    });

    let ls = local_storage.clone();
    let orig = page_origin.clone();
    let prefix = format!("{orig}\x1f__idb__");
    let idb_list_fn = native_fn!(move |_this, _args, ctx| {
        let names: Vec<JsValue> = ls
            .lock()
            .map(|map| {
                map.keys()
                    .filter(|k| k.starts_with(&prefix))
                    .map(|k| JsValue::from(boa_engine::js_string!(&k[prefix.len()..])))
                    .collect()
            })
            .unwrap_or_default();
        let arr = boa_engine::object::builtins::JsArray::from_iter(names, ctx);
        Ok(JsValue::from(arr))
    });

    // ── Register window dims ──────────────────────────────────────────────────

    let vw = *viewport_width.borrow();
    let vh = *viewport_height.borrow();
    context
        .register_global_property(
            boa_engine::js_string!("innerWidth"),
            JsValue::from(vw as i32),
            boa_engine::property::Attribute::WRITABLE
                | boa_engine::property::Attribute::CONFIGURABLE,
        )
        .expect("register innerWidth");
    context
        .register_global_property(
            boa_engine::js_string!("innerHeight"),
            JsValue::from(vh as i32),
            boa_engine::property::Attribute::WRITABLE
                | boa_engine::property::Attribute::CONFIGURABLE,
        )
        .expect("register innerHeight");

    // ── Register all callables ────────────────────────────────────────────────

    let fns: &[(&str, usize, NativeFunction)] = &[
        ("setTimeout", 2, set_timeout_fn),
        ("setInterval", 2, set_interval_fn),
        ("clearTimeout", 1, clear_timeout_fn),
        ("clearInterval", 1, clear_interval_fn),
        ("alert", 1, alert_fn),
        ("_mangoNavigate", 1, navigate_fn),
        ("_mangoScheduleTimer", 3, schedule_timer_fn),
        ("_mangoCancelTimer", 1, cancel_timer_fn),
        ("_mangoRandomBytes", 1, random_bytes_fn),
        ("_mangoStartFetch", 8, start_fetch_fn),
        ("_mangoFetch", 7, fetch_fn),
        ("_mangoLsGet", 1, ls_get_fn),
        ("_mangoLsSet", 2, ls_set_fn),
        ("_mangoLsRemove", 1, ls_remove_fn),
        ("_mangoLsClear", 0, ls_clear_fn),
        ("_mangoLsKeys", 0, ls_keys_fn),
        ("_mangoSsGet", 1, ss_get_fn),
        ("_mangoSsSet", 2, ss_set_fn),
        ("_mangoSsRemove", 1, ss_remove_fn),
        ("_mangoSsClear", 0, ss_clear_fn),
        ("_mangoSsKeys", 0, ss_keys_fn),
        ("_mangoIdbLoad", 1, idb_load_fn),
        ("_mangoIdbSave", 2, idb_save_fn),
        ("_mangoIdbDelete", 1, idb_delete_fn),
        ("_mangoIdbList", 0, idb_list_fn),
    ];

    for (name, len, func) in fns.iter().cloned() {
        context
            .register_global_callable(boa_engine::js_string!(name), len, func)
            .unwrap_or_else(|e| panic!("register {name}: {e}"));
    }
}

// ── JS shim ───────────────────────────────────────────────────────────────────

fn install_js_shim(context: &mut Context, page_url: &str) {
    let safe_url = page_url.replace('\\', "\\\\").replace('"', "\\\"");

    let shim = format!(
        r##"
(function() {{
    // ── 1. Window / global aliases ────────────────────────────────────────────
    globalThis.window = globalThis;
    globalThis.self   = globalThis;
    globalThis.top    = globalThis;
    globalThis.parent = globalThis;

    // ── 2. location ───────────────────────────────────────────────────────────
    function _parseUrlComponents(urlStr, baseStr) {{
        var str = String(urlStr !== undefined && urlStr !== null ? urlStr : '').trim();
        if (baseStr && str.indexOf('://') === -1 && str.indexOf('about:') !== 0) {{
            var b = _parseUrlComponents(baseStr);
            if (str.indexOf('//') === 0) {{
                str = b.protocol + '//' + str.substring(2);
            }} else if (str.charAt(0) === '/') {{
                str = b.origin + str;
            }} else if (str.charAt(0) === '?') {{
                str = b.origin + b.pathname + str;
            }} else if (str.charAt(0) === '#') {{
                str = b.origin + b.pathname + b.search + str;
            }} else {{
                var p = b.pathname;
                var lastSlash = p.lastIndexOf('/');
                var dir = lastSlash !== -1 ? p.substring(0, lastSlash + 1) : '/';
                str = b.origin + dir + str;
            }}
        }}

        var res = {{
            href: str,
            protocol: 'http:',
            host: '',
            hostname: '',
            port: '',
            pathname: '/',
            search: '',
            hash: '',
            origin: ''
        }};

        if (str.indexOf('about:') === 0) {{
            res.protocol = 'about:';
            res.origin = 'null';
            res.pathname = str.substring(6);
            res.href = str;
            return res;
        }}

        var schemeSep = str.indexOf('://');
        if (schemeSep === -1) {{
            res.pathname = str.charAt(0) === '/' ? str : '/' + str;
            res.origin = 'null';
            res.href = str;
            return res;
        }}

        res.protocol = str.substring(0, schemeSep + 1).toLowerCase();
        var afterScheme = str.substring(schemeSep + 3);

        var slashIdx = afterScheme.indexOf('/');
        var qIdx = afterScheme.indexOf('?');
        var hashIdx = afterScheme.indexOf('#');

        var authorityEnd = afterScheme.length;
        if (slashIdx !== -1 && slashIdx < authorityEnd) authorityEnd = slashIdx;
        if (qIdx !== -1 && qIdx < authorityEnd) authorityEnd = qIdx;
        if (hashIdx !== -1 && hashIdx < authorityEnd) authorityEnd = hashIdx;

        var authority = afterScheme.substring(0, authorityEnd);
        var rest = afterScheme.substring(authorityEnd);

        res.host = authority;
        var colonIdx = authority.lastIndexOf(':');
        if (colonIdx !== -1 && authority.indexOf(']') === -1) {{
            res.hostname = authority.substring(0, colonIdx);
            res.port = authority.substring(colonIdx + 1);
        }} else {{
            res.hostname = authority;
            res.port = '';
        }}
        res.origin = res.protocol + '//' + res.host;

        var pathEnd = rest.length;
        var rQIdx = rest.indexOf('?');
        var rHIdx = rest.indexOf('#');
        if (rQIdx !== -1 && rQIdx < pathEnd) pathEnd = rQIdx;
        if (rHIdx !== -1 && rHIdx < pathEnd) pathEnd = rHIdx;

        res.pathname = pathEnd > 0 ? rest.substring(0, pathEnd) : '/';
        if (res.pathname.charAt(0) !== '/') res.pathname = '/' + res.pathname;

        if (rQIdx !== -1) {{
            if (rHIdx !== -1 && rHIdx > rQIdx) {{
                res.search = rest.substring(rQIdx, rHIdx);
                res.hash = rest.substring(rHIdx);
            }} else {{
                res.search = rest.substring(rQIdx);
                res.hash = '';
            }}
        }} else if (rHIdx !== -1) {{
            res.search = '';
            res.hash = rest.substring(rHIdx);
        }} else {{
            res.search = '';
            res.hash = '';
        }}

        res.href = res.origin + res.pathname + res.search + res.hash;
        return res;
    }}

    var _curHref = "{safe_url}";
    var _locParsed = _parseUrlComponents(_curHref);
    var _historyStack = [{{ url: _curHref, state: null }}];
    var _historyIdx   = 0;

    function _updateLocationHref(newHref, triggerNav, isReplace) {{
        var parsed = _parseUrlComponents(newHref, _curHref);
        var oldHref = _curHref;
        var oldHash = _locParsed ? _locParsed.hash : '';
        _curHref = parsed.href;
        _locParsed = parsed;
        if (isReplace && _historyStack.length > 0) {{
            _historyStack[_historyIdx] = {{ url: _curHref, state: _historyStack[_historyIdx].state }};
        }} else if (triggerNav) {{
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({{ url: _curHref, state: null }});
            _historyIdx = _historyStack.length - 1;
        }}
        if (triggerNav && typeof _mangoNavigate === 'function') {{
            _mangoNavigate(_curHref);
        }}
        if (parsed.hash !== oldHash && !triggerNav) {{
            if (typeof window !== 'undefined' && window.dispatchEvent && typeof HashChangeEvent === 'function') {{
                window.dispatchEvent(new HashChangeEvent('hashchange', {{
                    oldURL: oldHref,
                    newURL: _curHref
                }}));
            }}
        }}
    }}

    function Location() {{}}
    Location.prototype = {{
        constructor: Location,
        get href() {{ return _locParsed.href; }},
        set href(val) {{
            _updateLocationHref(String(val), true, false);
        }},
        get protocol() {{ return _locParsed.protocol; }},
        set protocol(val) {{
            var p = String(val).toLowerCase();
            if (p.charAt(p.length - 1) !== ':') p += ':';
            var newHref = p + '//' + _locParsed.host + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get host() {{ return _locParsed.host; }},
        set host(val) {{
            var newHref = _locParsed.protocol + '//' + String(val) + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get hostname() {{ return _locParsed.hostname; }},
        set hostname(val) {{
            var portPart = _locParsed.port ? ':' + _locParsed.port : '';
            var newHref = _locParsed.protocol + '//' + String(val) + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get port() {{ return _locParsed.port; }},
        set port(val) {{
            var p = String(val).trim();
            var portPart = p ? ':' + p : '';
            var newHref = _locParsed.protocol + '//' + _locParsed.hostname + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get pathname() {{ return _locParsed.pathname; }},
        set pathname(val) {{
            var p = String(val);
            if (p.charAt(0) !== '/') p = '/' + p;
            var newHref = _locParsed.origin + p + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get search() {{ return _locParsed.search; }},
        set search(val) {{
            var s = String(val);
            if (s && s.charAt(0) !== '?') s = '?' + s;
            var newHref = _locParsed.origin + _locParsed.pathname + s + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get hash() {{ return _locParsed.hash; }},
        set hash(val) {{
            var h = String(val);
            if (h && h.charAt(0) !== '#') h = '#' + h;
            var oldHref = _curHref;
            var oldHash = _locParsed.hash;
            var newHref = _locParsed.origin + _locParsed.pathname + _locParsed.search + h;
            if (h !== oldHash) {{
                _historyStack.push({{ url: newHref, state: (globalThis.history ? globalThis.history.state : null) }});
                _historyIdx = _historyStack.length - 1;
                _updateLocationHref(newHref, false, false);
                if (typeof window !== 'undefined' && window.dispatchEvent && typeof PopStateEvent === 'function') {{
                    window.dispatchEvent(new PopStateEvent('popstate', {{
                        state: (globalThis.history ? globalThis.history.state : null)
                    }}));
                }}
            }}
        }},
        get origin() {{ return _locParsed.origin; }},
        assign: function(u) {{
            _updateLocationHref(String(u), true, false);
        }},
        replace: function(u) {{
            _updateLocationHref(String(u), true, true);
        }},
        reload: function() {{
            if (typeof _mangoNavigate === 'function') {{
                _mangoNavigate(_curHref);
            }}
        }},
        toString: function() {{
            return this.href;
        }}
    }};

    var _loc = Object.create(Location.prototype);
    globalThis.Location = Location;
    globalThis.location = _loc;

    // ── 3. navigator ─────────────────────────────────────────────────────────
    globalThis.navigator = {{
        userAgent:          "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Mango/0.1.0",
        appName:            "Netscape",
        appVersion:         "5.0 (Windows NT 10.0; Win64; x64)",
        platform:           "Win32",
        language:           "en-US",
        languages:          ["en-US", "en"],
        onLine:             true,
        cookieEnabled:      true,
        vendor:             "Google Inc.",
        hardwareConcurrency: 4,
        maxTouchPoints:     0,
        sendBeacon:         function() {{ return false; }},
        clipboard: {{
            readText:  function() {{ return Promise.resolve(""); }},
            writeText: function(t) {{ return Promise.resolve(); }}
        }},
        userAgentData: {{
            brands: [
                {{ brand: "Not-A.Brand", version: "24" }},
                {{ brand: "Chromium", version: "124" }},
                {{ brand: "Google Chrome", version: "124" }}
            ],
            mobile: false,
            platform: "Windows",
            getHighEntropyValues: function(hints) {{
                return Promise.resolve({{
                    architecture: "x86",
                    bitness: "64",
                    brands: this.brands,
                    mobile: false,
                    model: "",
                    platform: "Windows",
                    platformVersion: "15.0.0",
                    uaFullVersion: "124.0.0.0"
                }});
            }},
            toJSON: function() {{
                return {{ brands: this.brands, mobile: this.mobile, platform: this.platform }};
            }}
        }},
        connection: {{
            effectiveType: "4g",
            rtt: 50,
            downlink: 10,
            saveData: false,
            addEventListener: function() {{}},
            removeEventListener: function() {{}}
        }},
        permissions: {{
            query: function(desc) {{
                return Promise.resolve({{ state: "granted", addEventListener: function() {{}}, removeEventListener: function() {{}} }});
            }}
        }},
        mediaCapabilities: {{
            decodingInfo: function() {{ return Promise.resolve({{ supported: true, smooth: true, powerEfficient: true }}); }},
            encodingInfo: function() {{ return Promise.resolve({{ supported: true, smooth: true, powerEfficient: true }}); }}
        }},
        storage: {{
            estimate: function() {{ return Promise.resolve({{ quota: 1000000000, usage: 0 }}); }},
            persist:  function() {{ return Promise.resolve(true); }}
        }}
    }};

    // ── 4. screen ────────────────────────────────────────────────────────────
    globalThis.screen = {{
        width:       1920,
        height:      1080,
        availWidth:  1920,
        availHeight: 1040,
        colorDepth:  24,
        pixelDepth:  24
    }};

    // ── 5. history ───────────────────────────────────────────────────────────
    function History() {{}}
    History.prototype = {{
        constructor: History,
        get length() {{ return _historyStack.length; }},
        get state() {{ return _historyStack[_historyIdx] ? _historyStack[_historyIdx].state : null; }},
        get scrollRestoration() {{ return this._scrollRestoration || 'auto'; }},
        set scrollRestoration(v) {{ this._scrollRestoration = (v === 'manual') ? 'manual' : 'auto'; }},
        back: function() {{ this.go(-1); }},
        forward: function() {{ this.go(1); }},
        go: function(n) {{
            var delta = parseInt(n, 10) || 0;
            if (delta === 0) {{
                location.reload();
                return;
            }}
            var targetIdx = _historyIdx + delta;
            if (targetIdx >= 0 && targetIdx < _historyStack.length && targetIdx !== _historyIdx) {{
                var oldHref = _curHref;
                var oldHash = _locParsed.hash;
                _historyIdx = targetIdx;
                var entry = _historyStack[_historyIdx];
                _updateLocationHref(entry.url, false, false);
                if (typeof window !== 'undefined' && window.dispatchEvent && typeof PopStateEvent === 'function') {{
                    window.dispatchEvent(new PopStateEvent('popstate', {{
                        state: entry.state
                    }}));
                }}
            }}
        }},
        pushState: function(state, title, url) {{
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {{
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {{
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }}
                newUrl = resolved.href;
            }}
            var clonedState = null;
            if (state !== undefined && state !== null) {{
                try {{
                    clonedState = JSON.parse(JSON.stringify(state));
                }} catch(e) {{
                    clonedState = state;
                }}
            }}
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({{ url: newUrl, state: clonedState }});
            _historyIdx = _historyStack.length - 1;
            _updateLocationHref(newUrl, false, false);
        }},
        replaceState: function(state, title, url) {{
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {{
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {{
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }}
                newUrl = resolved.href;
            }}
            var clonedState = null;
            if (state !== undefined && state !== null) {{
                try {{
                    clonedState = JSON.parse(JSON.stringify(state));
                }} catch(e) {{
                    clonedState = state;
                }}
            }}
            _historyStack[_historyIdx] = {{ url: newUrl, state: clonedState }};
            _updateLocationHref(newUrl, false, false);
        }}
    }};

    var _hist = Object.create(History.prototype);
    _hist._scrollRestoration = 'auto';
    globalThis.History = History;
    globalThis.history = _hist;

    // ── 6. performance ───────────────────────────────────────────────────────
    var _p0 = Date.now();
    var _perfEntries = [];
    function PerformanceEntry(name, entryType, startTime, duration) {{
        this.name = String(name || '');
        this.entryType = String(entryType || '');
        this.startTime = Number(startTime || 0);
        this.duration = Number(duration || 0);
    }}
    PerformanceEntry.prototype.toJSON = function() {{
        return {{
            name: this.name,
            entryType: this.entryType,
            startTime: this.startTime,
            duration: this.duration
        }};
    }};
    globalThis.PerformanceEntry = PerformanceEntry;

    function PerformanceResourceTiming(name, initiatorType, startTime, duration, responseStatus) {{
        PerformanceEntry.call(this, name, 'resource', startTime, duration);
        this.initiatorType = initiatorType || 'fetch';
        this.responseStatus = responseStatus || 200;
        this.transferSize = 0;
        this.encodedBodySize = 0;
        this.decodedBodySize = 0;
        this.fetchStart = startTime;
        this.responseEnd = startTime + duration;
    }}
    PerformanceResourceTiming.prototype = Object.create(PerformanceEntry.prototype);
    PerformanceResourceTiming.prototype.constructor = PerformanceResourceTiming;
    globalThis.PerformanceResourceTiming = PerformanceResourceTiming;

    globalThis.performance = {{
        now:         function() {{ return Date.now() - _p0; }},
        timeOrigin:  _p0,
        timing:      {{ navigationStart: _p0, loadEventEnd: _p0 + 50 }},
        mark:        function(name) {{
            var entry = new PerformanceEntry(name, 'mark', Date.now() - _p0, 0);
            _perfEntries.push(entry);
            return entry;
        }},
        measure:     function(name, startMark, endMark) {{
            var start = 0;
            var end = Date.now() - _p0;
            if (startMark) {{
                for (var i = _perfEntries.length - 1; i >= 0; i--) {{
                    if (_perfEntries[i].name === startMark) {{ start = _perfEntries[i].startTime; break; }}
                }}
            }}
            if (endMark) {{
                for (var i = _perfEntries.length - 1; i >= 0; i--) {{
                    if (_perfEntries[i].name === endMark) {{ end = _perfEntries[i].startTime; break; }}
                }}
            }}
            var entry = new PerformanceEntry(name, 'measure', start, Math.max(0, end - start));
            _perfEntries.push(entry);
            return entry;
        }},
        clearMarks:  function(name) {{
            _perfEntries = _perfEntries.filter(function(e) {{
                return e.entryType !== 'mark' || (name && e.name !== name);
            }});
        }},
        clearMeasures: function(name) {{
            _perfEntries = _perfEntries.filter(function(e) {{
                return e.entryType !== 'measure' || (name && e.name !== name);
            }});
        }},
        clearResourceTimings: function() {{
            _perfEntries = _perfEntries.filter(function(e) {{ return e.entryType !== 'resource'; }});
        }},
        getEntries:  function() {{ return _perfEntries.slice(); }},
        getEntriesByName: function(name, type) {{
            return _perfEntries.filter(function(e) {{
                return e.name === name && (!type || e.entryType === type);
            }});
        }},
        getEntriesByType: function(type) {{
            return _perfEntries.filter(function(e) {{ return e.entryType === type; }});
        }}
    }};

    // ── 7. localStorage (backed by native Rust HashMap) ───────────────────────
    //
    // Reads/writes go through _mangoLs* native functions so the browser can
    // persist the map to disk across page loads (GAP-017 implemented).
    function Storage() {{}}
    var _storageProto = {{
        getItem: function(k) {{ return _mangoLsGet(String(k)); }},
        setItem: function(k, v) {{
            var old = _mangoLsGet(String(k));
            _mangoLsSet(String(k), String(v));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: String(k),
                    oldValue: old,
                    newValue: String(v),
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        removeItem: function(k) {{
            var old = _mangoLsGet(String(k));
            _mangoLsRemove(String(k));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: String(k),
                    oldValue: old,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        clear: function() {{
            _mangoLsClear();
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: null,
                    oldValue: null,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        get length() {{
            var keys = _mangoLsKeys();
            return keys ? keys.length : 0;
        }},
        key: function(i) {{
            var keys = _mangoLsKeys();
            return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
        }}
    }};
    Storage.prototype = _storageProto;
    globalThis.Storage = Storage;

    var _lsBase = Object.create(Storage.prototype);
    if (typeof Proxy !== 'undefined') {{
        globalThis.localStorage = new Proxy(_lsBase, {{
            get: function(target, prop) {{
                if (prop in target || typeof prop === 'symbol') {{
                    return target[prop];
                }}
                var val = target.getItem(prop);
                return val !== null ? val : undefined;
            }},
            set: function(target, prop, value) {{
                if (prop in target) {{
                    target[prop] = value;
                }} else {{
                    target.setItem(prop, value);
                }}
                return true;
            }},
            deleteProperty: function(target, prop) {{
                if (!(prop in target)) {{
                    target.removeItem(prop);
                }}
                return true;
            }},
            has: function(target, prop) {{
                if (prop in target) {{
                    return true;
                }}
                return target.getItem(prop) !== null;
            }},
            ownKeys: function(target) {{
                var keys = [];
                for (var i = 0; i < target.length; i++) {{
                    var k = target.key(i);
                    if (k !== null) keys.push(k);
                }}
                return keys;
            }}
        }});
    }} else {{
        globalThis.localStorage = _lsBase;
    }}

    // sessionStorage: tab-scoped in-memory, partitioned per origin.
    var _ssBase = Object.create(Storage.prototype);
    _ssBase.getItem = function(k) {{ return _mangoSsGet(String(k)); }};
    _ssBase.setItem = function(k, v) {{
        var old = _mangoSsGet(String(k));
        _mangoSsSet(String(k), String(v));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: String(k),
                oldValue: old,
                newValue: String(v),
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    _ssBase.removeItem = function(k) {{
        var old = _mangoSsGet(String(k));
        _mangoSsRemove(String(k));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: String(k),
                oldValue: old,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    _ssBase.clear = function() {{
        _mangoSsClear();
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: null,
                oldValue: null,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    Object.defineProperty(_ssBase, 'length', {{
        get: function() {{
            var keys = _mangoSsKeys();
            return keys ? keys.length : 0;
        }}
    }});
    _ssBase.key = function(i) {{
        var keys = _mangoSsKeys();
        return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
    }};

    if (typeof Proxy !== 'undefined') {{
        globalThis.sessionStorage = new Proxy(_ssBase, {{
            get: function(target, prop) {{
                if (prop in target || typeof prop === 'symbol') {{
                    return target[prop];
                }}
                var val = target.getItem(prop);
                return val !== null ? val : undefined;
            }},
            set: function(target, prop, value) {{
                if (prop in target) {{
                    target[prop] = value;
                }} else {{
                    target.setItem(prop, value);
                }}
                return true;
            }},
            deleteProperty: function(target, prop) {{
                if (!(prop in target)) {{
                    target.removeItem(prop);
                }}
                return true;
            }},
            has: function(target, prop) {{
                if (prop in target) {{
                    return true;
                }}
                return target.getItem(prop) !== null;
            }},
            ownKeys: function(target) {{
                var keys = [];
                for (var i = 0; i < target.length; i++) {{
                    var k = target.key(i);
                    if (k !== null) keys.push(k);
                }}
                return keys;
            }}
        }});
    }} else {{
        globalThis.sessionStorage = _ssBase;
    }}

    // ── 7b. EventTarget base class ───────────────────────────────────────────
    function EventTarget() {{
        this._listeners = {{}};
    }}
    EventTarget.prototype = {{
        addEventListener: function(type, listener, opts) {{
            if (typeof globalThis._addDOMEventListener === 'function') {{
                globalThis._addDOMEventListener(this, type, listener, opts);
            }} else {{
                if (!listener) return;
                var t = String(type);
                if (!this._listeners) this._listeners = {{}};
                if (!this._listeners[t]) this._listeners[t] = [];
                var cap = false, onc = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') {{ cap = !!opts.capture; onc = !!opts.once; }}
                for (var i = 0; i < this._listeners[t].length; i++) {{
                    if (this._listeners[t][i].fn === listener && this._listeners[t][i].capture === cap) return;
                }}
                this._listeners[t].push({{ fn: listener, capture: cap, once: onc }});
            }}
        }},
        removeEventListener: function(type, listener, opts) {{
            if (typeof globalThis._removeDOMEventListener === 'function') {{
                globalThis._removeDOMEventListener(this, type, listener, opts);
            }} else {{
                if (!this._listeners) return;
                var t = String(type);
                if (!this._listeners[t]) return;
                var cap = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') cap = !!opts.capture;
                this._listeners[t] = this._listeners[t].filter(function(item) {{
                    return !(item.fn === listener && item.capture === cap);
                }});
            }}
        }},
        dispatchEvent: function(evt) {{
            if (typeof globalThis._dispatchDOMEvent === 'function') {{
                return globalThis._dispatchDOMEvent(this, evt);
            }} else {{
                if (!evt) return true;
                var t = String(evt.type || evt);
                var handler = 'on' + t;
                if (typeof this[handler] === 'function') {{
                    try {{ this[handler].call(this, evt); }} catch(e) {{}}
                }}
                if (!this._listeners || !this._listeners[t]) return true;
                var list = this._listeners[t].slice();
                for (var i = 0; i < list.length; i++) {{
                    var item = list[i];
                    if (item.once) this.removeEventListener(t, item.fn, {{ capture: item.capture }});
                    try {{
                        if (typeof item.fn === 'function') item.fn.call(this, evt);
                        else if (item.fn && typeof item.fn.handleEvent === 'function') item.fn.handleEvent(evt);
                    }} catch(e) {{}}
                }}
                return true;
            }}
        }}
    }};
    globalThis.EventTarget = EventTarget;

    // ── 8. Window EventTarget ─────────────────────────────────────────────────
    globalThis.addEventListener = function(type, fn, opts) {{
        if (typeof globalThis._addDOMEventListener === 'function') {{
            globalThis._addDOMEventListener(globalThis, type, fn, opts);
        }}
    }};
    globalThis.removeEventListener = function(type, fn, opts) {{
        if (typeof globalThis._removeDOMEventListener === 'function') {{
            globalThis._removeDOMEventListener(globalThis, type, fn, opts);
        }}
    }};
    globalThis.dispatchEvent = function(evt) {{
        if (typeof globalThis._dispatchDOMEvent === 'function') {{
            return globalThis._dispatchDOMEvent(globalThis, evt);
        }}
        return true;
    }};

    // ── 9. Timers with closure preservation ──────────────────────────────────
    var _timerCallbacks = {{}};
    var _timerNextId = 1;
    globalThis._mangoRunTimer = function(id) {{
        var item = _timerCallbacks[id];
        if (!item) return;
        try {{
            if (typeof item.fn === 'function') item.fn.apply(globalThis, item.args || []);
            else if (typeof item.fn === 'string') (0, eval)(item.fn);
        }} catch(e) {{}}
        if (!item.repeat) delete _timerCallbacks[id];
    }};
    globalThis.setTimeout = function(fn, delay) {{
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = {{ fn: fn, repeat: false, args: args }};
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, false);
        return id;
    }};
    globalThis.setInterval = function(fn, delay) {{
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = {{ fn: fn, repeat: true, args: args }};
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, true);
        return id;
    }};
    globalThis.clearTimeout = function(id) {{
        var numId = Number(id) || 0;
        delete _timerCallbacks[numId];
        if (typeof _mangoCancelTimer === 'function') _mangoCancelTimer(numId);
    }};
    globalThis.clearInterval = globalThis.clearTimeout;

    // ── 10. requestAnimationFrame ─────────────────────────────────────────────
    globalThis.requestAnimationFrame = function(cb) {{
        return globalThis.setTimeout(function() {{
            try {{ if (typeof cb === 'function') cb(performance.now()); }} catch(e) {{}}
        }}, 16);
    }};
    globalThis.cancelAnimationFrame = globalThis.clearTimeout;
    globalThis.requestIdleCallback = function(cb, opts) {{
        return globalThis.setTimeout(function() {{
            var start = Date.now();
            try {{
                if (typeof cb === 'function') {{
                    cb({{
                        didTimeout: false,
                        timeRemaining: function() {{ return Math.max(0, 50 - (Date.now() - start)); }}
                    }});
                }}
            }} catch(e) {{}}
        }}, 1);
    }};
    globalThis.cancelIdleCallback = globalThis.clearTimeout;

    // ── 11. matchMedia ────────────────────────────────────────────────────────
    globalThis.matchMedia = function(q) {{
        var qs = String(q || "");
        // Honour prefers-color-scheme: dark — reported as false (light mode).
        // Honour hover and pointer features for desktop.
        var matchMap = {{
            "prefers-color-scheme: light": true,
            "prefers-color-scheme: dark":  false,
            "hover: hover":                true,
            "pointer: fine":               true
        }};
        var matched = false;
        for (var k in matchMap) {{
            if (qs.indexOf(k) !== -1) {{ matched = matchMap[k]; break; }}
        }}
        var minW = qs.match(/min-width:\s*(\d+)px/);
        var maxW = qs.match(/max-width:\s*(\d+)px/);
        if (minW || maxW) {{
            var curW = (typeof window !== 'undefined' && window.innerWidth) || globalThis.innerWidth || 800;
            var ok = true;
            if (minW && curW < parseInt(minW[1], 10)) {{ ok = false; }}
            if (maxW && curW > parseInt(maxW[1], 10)) {{ ok = false; }}
            matched = ok;
        }}
        var mql = {{
            matches:             matched,
            media:               qs,
            onchange:            null,
            addListener:         function(fn) {{ this.addEventListener('change', fn); }},
            removeListener:      function(fn) {{ this.removeEventListener('change', fn); }},
            addEventListener:    function() {{}},
            removeEventListener: function() {{}},
            dispatchEvent:       function() {{ return false; }}
        }};
        return mql;
    }};

    // ── 12. getComputedStyle & CSSStyleDeclaration ────────────────────────────
    function CSSStyleDeclaration(el) {{
        this._el = el || null;
    }}
    CSSStyleDeclaration.prototype.getPropertyValue = function(prop) {{
        var p = String(prop || '').trim();
        if (!p) return "";
        var camel = p.replace(/-([a-z])/g, function(g) {{ return g[1].toUpperCase(); }});
        if (this[camel] !== undefined) return String(this[camel]);
        if (this[p] !== undefined) return String(this[p]);
        return "";
    }};
    CSSStyleDeclaration.prototype.getPropertyPriority = function(prop) {{
        return "";
    }};
    CSSStyleDeclaration.prototype.setProperty = function(prop, val, prio) {{
        // Computed style declaration is read-only
    }};
    CSSStyleDeclaration.prototype.removeProperty = function(prop) {{
        return "";
    }};
    CSSStyleDeclaration.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : "";
    }};
    CSSStyleDeclaration.prototype[Symbol.iterator] = function() {{
        var self = this;
        var idx = 0;
        return {{
            next: function() {{
                if (idx < self.length) {{
                    return {{ value: self[idx++], done: false }};
                }} else {{
                    return {{ value: undefined, done: true }};
                }}
            }}
        }};
    }};
    CSSStyleDeclaration.prototype[Symbol.toStringTag] = 'CSSStyleDeclaration';
    globalThis.CSSStyleDeclaration = CSSStyleDeclaration;

    globalThis.getComputedStyle = function(el, pseudo) {{
        var cs = new CSSStyleDeclaration(el);
        var s = (el && el.style) ? el.style : {{}};

        var rect = (el && typeof el.getBoundingClientRect === 'function') ? el.getBoundingClientRect() : null;
        var widthStr = (rect && rect.width > 0) ? (rect.width + 'px') : (s.width || '0px');
        var heightStr = (rect && rect.height > 0) ? (rect.height + 'px') : (s.height || '0px');

        var tag = (el && el.tagName) ? el.tagName.toUpperCase() : '';
        var defaultDisplay = 'block';
        if (tag === 'SPAN' || tag === 'A' || tag === 'B' || tag === 'I' || tag === 'EM' || tag === 'STRONG' || tag === 'LABEL') {{
            defaultDisplay = 'inline';
        }} else if (tag === 'BUTTON' || tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA' || tag === 'IMG') {{
            defaultDisplay = 'inline-block';
        }} else if (tag === 'TABLE') {{
            defaultDisplay = 'table';
        }} else if (tag === 'TR') {{
            defaultDisplay = 'table-row';
        }} else if (tag === 'TD' || tag === 'TH') {{
            defaultDisplay = 'table-cell';
        }} else if (tag === 'LI') {{
            defaultDisplay = 'list-item';
        }}

        var props = {{
            display:          s.display          || defaultDisplay,
            visibility:       s.visibility       || 'visible',
            opacity:          s.opacity          || '1',
            width:            widthStr,
            height:           heightStr,
            color:            s.color            || 'rgb(0, 0, 0)',
            backgroundColor:  s.backgroundColor  || 'rgba(0, 0, 0, 0)',
            fontSize:         s.fontSize         || '16px',
            fontFamily:       s.fontFamily       || 'sans-serif',
            fontWeight:       s.fontWeight       || 'normal',
            lineHeight:       s.lineHeight       || 'normal',
            position:         s.position         || 'static',
            top:              s.top              || 'auto',
            left:             s.left             || 'auto',
            right:            s.right            || 'auto',
            bottom:           s.bottom           || 'auto',
            margin:           s.margin           || '0px',
            marginTop:        s.marginTop        || s.margin || '0px',
            marginRight:      s.marginRight      || s.margin || '0px',
            marginBottom:     s.marginBottom     || s.margin || '0px',
            marginLeft:       s.marginLeft       || s.margin || '0px',
            padding:          s.padding          || '0px',
            paddingTop:       s.paddingTop       || s.padding || '0px',
            paddingRight:     s.paddingRight     || s.padding || '0px',
            paddingBottom:    s.paddingBottom    || s.padding || '0px',
            paddingLeft:      s.paddingLeft      || s.padding || '0px',
            border:           s.border           || '0px none rgb(0, 0, 0)',
            borderWidth:      s.borderWidth      || '0px',
            borderTopWidth:   s.borderTopWidth   || s.borderWidth || '0px',
            borderRightWidth: s.borderRightWidth || s.borderWidth || '0px',
            borderBottomWidth:s.borderBottomWidth|| s.borderWidth || '0px',
            borderLeftWidth:  s.borderLeftWidth  || s.borderWidth || '0px',
            borderStyle:      s.borderStyle      || 'none',
            borderColor:      s.borderColor      || 'rgb(0, 0, 0)',
            boxSizing:        s.boxSizing        || 'content-box',
            transform:        s.transform        || 'none',
            transition:       s.transition       || 'none',
            overflow:         s.overflow         || 'visible',
            overflowX:        s.overflowX        || s.overflow || 'visible',
            overflowY:        s.overflowY        || s.overflow || 'visible',
            zIndex:           s.zIndex           || 'auto',
            flexDirection:    s.flexDirection    || 'row',
            alignItems:       s.alignItems       || 'stretch',
            justifyContent:   s.justifyContent   || 'flex-start',
            cursor:           s.cursor           || 'auto'
        }};

        var propNames = Object.keys(props);
        cs.length = propNames.length;
        for (var i = 0; i < propNames.length; i++) {{
            var k = propNames[i];
            var v = props[k];
            cs[k] = v;
            var kebab = k.replace(/([A-Z])/g, '-$1').toLowerCase();
            cs[kebab] = v;
            cs[i] = kebab;
        }}
        return cs;
    }};
    if (typeof window !== 'undefined') {{
        window.getComputedStyle = globalThis.getComputedStyle;
    }}

    // ── 13. URL & URLSearchParams ─────────────────────────────────────────────
    function URLSearchParams(init) {{
        this._entries = [];
        var self = this;
        if (init) {{
            if (typeof init === 'string') {{
                var s = init.replace(/^\?/, '');
                if (s.length > 0) {{
                    var pairs = s.split('&');
                    for (var i = 0; i < pairs.length; i++) {{
                        var pair = pairs[i];
                        var idx = pair.indexOf('=');
                        if (idx !== -1) {{
                            var k = decodeURIComponent(pair.substring(0, idx).replace(/\+/g, ' '));
                            var v = decodeURIComponent(pair.substring(idx + 1).replace(/\+/g, ' '));
                            this._entries.push([k, v]);
                        }} else if (pair.length > 0) {{
                            var k = decodeURIComponent(pair.replace(/\+/g, ' '));
                            this._entries.push([k, '']);
                        }}
                    }}
                }}
            }} else if (init instanceof URLSearchParams || (init._entries && Array.isArray(init._entries))) {{
                init.forEach(function(v, k) {{ self._entries.push([String(k), String(v)]); }});
            }} else if (Array.isArray(init)) {{
                for (var i = 0; i < init.length; i++) {{
                    var item = init[i];
                    if (item && item.length >= 2) {{
                        this._entries.push([String(item[0]), String(item[1])]);
                    }}
                }}
            }} else if (typeof init === 'object') {{
                var keys = Object.keys(init);
                for (var i = 0; i < keys.length; i++) {{
                    var k = keys[i];
                    this._entries.push([String(k), String(init[k])]);
                }}
            }}
        }}
    }}
    URLSearchParams.prototype = {{
        append: function(name, value) {{
            this._entries.push([String(name), String(value)]);
            if (this._url) this._url._syncFromSearchParams();
        }},
        delete: function(name, value) {{
            var strName = String(name);
            if (value !== undefined) {{
                var strVal = String(value);
                this._entries = this._entries.filter(function(e) {{ return !(e[0] === strName && e[1] === strVal); }});
            }} else {{
                this._entries = this._entries.filter(function(e) {{ return e[0] !== strName; }});
            }}
            if (this._url) this._url._syncFromSearchParams();
        }},
        get: function(name) {{
            var strName = String(name);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) return this._entries[i][1];
            }}
            return null;
        }},
        getAll: function(name) {{
            var strName = String(name);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) res.push(this._entries[i][1]);
            }}
            return res;
        }},
        has: function(name, value) {{
            var strName = String(name);
            if (value !== undefined) {{
                var strVal = String(value);
                for (var i = 0; i < this._entries.length; i++) {{
                    if (this._entries[i][0] === strName && this._entries[i][1] === strVal) return true;
                }}
                return false;
            }}
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) return true;
            }}
            return false;
        }},
        set: function(name, value) {{
            var strName = String(name);
            var strVal = String(value);
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) {{
                    if (!found) {{
                        newEntries.push([strName, strVal]);
                        found = true;
                    }}
                }} else {{
                    newEntries.push(this._entries[i]);
                }}
            }}
            if (!found) newEntries.push([strName, strVal]);
            this._entries = newEntries;
            if (this._url) this._url._syncFromSearchParams();
        }},
        sort: function() {{
            this._entries.sort(function(a, b) {{
                if (a[0] < b[0]) return -1;
                if (a[0] > b[0]) return 1;
                return 0;
            }});
            if (this._url) this._url._syncFromSearchParams();
        }},
        forEach: function(cb, thisArg) {{
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }}
        }},
        keys: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][0], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][1], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) {{
                        var e = self._entries[i++];
                        return {{ value: [e[0], e[1]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        toString: function() {{
            var parts = [];
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                parts.push(encodeURIComponent(e[0]).replace(/%20/g, '+') + '=' + encodeURIComponent(e[1]).replace(/%20/g, '+'));
            }}
            return parts.join('&');
        }}
    }};
    URLSearchParams.prototype[Symbol.iterator] = URLSearchParams.prototype.entries;
    Object.defineProperty(URLSearchParams.prototype, 'size', {{
        get: function() {{ return this._entries.length; }}
    }});
    globalThis.URLSearchParams = URLSearchParams;

    function URL(url, base) {{
        if (!url && url !== '') throw new TypeError('Invalid URL: url is required');
        var rawUrl = String(url);
        var full;
        if (base !== undefined && base !== null && base !== '') {{
            var baseStr = String(base);
            if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {{
                full = rawUrl;
            }} else {{
                var bSi = baseStr.indexOf('://');
                if (bSi === -1) throw new TypeError('Invalid base URL: ' + baseStr);
                var bScheme = baseStr.substring(0, bSi + 3);
                var bRest = baseStr.substring(bSi + 3);
                var bSlash = bRest.indexOf('/');
                var bHost = bSlash !== -1 ? bRest.substring(0, bSlash) : bRest;
                var bPath = bSlash !== -1 ? bRest.substring(bSlash).split('?')[0].split('#')[0] : '/';
                if (rawUrl.indexOf('//') === 0) {{
                    full = baseStr.substring(0, bSi + 1) + rawUrl;
                }} else if (rawUrl.charAt(0) === '/') {{
                    full = bScheme + bHost + rawUrl;
                }} else if (rawUrl.charAt(0) === '?') {{
                    full = bScheme + bHost + bPath + rawUrl;
                }} else if (rawUrl.charAt(0) === '#') {{
                    var bQuery = bRest.indexOf('?') !== -1 ? '?' + bRest.substring(bRest.indexOf('?') + 1).split('#')[0] : '';
                    full = bScheme + bHost + bPath + bQuery + rawUrl;
                }} else {{
                    var lastSlash = bPath.lastIndexOf('/');
                    var dir = lastSlash !== -1 ? bPath.substring(0, lastSlash + 1) : '/';
                    full = bScheme + bHost + dir + rawUrl;
                }}
            }}
        }} else {{
            if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {{
                throw new TypeError('Invalid URL: ' + rawUrl);
            }}
            full = rawUrl;
        }}
        
        this._searchParams = null;
        this._parse(full);
    }}
    URL.prototype = {{
        _parse: function(full) {{
            var si = full.indexOf('://');
            if (si === -1) {{
                var col = full.indexOf(':');
                if (col !== -1) {{
                    this.protocol = full.substring(0, col + 1).toLowerCase();
                    this.pathname = full.substring(col + 1);
                    this.host = '';
                    this.hostname = '';
                    this.port = '';
                    this.search = '';
                    this.hash = '';
                    this.origin = 'null';
                    this.href = full;
                    return;
                }}
                throw new TypeError('Invalid URL: ' + full);
            }}
            this.protocol = full.substring(0, si + 1).toLowerCase();
            var rest = full.substring(si + 3);
            var slash = rest.indexOf('/');
            var qm = rest.indexOf('?');
            var hash = rest.indexOf('#');
            var firstSep = Math.min(slash === -1 ? 99999 : slash, qm === -1 ? 99999 : qm, hash === -1 ? 99999 : hash);
            var authority = firstSep < 99999 ? rest.substring(0, firstSep) : rest;
            var pathAndRest = firstSep < 99999 ? rest.substring(firstSep) : '/';
            
            this.host = authority;
            var at = authority.indexOf('@');
            var hostPort = at !== -1 ? authority.substring(at + 1) : authority;
            var colon = hostPort.lastIndexOf(':');
            if (colon !== -1 && hostPort.indexOf(']') === -1) {{
                this.hostname = hostPort.substring(0, colon);
                this.port = hostPort.substring(colon + 1);
            }} else {{
                this.hostname = hostPort;
                this.port = '';
            }}
            
            var qIdx = pathAndRest.indexOf('?');
            var hIdx = pathAndRest.indexOf('#');
            if (qIdx !== -1) {{
                this.pathname = pathAndRest.substring(0, qIdx) || '/';
                if (hIdx !== -1 && hIdx > qIdx) {{
                    this.search = pathAndRest.substring(qIdx, hIdx);
                    this.hash = pathAndRest.substring(hIdx);
                }} else {{
                    this.search = pathAndRest.substring(qIdx);
                    this.hash = '';
                }}
            }} else if (hIdx !== -1) {{
                this.pathname = pathAndRest.substring(0, hIdx) || '/';
                this.search = '';
                this.hash = pathAndRest.substring(hIdx);
            }} else {{
                this.pathname = pathAndRest || '/';
                this.search = '';
                this.hash = '';
            }}
            var segments = this.pathname.split('/');
            var normalized = [];
            for (var si_idx = 0; si_idx < segments.length; si_idx++) {{
                var seg = segments[si_idx];
                if (seg === '' && si_idx > 0 && si_idx < segments.length - 1) continue;
                if (seg === '.') continue;
                if (seg === '..') {{
                    if (normalized.length > 1) {{
                        normalized.pop();
                    }}
                }} else {{
                    normalized.push(seg);
                }}
            }}
            this.pathname = normalized.join('/') || '/';
            if (!this.pathname.startsWith('/')) this.pathname = '/' + this.pathname;
            this.origin = this.protocol + '//' + this.host;
            this._updateHref();
        }},
        _updateHref: function() {{
            this.href = this.origin + this.pathname + this.search + this.hash;
        }},
        _syncFromSearchParams: function() {{
            if (!this._searchParams) return;
            var s = this._searchParams.toString();
            this.search = s.length > 0 ? '?' + s : '';
            this._updateHref();
        }},
        get searchParams() {{
            if (!this._searchParams) {{
                this._searchParams = new URLSearchParams(this.search);
                this._searchParams._url = this;
            }}
            return this._searchParams;
        }},
        toJSON: function() {{
            return this.href;
        }},
        toString: function() {{
            return this.href;
        }}
    }};
    URL.canParse = function(url, base) {{
        try {{
            new URL(url, base);
            return true;
        }} catch(e) {{
            return false;
        }}
    }};
    URL.createObjectURL = function(blob) {{
        return 'blob:mango-' + Math.random().toString(36).substring(2);
    }};
    URL.revokeObjectURL = function(url) {{}};
    globalThis.URL = URL;

    // ── 14. TextEncoder / TextDecoder ─────────────────────────────────────────
    function TextEncoder() {{
        this.encoding = 'utf-8';
    }}
    TextEncoder.prototype.encode = function(input) {{
        var s = String(input !== undefined ? input : '');
        var out = [];
        for (var i = 0; i < s.length; i++) {{
            var c = s.charCodeAt(i);
            if (c < 0x80) {{
                out.push(c);
            }} else if (c < 0x800) {{
                out.push(0xC0 | (c >> 6), 0x80 | (c & 0x3F));
            }} else if (c >= 0xD800 && c <= 0xDBFF && i + 1 < s.length) {{
                var c2 = s.charCodeAt(i + 1);
                if (c2 >= 0xDC00 && c2 <= 0xDFFF) {{
                    var codePoint = 0x10000 + ((c - 0xD800) << 10) + (c2 - 0xDC00);
                    out.push(0xF0 | (codePoint >> 18), 0x80 | ((codePoint >> 12) & 0x3F), 0x80 | ((codePoint >> 6) & 0x3F), 0x80 | (codePoint & 0x3F));
                    i++;
                    continue;
                }}
                out.push(0xEF, 0xBF, 0xBD);
            }} else {{
                out.push(0xE0 | (c >> 12), 0x80 | ((c >> 6) & 0x3F), 0x80 | (c & 0x3F));
            }}
        }}
        return new Uint8Array(out);
    }};
    TextEncoder.prototype.encodeInto = function(src, dest) {{
        var bytes = this.encode(src);
        var written = Math.min(bytes.length, dest.length);
        dest.set(bytes.subarray(0, written));
        return {{ read: String(src).length, written: written }};
    }};
    globalThis.TextEncoder = TextEncoder;

    function TextDecoder(enc, opts) {{
        this.encoding = enc ? String(enc).toLowerCase() : 'utf-8';
        this.fatal = !!(opts && opts.fatal);
        this.ignoreBOM = !!(opts && opts.ignoreBOM);
    }}
    TextDecoder.prototype.decode = function(input, opts) {{
        if (!input) return '';
        var arr;
        if (input instanceof ArrayBuffer) arr = new Uint8Array(input);
        else if (ArrayBuffer.isView(input)) arr = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
        else arr = new Uint8Array(input);
        
        var s = '';
        var i = 0;
        while (i < arr.length) {{
            var b = arr[i++];
            if (b < 0x80) {{
                s += String.fromCharCode(b);
            }} else if ((b & 0xE0) === 0xC0 && i < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x1F) << 6) | b2);
            }} else if ((b & 0xF0) === 0xE0 && i + 1 < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x0F) << 12) | (b2 << 6) | b3);
            }} else if ((b & 0xF8) === 0xF0 && i + 2 < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                var b4 = arr[i++] & 0x3F;
                var code = ((b & 0x07) << 18) | (b2 << 12) | (b3 << 6) | b4;
                code -= 0x10000;
                s += String.fromCharCode(0xD800 + (code >> 10), 0xDC00 + (code & 0x3FF));
            }} else {{
                s += '\uFFFD';
            }}
        }}
        return s;
    }};
    globalThis.TextDecoder = TextDecoder;

    // ── 15. btoa / atob ───────────────────────────────────────────────────────
    var _B64CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    globalThis.btoa = function(str) {{
        var s = String(str);
        var out = '';
        for (var i = 0; i < s.length; i += 3) {{
            var a = s.charCodeAt(i);
            var b = s.charCodeAt(i + 1);
            var c = s.charCodeAt(i + 2);
            if (a > 255 || b > 255 || c > 255) {{
                throw new Error('InvalidCharacterError: String contains characters outside Latin1 range');
            }}
            out += _B64CHARS[a >> 2];
            out += _B64CHARS[((a & 3) << 4) | (isNaN(b) ? 0 : b >> 4)];
            out += isNaN(b) ? '=' : _B64CHARS[((b & 0xF) << 2) | (isNaN(c) ? 0 : c >> 6)];
            out += (isNaN(b) || isNaN(c)) ? '=' : _B64CHARS[c & 0x3F];
        }}
        return out;
    }};
    globalThis.atob = function(str) {{
        var s = String(str).replace(/[\t\n\f\r ]+/g, '');
        if (s.length % 4 !== 0) throw new Error('InvalidCharacterError: Invalid base64 string length');
        var sClean = s.replace(/=+$/, '');
        var out = '';
        for (var i = 0; i < sClean.length; i += 4) {{
            var v0 = _B64CHARS.indexOf(sClean[i]);
            var v1 = _B64CHARS.indexOf(sClean[i + 1]);
            var v2 = i + 2 < sClean.length ? _B64CHARS.indexOf(sClean[i + 2]) : 0;
            var v3 = i + 3 < sClean.length ? _B64CHARS.indexOf(sClean[i + 3]) : 0;
            if (v0 === -1 || v1 === -1 || (i + 2 < sClean.length && v2 === -1) || (i + 3 < sClean.length && v3 === -1)) {{
                throw new Error('InvalidCharacterError: Invalid character in base64 string');
            }}
            var n = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
            out += String.fromCharCode((n >> 16) & 255);
            if (i + 2 < sClean.length) out += String.fromCharCode((n >> 8) & 255);
            if (i + 3 < sClean.length) out += String.fromCharCode(n & 255);
        }}
        return out;
    }};

    // ── 16. crypto ────────────────────────────────────────────────────────────
    function _sha256(data) {{
        var bytes;
        if (typeof data === 'string') bytes = new TextEncoder().encode(data);
        else if (data instanceof ArrayBuffer) bytes = new Uint8Array(data);
        else if (ArrayBuffer.isView(data)) bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        else bytes = new Uint8Array(data);

        function rightRotate(value, amount) {{ return (value >>> amount) | (value << (32 - amount)); }}
        var mathPow = Math.pow;
        var maxWord = mathPow(2, 32);
        var i, j;
        var words = [];
        var asciiBitLength = bytes.length * 8;
        var hash = [];
        var k = [];
        var primeCounter = 0;
        var isComposite = {{}};
        for (var candidate = 2; primeCounter < 64; candidate++) {{
            if (!isComposite[candidate]) {{
                for (i = 0; i < 313; i += candidate) isComposite[i] = candidate;
                hash[primeCounter] = (mathPow(candidate, .5) * maxWord) | 0;
                k[primeCounter++] = (mathPow(candidate, 1 / 3) * maxWord) | 0;
            }}
        }}
        for (i = 0; i < bytes.length; i++) {{
            words[i >> 2] |= bytes[i] << (24 - (i % 4) * 8);
        }}
        words[asciiBitLength >> 5] |= 0x80 << (24 - (asciiBitLength % 32));
        words[(((asciiBitLength + 64) >> 9) << 4) + 15] = asciiBitLength;
        var w = [];
        for (i = 0; i < words.length; i += 16) {{
            var h0 = hash[0], h1 = hash[1], h2 = hash[2], h3 = hash[3], h4 = hash[4], h5 = hash[5], h6 = hash[6], h7 = hash[7];
            for (j = 0; j < 64; j++) {{
                if (j < 16) w[j] = words[j + i] | 0;
                else {{
                    var s0 = rightRotate(w[j - 15], 7) ^ rightRotate(w[j - 15], 18) ^ (w[j - 15] >>> 3);
                    var s1 = rightRotate(w[j - 2], 17) ^ rightRotate(w[j - 2], 19) ^ (w[j - 2] >>> 10);
                    w[j] = (((w[j - 16] + s0) | 0) + ((w[j - 7] + s1) | 0)) | 0;
                }}
                var S1 = rightRotate(h4, 6) ^ rightRotate(h4, 11) ^ rightRotate(h4, 25);
                var ch = (h4 & h5) ^ ((~h4) & h6);
                var temp1 = (((h7 + S1) | 0) + ((ch + k[j]) | 0) + w[j]) | 0;
                var S0 = rightRotate(h0, 2) ^ rightRotate(h0, 13) ^ rightRotate(h0, 22);
                var maj = (h0 & h1) ^ (h0 & h2) ^ (h1 & h2);
                var temp2 = (S0 + maj) | 0;
                h7 = h6; h6 = h5; h5 = h4; h4 = (h3 + temp1) | 0;
                h3 = h2; h2 = h1; h1 = h0; h0 = (temp1 + temp2) | 0;
            }}
            hash[0] = (hash[0] + h0) | 0; hash[1] = (hash[1] + h1) | 0;
            hash[2] = (hash[2] + h2) | 0; hash[3] = (hash[3] + h3) | 0;
            hash[4] = (hash[4] + h4) | 0; hash[5] = (hash[5] + h5) | 0;
            hash[6] = (hash[6] + h6) | 0; hash[7] = (hash[7] + h7) | 0;
        }}
        var out = new Uint8Array(32);
        for (i = 0; i < 8; i++) {{
            out[i * 4] = (hash[i] >> 24) & 0xFF;
            out[i * 4 + 1] = (hash[i] >> 16) & 0xFF;
            out[i * 4 + 2] = (hash[i] >> 8) & 0xFF;
            out[i * 4 + 3] = hash[i] & 0xFF;
        }}
        return out.buffer;
    }}

    var _subtle = {{
        digest: function(algorithm, data) {{
            try {{
                return Promise.resolve(_sha256(data));
            }} catch(e) {{
                return Promise.reject(e);
            }}
        }},
        importKey: function() {{ return Promise.resolve({{}}); }},
        exportKey: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        generateKey: function() {{ return Promise.resolve({{}}); }},
        encrypt: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        decrypt: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        sign: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        verify: function() {{ return Promise.resolve(true); }}
    }};

    globalThis.crypto = {{
        subtle: _subtle,
        getRandomValues: function(buf) {{
            if (!buf || (typeof buf.byteLength !== 'number' && typeof buf.length !== 'number')) {{
                throw new TypeError('crypto.getRandomValues: invalid buffer');
            }}
            var len = buf.byteLength !== undefined ? buf.byteLength : buf.length;
            if (len > 65536) {{
                var err = new Error('crypto.getRandomValues: QuotaExceededError - requested length exceeds 65536 bytes');
                err.name = 'QuotaExceededError';
                throw err;
            }}
            if (typeof _mangoRandomBytes === 'function') {{
                var rand = _mangoRandomBytes(buf.length);
                for (var i = 0; i < rand.length; i++) {{
                    buf[i] = rand[i];
                }}
            }} else {{
                for (var i = 0; i < buf.length; i++) buf[i] = Math.floor(Math.random() * 256);
            }}
            return buf;
        }},
        randomUUID: function() {{
            var b = new Uint8Array(16);
            this.getRandomValues(b);
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            var hex = Array.from(b, function(x) {{ return x.toString(16).padStart(2, '0'); }});
            return hex[0]+hex[1]+hex[2]+hex[3]+'-'+hex[4]+hex[5]+'-'+hex[6]+hex[7]+'-'+hex[8]+hex[9]+'-'+hex[10]+hex[11]+hex[12]+hex[13]+hex[14]+hex[15];
        }}
    }};

    // ── 17. structuredClone ───────────────────────────────────────────────────
    globalThis.structuredClone = function(val, options) {{
        var seen = new Map();
        function clone(v) {{
            if (v === null || typeof v !== 'object') return v;
            if (seen.has(v)) return seen.get(v);
            
            if (v instanceof Date) return new Date(v.getTime());
            if (v instanceof RegExp) return new RegExp(v.source, v.flags);
            if (v instanceof ArrayBuffer) return v.slice(0);
            if (ArrayBuffer.isView(v)) {{
                if (v instanceof Uint8Array) return new Uint8Array(v);
                if (v instanceof Int8Array) return new Int8Array(v);
                if (v instanceof Uint16Array) return new Uint16Array(v);
                if (v instanceof Int16Array) return new Int16Array(v);
                if (v instanceof Uint32Array) return new Uint32Array(v);
                if (v instanceof Int32Array) return new Int32Array(v);
                if (v instanceof Float32Array) return new Float32Array(v);
                if (v instanceof Float64Array) return new Float64Array(v);
                return new Uint8Array(v);
            }}
            if (v instanceof Map) {{
                var m = new Map();
                seen.set(v, m);
                v.forEach(function(val, key) {{
                    m.set(clone(key), clone(val));
                }});
                return m;
            }}
            if (v instanceof Set) {{
                var s = new Set();
                seen.set(v, s);
                v.forEach(function(item) {{
                    s.add(clone(item));
                }});
                return s;
            }}
            if (Array.isArray(v)) {{
                var arr = [];
                seen.set(v, arr);
                for (var i = 0; i < v.length; i++) {{
                    arr[i] = clone(v[i]);
                }}
                return arr;
            }}
            
            var copy = {{}};
            seen.set(v, copy);
            var keys = Object.keys(v);
            for (var i = 0; i < keys.length; i++) {{
                var k = keys[i];
                copy[k] = clone(v[k]);
            }}
            return copy;
        }}
        return clone(val);
    }};

    // ── 18. FormData ──────────────────────────────────────────────────────────
    function FormData(form) {{
        this._entries = [];
        if (form && form.elements) {{
            var elements = Array.from(form.elements);
            for (var i = 0; i < elements.length; i++) {{
                var el = elements[i];
                if (el.name && el.value !== undefined && !el.disabled) {{
                    var type = (el.type || '').toLowerCase();
                    if (type === 'checkbox' || type === 'radio') {{
                        if (el.checked) this.append(el.name, el.value || 'on');
                    }} else if (type === 'file') {{
                        if (el.files && el.files.length) {{
                            for (var f = 0; f < el.files.length; f++) {{
                                this.append(el.name, el.files[f]);
                            }}
                        }} else {{
                            this.append(el.name, new File([], '', {{ type: 'application/octet-stream' }}));
                        }}
                    }} else {{
                        this.append(el.name, el.value);
                    }}
                }}
            }}
        }}
    }}
    FormData.prototype = {{
        append: function(k, v, filename) {{
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {{
                val = new File([val], filename);
            }}
            this._entries.push([String(k), val]);
        }},
        set: function(k, v, filename) {{
            var strKey = String(k);
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {{
                val = new File([val], filename);
            }}
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) {{
                    if (!found) {{
                        newEntries.push([strKey, val]);
                        found = true;
                    }}
                }} else {{
                    newEntries.push(this._entries[i]);
                }}
            }}
            if (!found) newEntries.push([strKey, val]);
            this._entries = newEntries;
        }},
        delete: function(k) {{
            var strKey = String(k);
            this._entries = this._entries.filter(function(e) {{ return e[0] !== strKey; }});
        }},
        get: function(k) {{
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) return this._entries[i][1];
            }}
            return null;
        }},
        getAll: function(k) {{
            var strKey = String(k);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) res.push(this._entries[i][1]);
            }}
            return res;
        }},
        has: function(k) {{
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) return true;
            }}
            return false;
        }},
        forEach: function(cb, thisArg) {{
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }}
        }},
        keys: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][0], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][1], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) {{
                        var e = self._entries[i++];
                        return {{ value: [e[0], e[1]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }}
    }};
    FormData.prototype[Symbol.iterator] = FormData.prototype.entries;
    globalThis.FormData = FormData;

    // ── 19. Blob / File / FileReader ──────────────────────────────────────────
    function Blob(parts, opts) {{
        this.type = (opts && opts.type) ? String(opts.type).toLowerCase() : '';
        var byteChunks = [];
        var totalSize = 0;
        if (parts && parts.length) {{
            for (var i = 0; i < parts.length; i++) {{
                var p = parts[i];
                if (p instanceof Blob) {{
                    var chunk = p._getBytes();
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else if (p instanceof ArrayBuffer) {{
                    var chunk = new Uint8Array(p);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else if (ArrayBuffer.isView(p)) {{
                    var chunk = new Uint8Array(p.buffer, p.byteOffset, p.byteLength);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else {{
                    var chunk = new TextEncoder().encode(String(p));
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }}
            }}
        }}
        var merged = new Uint8Array(totalSize);
        var offset = 0;
        for (var i = 0; i < byteChunks.length; i++) {{
            merged.set(byteChunks[i], offset);
            offset += byteChunks[i].length;
        }}
        this._bytes = merged;
        this.size = totalSize;
    }}
    Blob.prototype = {{
        _getBytes: function() {{
            return this._bytes;
        }},
        slice: function(start, end, ct) {{
            var len = this.size;
            var s = start === undefined ? 0 : (start < 0 ? Math.max(len + start, 0) : Math.min(start, len));
            var e = end === undefined ? len : (end < 0 ? Math.max(len + end, 0) : Math.min(end, len));
            var span = Math.max(e - s, 0);
            var sub = this._bytes.subarray(s, s + span);
            return new Blob([sub], {{ type: ct !== undefined ? ct : this.type }});
        }},
        text: function() {{
            return Promise.resolve(new TextDecoder().decode(this._bytes));
        }},
        arrayBuffer: function() {{
            var copy = new Uint8Array(this._bytes.length);
            copy.set(this._bytes);
            return Promise.resolve(copy.buffer);
        }},
        bytes: function() {{
            return Promise.resolve(new Uint8Array(this._bytes));
        }},
        stream: function() {{
            return null;
        }}
    }};
    globalThis.Blob = Blob;

    function File(parts, name, opts) {{
        Blob.call(this, parts, opts);
        this.name = String(name || '');
        this.lastModified = (opts && opts.lastModified) ? Number(opts.lastModified) : Date.now();
        this.webkitRelativePath = '';
    }}
    File.prototype = Object.create(Blob.prototype);
    globalThis.File = File;

    function FileReader() {{
        EventTarget.call(this);
        this.readyState = 0; // EMPTY
        this.result = null;
        this.error = null;
        this.onloadstart = null;
        this.onprogress = null;
        this.onload = null;
        this.onabort = null;
        this.onerror = null;
        this.onloadend = null;
        this._aborted = false;
    }}
    FileReader.EMPTY   = 0;
    FileReader.LOADING = 1;
    FileReader.DONE    = 2;
    FileReader.prototype = Object.create(EventTarget.prototype);
    Object.assign(FileReader.prototype, {{ EMPTY: 0, LOADING: 1, DONE: 2 }});

    FileReader.prototype.readAsText = function(blob, encoding) {{
        this._read(blob, 'text', encoding);
    }};
    FileReader.prototype.readAsDataURL = function(blob) {{
        this._read(blob, 'dataurl');
    }};
    FileReader.prototype.readAsArrayBuffer = function(blob) {{
        this._read(blob, 'arraybuffer');
    }};
    FileReader.prototype.readAsBinaryString = function(blob) {{
        this._read(blob, 'binarystring');
    }};
    FileReader.prototype.abort = function() {{
        this._aborted = true;
        this.readyState = 2; // DONE
        this.result = null;
        this.dispatchEvent({{ type: 'abort', target: this }});
        this.dispatchEvent({{ type: 'loadend', target: this }});
    }};
    FileReader.prototype._read = function(blob, mode, enc) {{
        var self = this;
        if (!(blob instanceof Blob)) {{
            throw new TypeError("Failed to execute read on 'FileReader': parameter 1 is not of type 'Blob'.");
        }}
        this._aborted = false;
        this.readyState = 1; // LOADING
        this.result = null;
        this.error = null;
        this.dispatchEvent({{ type: 'loadstart', target: this }});
        
        var schedule = (typeof queueMicrotask === 'function') ? queueMicrotask : function(fn) {{ setTimeout(fn, 0); }};
        schedule(function() {{
            if (self._aborted) return;
            var bytes = blob._getBytes();
            if (mode === 'text') {{
                self.result = new TextDecoder(enc || 'utf-8').decode(bytes);
            }} else if (mode === 'dataurl') {{
                var b64 = '';
                for (var i = 0; i < bytes.length; i++) b64 += String.fromCharCode(bytes[i]);
                self.result = 'data:' + (blob.type || 'application/octet-stream') + ';base64,' + btoa(b64);
            }} else if (mode === 'arraybuffer') {{
                var copy = new Uint8Array(bytes.length);
                copy.set(bytes);
                self.result = copy.buffer;
            }} else if (mode === 'binarystring') {{
                var bs = '';
                for (var i = 0; i < bytes.length; i++) bs += String.fromCharCode(bytes[i]);
                self.result = bs;
            }}
            self.readyState = 2; // DONE
            self.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: bytes.length, total: bytes.length, target: self }});
            self.dispatchEvent({{ type: 'load', target: self }});
            self.dispatchEvent({{ type: 'loadend', target: self }});
        }});
    }};
    globalThis.FileReader = FileReader;

    // ── 20. AbortController / AbortSignal ────────────────────────────────────
    function AbortSignal() {{
        EventTarget.call(this);
        this.aborted = false;
        this.reason = undefined;
        this.onabort = null;
    }}
    AbortSignal.prototype = Object.create(EventTarget.prototype);
    AbortSignal.prototype.throwIfAborted = function() {{
        if (this.aborted) throw (this.reason !== undefined ? this.reason : new Error('The operation was aborted.'));
    }};
    AbortSignal.abort = function(reason) {{
        var sig = new AbortSignal();
        sig.aborted = true;
        sig.reason = reason !== undefined ? reason : new Error('This operation was aborted');
        return sig;
    }};
    AbortSignal.timeout = function(ms) {{
        var c = new AbortController();
        setTimeout(function() {{
            c.abort(new Error('TimeoutError: The operation timed out.'));
        }}, ms);
        return c.signal;
    }};
    AbortSignal.any = function(signals) {{
        var sigs = Array.from(signals || []);
        var ac = new AbortController();
        for (var i = 0; i < sigs.length; i++) {{
            var s = sigs[i];
            if (s.aborted) {{
                ac.abort(s.reason);
                return ac.signal;
            }}
            s.addEventListener('abort', function() {{
                ac.abort(this.reason);
            }});
        }}
        return ac.signal;
    }};
    globalThis.AbortSignal = AbortSignal;

    function AbortController() {{
        this.signal = new AbortSignal();
    }}
    AbortController.prototype.abort = function(reason) {{
        if (!this.signal.aborted) {{
            this.signal.aborted = true;
            this.signal.reason = reason !== undefined ? reason : new Error('The operation was aborted.');
            var evt = {{ type: 'abort', target: this.signal, currentTarget: this.signal }};
            this.signal.dispatchEvent(evt);
        }}
    }};
    globalThis.AbortController = AbortController;

    // ── 21. fetch() ───────────────────────────────────────────────────────────
    function Headers(init) {{
        this._map = {{}};
        var self = this;
        if (init) {{
            if (init instanceof Headers || (init._map && typeof init._map === 'object')) {{
                for (var k in init._map) {{
                    this._map[k] = init._map[k];
                }}
            }} else if (Array.isArray(init)) {{
                for (var i = 0; i < init.length; i++) {{
                    var p = init[i];
                    if (p && p.length >= 2) this.append(p[0], p[1]);
                }}
            }} else if (typeof init === 'object') {{
                var keys = Object.keys(init);
                for (var i = 0; i < keys.length; i++) {{
                    var k = keys[i];
                    this.append(k, init[k]);
                }}
            }}
        }}
    }}
    Headers.prototype = {{
        append: function(k, v) {{
            var lk = String(k).toLowerCase();
            this._map[lk] = this._map[lk] ? this._map[lk] + ', ' + String(v) : String(v);
        }},
        delete: function(k) {{
            delete this._map[String(k).toLowerCase()];
        }},
        get: function(k) {{
            var lk = String(k).toLowerCase();
            return this._map[lk] !== undefined ? this._map[lk] : null;
        }},
        getSetCookie: function() {{
            var c = this.get('set-cookie');
            return c ? c.split(', ') : [];
        }},
        has: function(k) {{
            return String(k).toLowerCase() in this._map;
        }},
        set: function(k, v) {{
            this._map[String(k).toLowerCase()] = String(v);
        }},
        forEach: function(cb, thisArg) {{
            for (var k in this._map) {{
                cb.call(thisArg, this._map[k], k, this);
            }}
        }},
        keys: function() {{
            var keys = Object.keys(this._map);
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) return {{ value: keys[i++], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) return {{ value: self._map[keys[i++]], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) {{
                        var k = keys[i++];
                        return {{ value: [k, self._map[k]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }}
    }};
    Headers.prototype[Symbol.iterator] = Headers.prototype.entries;
    globalThis.Headers = Headers;

    function Request(input, init) {{
        init = init || {{}};
        if (input instanceof Request) {{
            this.url = input.url;
            this.method = init.method ? String(init.method).toUpperCase() : input.method;
            this.headers = new Headers(init.headers || input.headers);
            this.body = init.body !== undefined ? init.body : input.body;
            this.signal = init.signal || input.signal;
        }} else {{
            this.url = String(input || '');
            this.method = init.method ? String(init.method).toUpperCase() : 'GET';
            this.headers = new Headers(init.headers);
            this.body = init.body !== undefined ? init.body : null;
            this.signal = init.signal || null;
        }}
        this.mode = init.mode || 'cors';
        this.credentials = init.credentials || 'same-origin';
        this.cache = init.cache || 'default';
        this.redirect = init.redirect || 'follow';
        this.referrer = init.referrer || 'about:client';
        this.destination = init.destination || '';
        this.bodyUsed = false;
    }}
    Request.prototype = {{
        clone: function() {{
            return new Request(this, {{}});
        }},
        text: function() {{
            this.bodyUsed = true;
            return Promise.resolve(String(this.body || ''));
        }},
        json: function() {{
            this.bodyUsed = true;
            try {{ return Promise.resolve(JSON.parse(this.body || 'null')); }}
            catch(e) {{ return Promise.reject(e); }}
        }}
    }};
    globalThis.Request = Request;

    function Response(body, init) {{
        this._body = body !== undefined && body !== null ? body : '';
        this.status = (init && init.status !== undefined) ? Number(init.status) : 200;
        this.statusText = (init && init.statusText !== undefined) ? String(init.statusText) : (this.status === 200 ? 'OK' : String(this.status));
        this.ok = this.status >= 200 && this.status < 300;
        this.headers = new Headers(init && init.headers);
        this.url = (init && init.url) ? String(init.url) : '';
        this.type = (init && init.type) ? String(init.type) : 'default';
        this.redirected = !!(init && init.redirected);
        this.bodyUsed = false;
    }}
    Response.prototype = {{
        text: function() {{
            this.bodyUsed = true;
            if (typeof this._body === 'string') return Promise.resolve(this._body);
            if (this._body instanceof Blob) return this._body.text();
            if (this._body instanceof ArrayBuffer || ArrayBuffer.isView(this._body)) {{
                return Promise.resolve(new TextDecoder().decode(this._body));
            }}
            return Promise.resolve(String(this._body));
        }},
        json: function() {{
            var self = this;
            return this.text().then(function(t) {{
                return JSON.parse(t);
            }});
        }},
        blob: function() {{
            this.bodyUsed = true;
            if (this._body instanceof Blob) return Promise.resolve(this._body);
            var ct = this.headers.get('content-type') || 'text/plain';
            return Promise.resolve(new Blob([this._body], {{ type: ct }}));
        }},
        arrayBuffer: function() {{
            this.bodyUsed = true;
            if (this._body instanceof ArrayBuffer) return Promise.resolve(this._body);
            if (ArrayBuffer.isView(this._body)) return Promise.resolve(this._body.buffer);
            if (this._body instanceof Blob) return this._body.arrayBuffer();
            var enc = new TextEncoder().encode(String(this._body));
            return Promise.resolve(enc.buffer);
        }},
        formData: function() {{
            this.bodyUsed = true;
            var fd = new FormData();
            return this.text().then(function(t) {{
                var params = new URLSearchParams(t);
                params.forEach(function(v, k) {{ fd.append(k, v); }});
                return fd;
            }});
        }},
        clone: function() {{
            return new Response(this._body, {{
                status: this.status,
                statusText: this.statusText,
                headers: this.headers,
                url: this.url,
                type: this.type,
                redirected: this.redirected
            }});
        }}
    }};
    Response.error = function() {{
        var r = new Response(null, {{ status: 0, statusText: '' }});
        r.type = 'error';
        r.ok = false;
        return r;
    }};
    Response.redirect = function(url, status) {{
        status = status || 302;
        if ([301, 302, 303, 307, 308].indexOf(status) === -1) {{
            throw new RangeError('Invalid redirect status');
        }}
        return new Response(null, {{ status: status, headers: {{ Location: String(url) }} }});
    }};
    Response.json = function(data, init) {{
        var body = JSON.stringify(data);
        var hdrs = new Headers(init && init.headers);
        if (!hdrs.has('content-type')) hdrs.set('content-type', 'application/json');
        var opts = Object.assign({{}}, init, {{ headers: hdrs }});
        return new Response(body, opts);
    }};
    globalThis.Response = Response;

    // ── Cache API (caches.open, caches.match, cache.put, cache.delete, cache.keys) ──
    function Cache() {{
        this._items = [];
    }}
    Cache.prototype = {{
        match: function(request, options) {{
            var url = (request instanceof Request) ? request.url : String(request);
            for (var i = 0; i < this._items.length; i++) {{
                if (this._items[i].req.url === url) {{
                    return Promise.resolve(this._items[i].res.clone());
                }}
            }}
            return Promise.resolve(undefined);
        }},
        matchAll: function(request, options) {{
            var results = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            for (var i = 0; i < this._items.length; i++) {{
                if (!url || this._items[i].req.url === url) {{
                    results.push(this._items[i].res.clone());
                }}
            }}
            return Promise.resolve(results);
        }},
        add: function(request) {{
            return this.addAll([request]);
        }},
        addAll: function(requests) {{
            var self = this;
            var promises = requests.map(function(r) {{ return fetch(r); }});
            return Promise.all(promises).then(function(responses) {{
                for (var i = 0; i < requests.length; i++) {{
                    var req = (requests[i] instanceof Request) ? requests[i] : new Request(requests[i]);
                    self.put(req, responses[i]);
                }}
            }});
        }},
        put: function(request, response) {{
            var req = (request instanceof Request) ? request : new Request(request);
            var res = response.clone();
            for (var i = 0; i < this._items.length; i++) {{
                if (this._items[i].req.url === req.url) {{
                    this._items[i] = {{ req: req, res: res }};
                    return Promise.resolve();
                }}
            }}
            this._items.push({{ req: req, res: res }});
            return Promise.resolve();
        }},
        delete: function(request, options) {{
            var url = (request instanceof Request) ? request.url : String(request);
            for (var i = 0; i < this._items.length; i++) {{
                if (this._items[i].req.url === url) {{
                    this._items.splice(i, 1);
                    return Promise.resolve(true);
                }}
            }}
            return Promise.resolve(false);
        }},
        keys: function(request, options) {{
            var keys = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            for (var i = 0; i < this._items.length; i++) {{
                if (!url || this._items[i].req.url === url) {{
                    keys.push(this._items[i].req.clone());
                }}
            }}
            return Promise.resolve(keys);
        }}
    }};
    globalThis.Cache = Cache;

    function CacheStorage() {{
        this._caches = {{}};
    }}
    CacheStorage.prototype = {{
        open: function(cacheName) {{
            var name = String(cacheName);
            if (!this._caches[name]) {{
                this._caches[name] = new Cache();
            }}
            return Promise.resolve(this._caches[name]);
        }},
        has: function(cacheName) {{
            return Promise.resolve(Object.prototype.hasOwnProperty.call(this._caches, String(cacheName)));
        }},
        delete: function(cacheName) {{
            var name = String(cacheName);
            if (Object.prototype.hasOwnProperty.call(this._caches, name)) {{
                delete this._caches[name];
                return Promise.resolve(true);
            }}
            return Promise.resolve(false);
        }},
        keys: function() {{
            return Promise.resolve(Object.keys(this._caches));
        }},
        match: function(request, options) {{
            var self = this;
            var keys = Object.keys(this._caches);
            function checkNext(idx) {{
                if (idx >= keys.length) return Promise.resolve(undefined);
                return self._caches[keys[idx]].match(request, options).then(function(res) {{
                    if (res) return res;
                    return checkNext(idx + 1);
                }});
            }}
            return checkNext(0);
        }}
    }};
    globalThis.CacheStorage = CacheStorage;
    globalThis.caches = new CacheStorage();

    var _nextFetchId = 1;
    var _pendingFetches = {{}};

    globalThis._mangoResolveFetch = function(id, rawJson) {{
        var entry = _pendingFetches[id];
        if (!entry) return;
        delete _pendingFetches[id];

        var raw;
        try {{
            raw = typeof rawJson === 'string' ? JSON.parse(rawJson) : rawJson;
        }} catch(e) {{
            entry.reject(new TypeError('fetch: invalid response from native bridge: ' + e));
            return;
        }}

        if (raw.error) {{
            entry.reject(new TypeError('fetch failed: ' + raw.error));
        }} else {{
            var res = _makeResponse(raw);
            if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {{
                var dur = Math.max(0, (Date.now() - _p0) - (entry.startTime || 0));
                _perfEntries.push(new PerformanceResourceTiming(entry.url || '', 'fetch', entry.startTime || 0, dur, raw.status || 200));
            }}
            entry.resolve(res);
        }}
    }};

    function _makeResponse(raw) {{
        var hdrs = new Headers();
        if (raw.headers) {{
            for (var k in raw.headers) {{
                hdrs.set(k, raw.headers[k]);
            }}
        }}
        if (!hdrs.has('content-type') && raw.contentType) {{
            hdrs.set('content-type', raw.contentType);
        }}

        var r = new Response(raw.body || '', {{
            status: raw.status,
            statusText: raw.statusText || (raw.status === 200 ? 'OK' : String(raw.status)),
            headers: hdrs
        }});
        r.ok = raw.ok;
        r.type = 'basic';
        r._raw = raw;

        if (raw.bodyBase64 && typeof atob === 'function') {{
            r.arrayBuffer = function() {{
                this.bodyUsed = true;
                try {{
                    var bin = atob(raw.bodyBase64);
                    var buf = new Uint8Array(bin.length);
                    for (var i = 0; i < bin.length; i++) {{
                        buf[i] = bin.charCodeAt(i);
                    }}
                    return Promise.resolve(buf.buffer);
                }} catch(e) {{
                    var enc = new TextEncoder().encode(raw.body || '');
                    return Promise.resolve(enc.buffer);
                }}
            }};
            r.blob = function() {{
                this.bodyUsed = true;
                return this.arrayBuffer().then(function(buf) {{
                    return new Blob([buf], {{ type: raw.contentType || 'application/octet-stream' }});
                }});
            }};
        }}
        return r;
    }}

    globalThis.fetch = function(input, init) {{
        var req;
        if (input instanceof Request) {{
            req = init ? new Request(input, init) : input;
        }} else {{
            req = new Request(input, init);
        }}
        var signal = req.signal;
        if (signal && signal.aborted) {{
            return Promise.reject(new Error('The user aborted a request.'));
        }}
        var url = req.url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {{
            try {{
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            }} catch(e) {{}}
        }}
        
        var hdrs = {{}};
        req.headers.forEach(function(v, k) {{ hdrs[k] = v; }});
        var bodyStr = (req.body !== undefined && req.body !== null) ? String(req.body) : '';
        var pageOrigin = (globalThis.location && globalThis.location.origin) ? globalThis.location.origin : '';
        var reqMode = req.mode || (init && init.mode) || 'cors';
        var reqCreds = (req.credentials === 'include' || (init && init.credentials === 'include'));
        
        return new Promise(function(resolve, reject) {{
            if (signal) {{
                signal.addEventListener('abort', function() {{
                    reject(new Error('The user aborted a request.'));
                }});
            }}

            var fetchStart = (typeof _p0 !== 'undefined') ? (Date.now() - _p0) : 0;
            if (typeof _mangoStartFetch === 'function') {{
                var fetchId = _nextFetchId++;
                _pendingFetches[fetchId] = {{ resolve: resolve, reject: reject, url: url, startTime: fetchStart }};
                _mangoStartFetch(fetchId, url, req.method, JSON.stringify(hdrs), bodyStr, pageOrigin, reqMode, reqCreds);
            }} else if (typeof _mangoFetch === 'function') {{
                try {{
                    var resultJson = _mangoFetch(url, req.method, JSON.stringify(hdrs), bodyStr, pageOrigin, reqMode, reqCreds);
                    var raw = JSON.parse(resultJson);
                    if (raw.error) reject(new TypeError('fetch failed: ' + raw.error));
                    else {{
                        var res = _makeResponse(raw);
                        if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {{
                            var dur = Math.max(0, (Date.now() - _p0) - fetchStart);
                            _perfEntries.push(new PerformanceResourceTiming(url, 'fetch', fetchStart, dur, raw.status || 200));
                        }}
                        resolve(res);
                    }}
                }} catch(e) {{
                    reject(new TypeError('fetch error: ' + (e.message || e)));
                }}
            }} else {{
                reject(new TypeError('fetch: native bridge unavailable'));
            }}
        }});
    }};

    // ── 22. XMLHttpRequest (legacy compatibility) ─────────────────────────────
    function XMLHttpRequestUpload() {{
        EventTarget.call(this);
        this.onloadstart = null;
        this.onprogress  = null;
        this.onabort     = null;
        this.onerror     = null;
        this.onload      = null;
        this.ontimeout   = null;
        this.onloadend   = null;
    }}
    XMLHttpRequestUpload.prototype = Object.create(EventTarget.prototype);
    globalThis.XMLHttpRequestUpload = XMLHttpRequestUpload;

    function XMLHttpRequest() {{
        EventTarget.call(this);
        this.readyState    = 0; // UNSENT
        this.status        = 0;
        this.statusText    = '';
        this.responseText  = '';
        this.response      = '';
        this.responseType  = '';
        this.responseURL   = '';
        this.responseXML   = null;
        this.timeout       = 0;
        this.withCredentials = false;
        this.upload        = new XMLHttpRequestUpload();
        this.onreadystatechange = null;
        this.onloadstart   = null;
        this.onprogress    = null;
        this.onabort       = null;
        this.onerror       = null;
        this.onload        = null;
        this.ontimeout     = null;
        this.onloadend     = null;
        this._method       = 'GET';
        this._url          = '';
        this._async        = true;
        this._headers      = {{}};
        this._responseHeaders = {{}};
        this._aborted      = false;
    }}
    XMLHttpRequest.prototype = Object.create(EventTarget.prototype);
    var _xhrConsts = {{ UNSENT: 0, OPENED: 1, HEADERS_RECEIVED: 2, LOADING: 3, DONE: 4 }};
    Object.assign(XMLHttpRequest, _xhrConsts);
    Object.assign(XMLHttpRequest.prototype, _xhrConsts);

    XMLHttpRequest.prototype.open = function(method, url, async, user, password) {{
        this._method = String(method || 'GET').toUpperCase();
        this._url    = String(url || '');
        this._async  = async !== false;
        this._aborted = false;
        this._setReadyState(1 /* OPENED */);
    }};
    XMLHttpRequest.prototype.setRequestHeader = function(k, v) {{
        var lk = String(k).toLowerCase();
        this._headers[lk] = this._headers[lk] ? this._headers[lk] + ', ' + String(v) : String(v);
    }};
    XMLHttpRequest.prototype.getResponseHeader = function(k) {{
        if (this.readyState < 2) return null;
        var lk = String(k).toLowerCase();
        return this._responseHeaders[lk] !== undefined ? this._responseHeaders[lk] : null;
    }};
    XMLHttpRequest.prototype.getAllResponseHeaders = function() {{
        if (this.readyState < 2) return '';
        var lines = [];
        for (var k in this._responseHeaders) {{
            lines.push(k + ': ' + this._responseHeaders[k]);
        }}
        return lines.join('\r\n') + (lines.length > 0 ? '\r\n' : '');
    }};
    XMLHttpRequest.prototype.overrideMimeType = function(m) {{
        this._overrideMime = m;
    }};
    XMLHttpRequest.prototype.abort = function() {{
        if (this.readyState > 0 && this.readyState < 4) {{
            this._aborted = true;
            this._setReadyState(4);
            var evt = {{ type: 'abort', target: this, currentTarget: this }};
            this.dispatchEvent(evt);
            var endEvt = {{ type: 'loadend', target: this, currentTarget: this }};
            this.dispatchEvent(endEvt);
        }}
    }};
    XMLHttpRequest.prototype.send = function(body) {{
        var self = this;
        if (this._aborted) return;
        this.dispatchEvent({{ type: 'loadstart', target: this }});
        this.upload.dispatchEvent({{ type: 'loadstart', target: this.upload }});
        
        var hdrs = JSON.stringify(this._headers);
        var b    = (body !== undefined && body !== null) ? String(body) : '';
        var url  = this._url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {{
            try {{
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            }} catch(e) {{}}
        }}
        
        setTimeout(function() {{
            if (self._aborted) return;
            try {{
                self.upload.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: b.length, total: b.length, target: self.upload }});
                self.upload.dispatchEvent({{ type: 'load', target: self.upload }});
                self.upload.dispatchEvent({{ type: 'loadend', target: self.upload }});
                
                var result = JSON.parse(_mangoFetch(url, self._method, hdrs, b));
                self._setReadyState(2 /* HEADERS_RECEIVED */);
                self.status       = result.status;
                self.statusText   = result.status === 200 ? 'OK' : String(result.status);
                self.responseURL  = url;
                if (result.contentType) {{
                    self._responseHeaders['content-type'] = result.contentType;
                }}
                
                self._setReadyState(3 /* LOADING */);
                var bodyText = result.body || '';
                self.responseText = bodyText;
                if (self.responseType === 'json') {{
                    try {{ self.response = JSON.parse(bodyText); }} catch(_) {{ self.response = null; }}
                }} else if (self.responseType === 'arraybuffer') {{
                    self.response = new TextEncoder().encode(bodyText).buffer;
                }} else if (self.responseType === 'blob') {{
                    self.response = new Blob([bodyText], {{ type: result.contentType || 'text/plain' }});
                }} else {{
                    self.response = bodyText;
                }}
                
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: bodyText.length, total: bodyText.length, target: self }});
                if (result.ok) {{
                    self.dispatchEvent({{ type: 'load', target: self }});
                }} else {{
                    self.dispatchEvent({{ type: 'error', target: self }});
                }}
                self.dispatchEvent({{ type: 'loadend', target: self }});
            }} catch(e) {{
                self.status = 0;
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({{ type: 'error', target: self, message: e.message }});
                self.dispatchEvent({{ type: 'loadend', target: self }});
            }}
        }}, 0);
    }};
    XMLHttpRequest.prototype._setReadyState = function(rs) {{
        this.readyState = rs;
        this.dispatchEvent({{ type: 'readystatechange', target: this }});
    }};
    globalThis.XMLHttpRequest = XMLHttpRequest;

    // ── 23. WebSocket ─────────────────────────────────────────────────────────
    function WebSocket(url, protocols) {{
        EventTarget.call(this);
        this.url           = String(url || '');
        this.protocol      = '';
        this.extensions    = '';
        this.bufferedAmount = 0;
        this.readyState    = 0; // CONNECTING
        this.binaryType    = 'blob';
        this.onopen        = null;
        this.onclose       = null;
        this.onmessage     = null;
        this.onerror       = null;
        
        var self = this;
        setTimeout(function() {{
            self.readyState = 3; // CLOSED
            var errEvt = {{ type: 'error', target: self, message: 'WebSocket: real-time connections not yet supported in Mango' }};
            self.dispatchEvent(errEvt);
            var closeEvt = {{ type: 'close', target: self, code: 1001, reason: 'Mango does not yet support WebSocket', wasClean: false }};
            self.dispatchEvent(closeEvt);
        }}, 0);
    }}
    WebSocket.CONNECTING = 0;
    WebSocket.OPEN       = 1;
    WebSocket.CLOSING    = 2;
    WebSocket.CLOSED     = 3;
    WebSocket.prototype = Object.create(EventTarget.prototype);
    Object.assign(WebSocket.prototype, {{
        CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3,
        send: function(data) {{}},
        close: function(code, reason) {{ this.readyState = 3; }}
    }});
    globalThis.WebSocket = WebSocket;

    // ── 24. DOM constructor hierarchy ─────────────────────────────────────────

    globalThis.Node = function() {{}};
    globalThis.Node.prototype = Object.create(EventTarget.prototype);
    // Node type constants
    var _nodeConsts = {{ ELEMENT_NODE:1, ATTRIBUTE_NODE:2, TEXT_NODE:3, CDATA_SECTION_NODE:4,
        ENTITY_REFERENCE_NODE:5, ENTITY_NODE:6, PROCESSING_INSTRUCTION_NODE:7, COMMENT_NODE:8,
        DOCUMENT_NODE:9, DOCUMENT_TYPE_NODE:10, DOCUMENT_FRAGMENT_NODE:11, NOTATION_NODE:12 }};
    Object.assign(globalThis.Node, _nodeConsts);
    Object.assign(globalThis.Node.prototype, _nodeConsts);
    globalThis.Node.prototype.insertBefore    = function(n, r) {{ return this.appendChild ? this.appendChild(n) : n; }};
    globalThis.Node.prototype.replaceChild    = function(n, o) {{ if (this.removeChild) this.removeChild(o); return this.appendChild ? this.appendChild(n) : n; }};
    globalThis.Node.prototype.cloneNode       = function(deep) {{ return this; }};
    globalThis.Node.prototype.contains        = function(o) {{ return o === this; }};
    globalThis.Node.prototype.hasChildNodes   = function() {{ return (this.children && this.children.length > 0) || false; }};
    globalThis.Node.prototype.normalize       = function() {{}};
    globalThis.Node.prototype.isConnected     = true;
    globalThis.Node.prototype.isEqualNode     = function(o) {{ return this === o; }};
    globalThis.Node.prototype.isSameNode      = function(o) {{ return this === o; }};
    globalThis.Node.prototype.compareDocumentPosition = function() {{ return 0; }};

    globalThis.Element = function() {{}};
    globalThis.Element.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Element.prototype.getAttribute       = function(n) {{ return null; }};
    globalThis.Element.prototype.setAttribute       = function(n, v) {{}};
    globalThis.Element.prototype.removeAttribute    = function(n) {{}};
    globalThis.Element.prototype.hasAttribute       = function(n) {{ return false; }};
    globalThis.Element.prototype.toggleAttribute    = function(n, f) {{ var has = this.hasAttribute(n); if (f === undefined ? !has : f) this.setAttribute(n,''); else this.removeAttribute(n); return this.hasAttribute(n); }};
    globalThis.Element.prototype.getAttributeNS     = function(ns, n) {{ return this.getAttribute(n); }};
    globalThis.Element.prototype.setAttributeNS     = function(ns, n, v) {{ this.setAttribute(n, v); }};
    globalThis.Element.prototype.removeAttributeNS  = function(ns, n) {{ this.removeAttribute(n); }};
    globalThis.Element.prototype.hasAttributeNS     = function(ns, n) {{ return this.hasAttribute(n); }};
    globalThis.Element.prototype.getAttributeNames  = function() {{ return []; }};
    globalThis.Element.prototype.attachShadow = function(init) {{
        var sr = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {{}};
        sr.mode = (init && init.mode) || 'open'; sr.host = this; this.shadowRoot = sr; return sr;
    }};
    globalThis.Element.prototype.getBoundingClientRect = function() {{
        var r = (typeof _getElementRect === 'function' && this._nodeId !== undefined) ? _getElementRect(this._nodeId) : null;
        var x = (r && r[0]) || 0;
        var y = (r && r[1]) || 0;
        var w = (r && r[2]) || (this.offsetWidth || 0);
        var h = (r && r[3]) || (this.offsetHeight || 0);
        if (globalThis.DOMRect) return new globalThis.DOMRect(x, y, w, h);
        return {{
            x: x, y: y, left: x, top: y, width: w, height: h, right: x + w, bottom: y + h,
            toJSON: function() {{ return this; }}
        }};
    }};
    globalThis.Element.prototype.getClientRects = function() {{
        var r = this.getBoundingClientRect();
        if (globalThis.DOMRectList) return new globalThis.DOMRectList([r]);
        return [r];
    }};
    globalThis.Element.prototype.matches       = function(s) {{ return false; }};
    globalThis.Element.prototype.closest       = function(s) {{ return null; }};
    globalThis.Element.prototype.focus         = function() {{
        if (this.focus && this.focus !== globalThis.Element.prototype.focus) return this.focus.apply(this, arguments);
        if (this.disabled) return;
        if (typeof document !== 'undefined') {{
            var prev = document.activeElement;
            if (prev === this) return;
            document.activeElement = this;
            if (prev && prev !== this && prev.dispatchEvent) {{
                if (globalThis._setFocusState && prev._nodeId) globalThis._setFocusState(prev._nodeId, false);
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', {{ bubbles: false, relatedTarget: this }}));
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', {{ bubbles: true, relatedTarget: this }}));
            }}
            if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, true);
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focus', {{ bubbles: false, relatedTarget: prev }}));
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusin', {{ bubbles: true, relatedTarget: prev }}));
        }}
    }};
    globalThis.Element.prototype.blur          = function() {{
        if (this.blur && this.blur !== globalThis.Element.prototype.blur) return this.blur.apply(this, arguments);
        if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, false);
        if (typeof document !== 'undefined' && document.activeElement === this) {{
            document.activeElement = document.body || null;
        }}
        var next = (typeof document !== 'undefined') ? document.activeElement : null;
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', {{ bubbles: false, relatedTarget: next }}));
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', {{ bubbles: true, relatedTarget: next }}));
    }};
    globalThis.Element.prototype.click         = function() {{
        if (this.click && this.click !== globalThis.Element.prototype.click) return this.click.apply(this, arguments);
        if (this.disabled) return;
        var evt = (typeof globalThis.MouseEvent === 'function')
            ? new globalThis.MouseEvent('click', {{ bubbles: true, cancelable: true, view: (typeof window !== 'undefined' ? window : null), detail: 1 }})
            : {{ type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false }};
        var notPrevented = this.dispatchEvent(evt);
        if (notPrevented !== false && !evt.defaultPrevented) {{
            var tag = (this.tagName || '').toLowerCase();
            if (tag === 'input') {{
                var itype = (this.type || (this.getAttribute && this.getAttribute('type')) || 'text').toLowerCase();
                if (itype === 'checkbox') this.checked = !this.checked;
                else if (itype === 'radio') this.checked = true;
            }}
        }}
    }};
    globalThis.Element.prototype.scrollIntoView = function(options) {{
        var rect = this.getBoundingClientRect();
        var alignToTop = (options === undefined || options === true || (typeof options === 'object' && options.block !== 'end'));
        var top = (globalThis.scrollY || 0) + (alignToTop ? rect.top : (rect.bottom - ((typeof window !== 'undefined' && window.innerHeight) || 600)));
        var left = (globalThis.scrollX || 0) + rect.left;
        var behavior = (typeof options === 'object' && options.behavior) || 'auto';
        if (typeof globalThis.scrollTo === 'function') {{
            globalThis.scrollTo({{ top: top, left: left, behavior: behavior }});
        }}
    }};
    function _animateElemScroll(el, targetLeft, targetTop) {{
        var startLeft = el.scrollLeft || 0;
        var startTop = el.scrollTop || 0;
        var endLeft = (targetLeft !== undefined) ? targetLeft : startLeft;
        var endTop = (targetTop !== undefined) ? targetTop : startTop;
        var startTime = Date.now();
        var duration = 300;
        function step() {{
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            el.scrollLeft = startLeft + (endLeft - startLeft) * ease;
            el.scrollTop = startTop + (endTop - startTop) * ease;
            if (progress < 1.0) {{
                if (typeof globalThis.requestAnimationFrame === 'function') {{
                    globalThis.requestAnimationFrame(step);
                }} else if (typeof setTimeout === 'function') {{
                    setTimeout(step, 16);
                }}
            }}
        }}
        step();
    }}
    globalThis.Element.prototype.scrollTo      = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            if (x.behavior === 'smooth') {{
                _animateElemScroll(this, x.left, x.top);
            }} else {{
                if (x.left !== undefined) this.scrollLeft = x.left;
                if (x.top !== undefined) this.scrollTop = x.top;
            }}
        }} else {{
            if (x !== undefined) this.scrollLeft = x;
            if (y !== undefined) this.scrollTop = y;
        }}
    }};
    globalThis.Element.prototype.scrollBy      = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var curLeft = this.scrollLeft || 0;
            var curTop = this.scrollTop || 0;
            var targetLeft = (x.left !== undefined) ? curLeft + x.left : curLeft;
            var targetTop = (x.top !== undefined) ? curTop + x.top : curTop;
            if (x.behavior === 'smooth') {{
                _animateElemScroll(this, targetLeft, targetTop);
            }} else {{
                this.scrollLeft = targetLeft;
                this.scrollTop = targetTop;
            }}
        }} else {{
            if (x !== undefined) this.scrollLeft += x;
            if (y !== undefined) this.scrollTop += y;
        }}
    }};
    globalThis.Element.prototype.scroll        = globalThis.Element.prototype.scrollTo;
    globalThis.Element.prototype.prepend       = function() {{
        if (this.prepend && this.prepend !== globalThis.Element.prototype.prepend) return this.prepend.apply(this, arguments);
        var first = this.firstChild;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (first) this.insertBefore(node, first);
            else if (this.appendChild) this.appendChild(node);
        }}
    }};
    globalThis.Element.prototype.append        = function() {{
        if (this.append && this.append !== globalThis.Element.prototype.append) return this.append.apply(this, arguments);
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (this.appendChild) this.appendChild(node);
        }}
    }};
    globalThis.Element.prototype.before        = function() {{
        if (this.before && this.before !== globalThis.Element.prototype.before) return this.before.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, this);
        }}
    }};
    globalThis.Element.prototype.after         = function() {{
        if (this.after && this.after !== globalThis.Element.prototype.after) return this.after.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        var next = this.nextSibling;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, next);
        }}
    }};
    globalThis.Element.prototype.replaceWith   = function() {{
        if (this.replaceWith && this.replaceWith !== globalThis.Element.prototype.replaceWith) return this.replaceWith.apply(this, arguments);
        var p = this.parentNode;
        if (!p) return;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (p.insertBefore) p.insertBefore(node, this);
        }}
        if (p.removeChild) p.removeChild(this);
    }};
    globalThis.Element.prototype.replaceChildren = function() {{
        if (this.replaceChildren && this.replaceChildren !== globalThis.Element.prototype.replaceChildren) return this.replaceChildren.apply(this, arguments);
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            nodes.push(node);
        }}
        while (this.firstChild && this.removeChild) {{
            this.removeChild(this.firstChild);
        }}
        for (var j = 0; j < nodes.length; j++) {{
            if (this.appendChild) this.appendChild(nodes[j]);
        }}
    }};
    globalThis.Element.prototype.remove        = function() {{
        if (this.remove && this.remove !== globalThis.Element.prototype.remove) return this.remove.apply(this, arguments);
        if (this.parentNode && this.parentNode.removeChild) this.parentNode.removeChild(this);
    }};
    globalThis.Element.prototype.insertAdjacentHTML = function(pos, html) {{
        if (this.insertAdjacentHTML && this.insertAdjacentHTML !== globalThis.Element.prototype.insertAdjacentHTML) return this.insertAdjacentHTML.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p !== 'beforebegin' && p !== 'afterbegin' && p !== 'beforeend' && p !== 'afterend') {{
            throw new Error("SyntaxError: The value provided ('" + pos + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
        }}
        if (typeof document !== 'undefined' && document.createRange) {{
            var range = document.createRange();
            var frag = range.createContextualFragment(String(html));
            if (p === 'beforebegin') {{
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this);
            }} else if (p === 'afterbegin') {{
                if (this.insertBefore) this.insertBefore(frag, this.firstChild);
            }} else if (p === 'beforeend') {{
                if (this.appendChild) this.appendChild(frag);
            }} else if (p === 'afterend') {{
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this.nextSibling);
            }}
        }}
    }};
    globalThis.Element.prototype.insertAdjacentElement = function(pos, el) {{
        if (this.insertAdjacentElement && this.insertAdjacentElement !== globalThis.Element.prototype.insertAdjacentElement) return this.insertAdjacentElement.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p === 'beforebegin') {{
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this);
        }} else if (p === 'afterbegin') {{
            if (this.insertBefore) this.insertBefore(el, this.firstChild);
        }} else if (p === 'beforeend') {{
            if (this.appendChild) this.appendChild(el);
        }} else if (p === 'afterend') {{
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this.nextSibling);
        }} else {{
            throw new Error("SyntaxError: Invalid position: " + pos);
        }}
        return el;
    }};
    globalThis.Element.prototype.insertAdjacentText    = function(pos, text) {{
        if (this.insertAdjacentText && this.insertAdjacentText !== globalThis.Element.prototype.insertAdjacentText) return this.insertAdjacentText.apply(this, arguments);
        var node = (typeof document !== 'undefined' && document.createTextNode)
            ? document.createTextNode(String(text))
            : String(text);
        return this.insertAdjacentElement(pos, node);
    }};
    globalThis.Element.prototype.animate               = function() {{ return {{ finished: Promise.resolve(), cancel: function() {{}} }}; }};
    globalThis.Element.prototype.getAnimations         = function() {{ return []; }};

    globalThis.HTMLElement = function() {{}};
    globalThis.HTMLElement.prototype = Object.create(globalThis.Element.prototype);
    Object.defineProperties(globalThis.HTMLElement.prototype, {{
        offsetWidth: {{
            get: function() {{ return Math.round(this.getBoundingClientRect().width); }},
            configurable: true
        }},
        offsetHeight: {{
            get: function() {{ return Math.round(this.getBoundingClientRect().height); }},
            configurable: true
        }},
        offsetParent: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var cs = window.getComputedStyle(this);
                    if (cs && cs.position === 'fixed') return null;
                }}
                if (this.tagName === 'BODY' || this.tagName === 'HTML') return null;
                var cur = this.parentElement;
                while (cur && cur.tagName !== 'BODY' && cur.tagName !== 'HTML') {{
                    if (typeof window !== 'undefined' && window.getComputedStyle) {{
                        var s = window.getComputedStyle(cur);
                        if (s && s.position && s.position !== 'static') return cur;
                    }}
                    cur = cur.parentElement;
                }}
                if (cur && cur.tagName === 'BODY') return cur;
                return null;
            }},
            configurable: true
        }},
        offsetLeft: {{
            get: function() {{
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().left - p.getBoundingClientRect().left);
                return Math.round(this.getBoundingClientRect().left);
            }},
            configurable: true
        }},
        offsetTop: {{
            get: function() {{
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().top - p.getBoundingClientRect().top);
                return Math.round(this.getBoundingClientRect().top);
            }},
            configurable: true
        }},
        clientWidth: {{
            get: function() {{
                var bl = 0, br = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    bl = parseFloat(s.borderLeftWidth) || 0;
                    br = parseFloat(s.borderRightWidth) || 0;
                }}
                return Math.max(0, Math.round(this.getBoundingClientRect().width - bl - br));
            }},
            configurable: true
        }},
        clientHeight: {{
            get: function() {{
                var bt = 0, bb = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    bt = parseFloat(s.borderTopWidth) || 0;
                    bb = parseFloat(s.borderBottomWidth) || 0;
                }}
                return Math.max(0, Math.round(this.getBoundingClientRect().height - bt - bb));
            }},
            configurable: true
        }},
        clientTop: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderTopWidth) || 0);
                }}
                return 0;
            }},
            configurable: true
        }},
        clientLeft: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderLeftWidth) || 0);
                }}
                return 0;
            }},
            configurable: true
        }},
        scrollWidth: {{
            get: function() {{ return Math.max(this.clientWidth, Math.round(this.getBoundingClientRect().width)); }},
            configurable: true
        }},
        scrollHeight: {{
            get: function() {{ return Math.max(this.clientHeight, Math.round(this.getBoundingClientRect().height)); }},
            configurable: true
        }},
        scrollTop: {{
            get: function() {{ return this._scrollTop || 0; }},
            set: function(v) {{
                this._scrollTop = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {{
                    if (typeof window !== 'undefined') window.scrollY = this._scrollTop;
                }}
            }},
            configurable: true
        }},
        scrollLeft: {{
            get: function() {{ return this._scrollLeft || 0; }},
            set: function(v) {{
                this._scrollLeft = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {{
                    if (typeof window !== 'undefined') window.scrollX = this._scrollLeft;
                }}
            }},
            configurable: true
        }}
    }});

    globalThis.SVGElement    = function() {{}};
    globalThis.SVGElement.prototype = Object.create(globalThis.Element.prototype);
    globalThis.SVGSVGElement = function() {{}};
    globalThis.SVGSVGElement.prototype = Object.create(globalThis.SVGElement.prototype);
    globalThis.SVGSVGElement.prototype.createSVGPoint = function() {{ return {{ x:0, y:0, matrixTransform: function() {{ return this; }} }}; }};
    globalThis.SVGSVGElement.prototype.getScreenCTM   = function() {{ return {{ a:1, b:0, c:0, d:1, e:0, f:0, inverse: function() {{ return this; }} }}; }};

    // Specialised HTML element subclasses
    var _htmlSubclasses = [
        'HTMLTemplateElement','HTMLSlotElement','HTMLScriptElement','HTMLStyleElement','HTMLAnchorElement',
        'HTMLInputElement','HTMLButtonElement','HTMLDivElement','HTMLSpanElement',
        'HTMLIFrameElement','HTMLFormElement','HTMLSelectElement','HTMLOptionElement',
        'HTMLTableElement','HTMLTableRowElement','HTMLTableCellElement',
        'HTMLParagraphElement','HTMLHeadingElement','HTMLImageElement',
        'HTMLLIElement','HTMLUListElement','HTMLOListElement',
        'HTMLTextAreaElement','HTMLLabelElement','HTMLLinkElement','HTMLMetaElement',
        'HTMLTitleElement','HTMLBodyElement','HTMLHtmlElement','HTMLHeadElement',
        'HTMLBaseElement','HTMLBRElement','HTMLHRElement','HTMLPreElement',
        'HTMLDetailsElement','HTMLSummaryElement','HTMLDialogElement',
        'HTMLProgressElement','HTMLMeterElement','HTMLOutputElement',
        'HTMLDataListElement','HTMLFieldSetElement','HTMLLegendElement','HTMLOptGroupElement',
        'HTMLMapElement','HTMLAreaElement','HTMLObjectElement','HTMLEmbedElement',
        'HTMLSourceElement','HTMLPictureElement','HTMLTrackElement',
        'HTMLTimeElement','HTMLDataElement','HTMLRubyElement','HTMLTableSectionElement',
        'HTMLTableColElement','HTMLTableCaptionElement'
    ];
    _htmlSubclasses.forEach(function(name) {{
        globalThis[name] = function() {{}};
        globalThis[name].prototype = Object.create(globalThis.HTMLElement.prototype);
    }});

    // HTMLSelectElement prototype surface
    globalThis.HTMLSelectElement.prototype.checkValidity = function() {{
        return typeof this._computeValidity === 'function' ? this._computeValidity().valid : true;
    }};
    globalThis.HTMLSelectElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLSelectElement.prototype.setCustomValidity = function(message) {{
        if (message) {{
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        }} else {{
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }}
    }};
    globalThis.HTMLSelectElement.prototype.add = function(opt, before) {{
        if (before === undefined || before === null) {{
            this.appendChild(opt);
        }} else if (typeof before === 'number') {{
            var ref = (this.options || [])[before];
            if (ref) this.insertBefore(opt, ref);
            else this.appendChild(opt);
        }} else {{
            this.insertBefore(opt, before);
        }}
    }};
    globalThis.HTMLSelectElement.prototype.remove = function(index) {{
        var opts = this.options || [];
        var opt = opts[index];
        if (opt && opt.parentNode) opt.parentNode.removeChild(opt);
    }};

    // HTMLInputElement prototype surface
    globalThis.HTMLInputElement.prototype.stepUp = function(n) {{
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        var stepAttr = this.getAttribute ? this.getAttribute('step') : null;
        if (stepAttr && stepAttr.toLowerCase() === 'any') return;
        var step = parseFloat(stepAttr);
        if (isNaN(step) || step <= 0) step = 1.0;
        var cur = parseFloat(this.value);
        if (isNaN(cur)) {{
            var minVal = parseFloat(this.min);
            cur = isNaN(minVal) ? 0.0 : minVal;
        }}
        var next = cur + count * step;
        var min = parseFloat(this.min);
        if (!isNaN(min) && next < min) next = min;
        var max = parseFloat(this.max);
        if (!isNaN(max) && next > max) next = max;
        var decimals = 0;
        var stepStr = String(step);
        var dotIdx = stepStr.indexOf('.');
        if (dotIdx >= 0) decimals = stepStr.length - dotIdx - 1;
        this.value = decimals > 0 ? next.toFixed(decimals) : String(Math.round(next));
    }};
    globalThis.HTMLInputElement.prototype.stepDown = function(n) {{
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        this.stepUp(-count);
    }};
    globalThis.HTMLInputElement.prototype.checkValidity = function() {{
        var v = typeof this._computeValidity === 'function' ? this._computeValidity() : {{ valid: true }};
        if (!v.valid && this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'invalid', target: this }});
        }}
        return v.valid;
    }};
    globalThis.HTMLInputElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLInputElement.prototype.setCustomValidity = function(message) {{
        if (message) {{
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        }} else {{
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }}
    }};

    // HTMLFormElement prototype surface
    globalThis.HTMLFormElement.prototype.checkValidity = function() {{
        var elems = this.elements || [];
        var allValid = true;
        var firstInvalid = null;
        for (var i = 0; i < elems.length; i++) {{
            var el = elems[i];
            if (el.willValidate && typeof el.checkValidity === 'function' && !el.checkValidity()) {{
                if (!firstInvalid) firstInvalid = el;
                allValid = false;
            }}
        }}
        return allValid;
    }};
    globalThis.HTMLFormElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLFormElement.prototype.submit = function() {{
        if (this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'submit', target: this }});
        }}
    }};
    globalThis.HTMLFormElement.prototype.reset = function() {{
        var elems = this.elements || [];
        for (var i = 0; i < elems.length; i++) {{
            var el = elems[i];
            var elTag = (el.tagName || '').toLowerCase();
            if (elTag === 'input') {{
                var t = el.type;
                if (t === 'checkbox' || t === 'radio') {{
                    el.checked = el.defaultChecked;
                }} else {{
                    el.value = el.defaultValue;
                }}
            }} else if (elTag === 'textarea') {{
                el.value = el.defaultValue;
            }} else if (elTag === 'select') {{
                var opts = el.options || [];
                for (var j = 0; j < opts.length; j++) {{
                    opts[j].selected = opts[j].defaultSelected;
                }}
            }}
        }}
        if (this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'reset', target: this }});
        }}
    }};

    // HTMLDialogElement prototype surface
    globalThis.HTMLDialogElement.prototype.show = function() {{
        if (this.setAttribute) this.setAttribute('open', '');
        if (this.removeAttribute) this.removeAttribute('data-mango-modal');
    }};
    globalThis.HTMLDialogElement.prototype.showModal = function() {{
        if (this.hasAttribute && this.hasAttribute('open')) {{
            throw new Error('InvalidStateError: Dialog is already open');
        }}
        if (this.setAttribute) {{
            this.setAttribute('data-mango-modal', 'true');
            this.setAttribute('open', '');
        }}
        if (this.focus) this.focus();
    }};
    globalThis.HTMLDialogElement.prototype.close = function(retVal) {{
        if (this.hasAttribute && !this.hasAttribute('open')) return;
        if (retVal !== undefined) this.returnValue = String(retVal);
        if (this.removeAttribute) {{
            this.removeAttribute('open');
            this.removeAttribute('data-mango-modal');
        }}
        if (this.dispatchEvent) this.dispatchEvent({{ type: 'close', target: this }});
    }};
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'open', {{
        get: function() {{ return this.hasAttribute ? this.hasAttribute('open') : false; }},
        set: function(v) {{ if (v) {{ if (this.setAttribute) this.setAttribute('open', ''); }} else {{ if (this.removeAttribute) this.removeAttribute('open'); }} }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'returnValue', {{
        get: function() {{ return this._returnValue || ''; }},
        set: function(v) {{ this._returnValue = String(v); }},
        configurable: true
    }});

    // HTMLProgressElement prototype surface
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'value', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var n = parseFloat(v);
            return isNaN(n) ? 0 : Math.max(0, n);
        }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('value', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'max', {{
        get: function() {{
            var m = this.getAttribute ? this.getAttribute('max') : null;
            if (m === null) return 1.0;
            var n = parseFloat(m);
            return isNaN(n) || n <= 0 ? 1.0 : n;
        }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('max', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'position', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var val = this.value;
            var max = this.max;
            return max > 0 ? Math.max(0, Math.min(1, val / max)) : -1;
        }},
        configurable: true
    }});

    // HTMLMeterElement prototype surface
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'min', {{
        get: function() {{ var m = this.getAttribute ? this.getAttribute('min') : null; var n = parseFloat(m); return isNaN(n) ? 0.0 : n; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('min', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'max', {{
        get: function() {{ var m = this.getAttribute ? this.getAttribute('max') : null; var n = parseFloat(m); var min = this.min; return isNaN(n) ? Math.max(min, 1.0) : Math.max(min, n); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('max', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'value', {{
        get: function() {{ var v = this.getAttribute ? this.getAttribute('value') : null; var n = parseFloat(v); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('value', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'low', {{
        get: function() {{ var l = this.getAttribute ? this.getAttribute('low') : null; var n = parseFloat(l); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('low', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'high', {{
        get: function() {{ var h = this.getAttribute ? this.getAttribute('high') : null; var n = parseFloat(h); var min = this.min; var max = this.max; var low = this.low; if (isNaN(n)) return max; return Math.max(low, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('high', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'optimum', {{
        get: function() {{ var o = this.getAttribute ? this.getAttribute('optimum') : null; var n = parseFloat(o); var min = this.min; var max = this.max; if (isNaN(n)) return min + (max - min) / 2.0; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('optimum', String(v)); }},
        configurable: true
    }});

    // HTMLOutputElement prototype surface
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'value', {{
        get: function() {{ return this.textContent || ''; }},
        set: function(v) {{ this.textContent = String(v); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'defaultValue', {{
        get: function() {{ return this._defaultValue !== undefined ? this._defaultValue : (this.textContent || ''); }},
        set: function(v) {{ this._defaultValue = String(v); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'type', {{
        get: function() {{ return 'output'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'htmlFor', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('for') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('for', String(v)); }},
        configurable: true
    }});

    // HTMLDataListElement prototype surface
    Object.defineProperty(globalThis.HTMLDataListElement.prototype, 'options', {{
        get: function() {{ return this.getElementsByTagName ? this.getElementsByTagName('option') : []; }},
        configurable: true
    }});

    // HTMLInputElement.list surface
    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'list', {{
        get: function() {{
            var listId = this.getAttribute ? this.getAttribute('list') : null;
            if (!listId || typeof document === 'undefined') return null;
            var el = document.getElementById(listId);
            return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
        }},
        configurable: true
    }});

    // HTMLMapElement prototype surface
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'areas', {{
        get: function() {{ return this.getElementsByTagName ? this.getElementsByTagName('area') : []; }},
        configurable: true
    }});

    // HTMLAreaElement prototype surface
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'shape', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('shape') || 'rect') : 'rect'; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('shape', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'coords', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('coords') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('coords', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'alt', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('alt') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('alt', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'target', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('target') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('target', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'href', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('href') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('href', String(v)); }},
        configurable: true
    }});

    // HTMLObjectElement prototype surface
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'data', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('data') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('data', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'type', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('type') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('type', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'width', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('width') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('width', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'height', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('height') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('height', String(v)); }},
        configurable: true
    }});

    // HTMLEmbedElement prototype surface
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'src', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('src') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('src', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'type', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('type') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('type', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'width', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('width') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('width', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'height', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('height') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('height', String(v)); }},
        configurable: true
    }});

    globalThis.HTMLMediaElement = function() {{}};
    globalThis.HTMLMediaElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.HTMLMediaElement.NETWORK_EMPTY    = 0;
    globalThis.HTMLMediaElement.NETWORK_IDLE     = 1;
    globalThis.HTMLMediaElement.NETWORK_LOADING  = 2;
    globalThis.HTMLMediaElement.NETWORK_NO_SOURCE= 3;
    globalThis.HTMLMediaElement.HAVE_NOTHING     = 0;
    globalThis.HTMLMediaElement.HAVE_METADATA    = 1;
    globalThis.HTMLMediaElement.HAVE_CURRENT_DATA= 2;
    globalThis.HTMLMediaElement.HAVE_FUTURE_DATA = 3;
    globalThis.HTMLMediaElement.HAVE_ENOUGH_DATA = 4;
    Object.assign(globalThis.HTMLMediaElement.prototype, globalThis.HTMLMediaElement);
    globalThis.HTMLMediaElement.prototype.play = function() {{
        this._playing = true; this._paused = false;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'true');
        var self = this;
        setTimeout(function() {{
            if (self.dispatchEvent) {{ self.dispatchEvent({{ type:'play' }}); self.dispatchEvent({{ type:'playing' }}); }}
        }}, 0);
        return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
    }};
    globalThis.HTMLMediaElement.prototype.pause = function() {{
        this._playing = false; this._paused = true;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'false');
        var self = this;
        setTimeout(function() {{ if (self.dispatchEvent) self.dispatchEvent({{ type:'pause' }}); }}, 0);
    }};
    globalThis.HTMLMediaElement.prototype.load = function() {{
        var self = this;
        setTimeout(function() {{
            if (self.dispatchEvent) {{
                self.dispatchEvent({{ type:'loadstart' }});
                self.dispatchEvent({{ type:'loadedmetadata' }});
                self.dispatchEvent({{ type:'canplay' }});
            }}
        }}, 0);
    }};
    globalThis.HTMLMediaElement.prototype.canPlayType = function(type) {{
        if (!type) return '';
        var t = String(type).toLowerCase();
        if (t.indexOf('mp4') !== -1 || t.indexOf('webm') !== -1 || t.indexOf('ogg') !== -1 ||
            t.indexOf('audio/mpeg') !== -1 || t.indexOf('audio/mp3') !== -1 ||
            t.indexOf('audio/wav') !== -1 || t.indexOf('audio/ogg') !== -1 || t.indexOf('audio/aac') !== -1) {{
            return 'probably';
        }}
        return 'maybe';
    }};
    globalThis.HTMLVideoElement = function() {{}};
    globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLAudioElement = function() {{}};
    globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLCanvasElement = function() {{}};
    globalThis.HTMLCanvasElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.CanvasRenderingContext2D = function() {{}};
    globalThis.ImageData = function(dataOrWidth, widthOrHeight, heightOpt) {{
        if (typeof dataOrWidth === 'number') {{
            this.width  = Math.max(1, Math.floor(dataOrWidth));
            this.height = (typeof widthOrHeight === 'number') ? Math.max(1, Math.floor(widthOrHeight)) : 1;
            this.data   = new Uint8ClampedArray(this.width * this.height * 4);
        }} else {{
            this.data   = (dataOrWidth instanceof Uint8ClampedArray) ? dataOrWidth : new Uint8ClampedArray(dataOrWidth);
            this.width  = Math.max(1, Math.floor(widthOrHeight));
            this.height = heightOpt !== undefined ? Math.max(1, Math.floor(heightOpt)) : Math.floor(this.data.length / (this.width * 4));
        }}
    }};

    globalThis.Document = function() {{}};
    globalThis.Document.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Document.prototype.createElement      = function(t) {{ return (typeof document !== 'undefined') ? document.createElement(t) : null; }};
    globalThis.Document.prototype.createElementNS    = function(ns, t) {{ return (typeof document !== 'undefined' && document.createElementNS) ? document.createElementNS(ns, t) : this.createElement(t); }};
    globalThis.Document.prototype.createTextNode     = function(t) {{ return (typeof document !== 'undefined') ? document.createTextNode(t) : null; }};
    globalThis.Document.prototype.createDocumentFragment = function() {{ return (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null; }};
    globalThis.Document.prototype.querySelector      = function(s) {{ return (typeof document !== 'undefined') ? document.querySelector(s) : null; }};
    globalThis.Document.prototype.querySelectorAll   = function(s) {{ return (typeof document !== 'undefined') ? document.querySelectorAll(s) : []; }};
    globalThis.Document.prototype.getElementById     = function(i) {{ return (typeof document !== 'undefined') ? document.getElementById(i) : null; }};
    globalThis.Document.prototype.getElementsByTagName    = function(t) {{ return (typeof document !== 'undefined') ? document.getElementsByTagName(t) : []; }};
    globalThis.Document.prototype.getElementsByClassName  = function(c) {{ return (typeof document !== 'undefined') ? document.getElementsByClassName(c) : []; }};
    globalThis.Document.prototype.createEvent        = function(t) {{ return (typeof document !== 'undefined' && document.createEvent) ? document.createEvent(t) : new globalThis.Event(t); }};
    globalThis.Document.prototype.importNode         = function(n, d) {{ return n && n.cloneNode ? n.cloneNode(d) : n; }};
    globalThis.Document.prototype.adoptNode          = function(n) {{ return n; }};
    globalThis.Document.prototype.elementFromPoint   = function(x, y) {{ return (typeof document !== 'undefined' && document.elementFromPoint) ? document.elementFromPoint(x, y) : null; }};
    globalThis.Document.prototype.elementsFromPoint  = function(x, y) {{ return (typeof document !== 'undefined' && document.elementsFromPoint) ? document.elementsFromPoint(x, y) : []; }};
    globalThis.Document.prototype.caretPositionFromPoint = function() {{ return null; }};
    globalThis.Document.prototype.createRange        = function() {{
        return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : new globalThis.Range();
    }};
    globalThis.Document.prototype.getSelection      = function() {{
        return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null;
    }};
    Object.defineProperty(globalThis.Document.prototype, 'activeElement', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.activeElement : null; }},
        set: function(val) {{ if (typeof document !== 'undefined') document.activeElement = val; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'characterSet', {{
        get: function() {{ return (typeof document !== 'undefined' && document.characterSet) ? document.characterSet : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'charset', {{
        get: function() {{ return (typeof document !== 'undefined' && document.charset) ? document.charset : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'inputEncoding', {{
        get: function() {{ return (typeof document !== 'undefined' && document.inputEncoding) ? document.inputEncoding : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'contentType', {{
        get: function() {{ return (typeof document !== 'undefined' && document.contentType) ? document.contentType : 'text/html'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'documentElement', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.documentElement : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'head', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.head : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'body', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.body : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'forms', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.forms : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'images', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.images : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'links', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.links : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'scripts', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.scripts : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'children', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.children : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'childNodes', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.childNodes : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'firstChild', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.firstChild : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'lastChild', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.lastChild : null; }},
        configurable: true
    }});
    globalThis.Document.prototype.appendChild = function(c) {{ return (typeof document !== 'undefined' && document.appendChild) ? document.appendChild(c) : c; }};
    globalThis.Document.prototype.removeChild = function(c) {{ return (typeof document !== 'undefined' && document.removeChild) ? document.removeChild(c) : c; }};
    globalThis.Document.prototype.insertBefore = function(n, r) {{ return (typeof document !== 'undefined' && document.insertBefore) ? document.insertBefore(n, r) : n; }};
    globalThis.Document.prototype.replaceChild = function(n, o) {{ return (typeof document !== 'undefined' && document.replaceChild) ? document.replaceChild(n, o) : o; }};
    globalThis.HTMLDocument = globalThis.Document;

    // window / Window prototype
    globalThis.Window = function() {{}};
    globalThis.Window.prototype = Object.create(globalThis.EventTarget.prototype);
    try {{ Object.setPrototypeOf(globalThis, globalThis.Window.prototype); }} catch(e) {{}}
    var _winScrollX = 0;
    var _winScrollY = 0;

    function _setWinScroll(x, y, suppressEvent) {{
        var newX = Math.max(0, Number(x) || 0);
        var newY = Math.max(0, Number(y) || 0);
        var changed = (newX !== _winScrollX || newY !== _winScrollY);
        _winScrollX = newX;
        _winScrollY = newY;
        if (typeof document !== 'undefined') {{
            if (document.documentElement) {{
                document.documentElement._scrollLeft = newX;
                document.documentElement._scrollTop = newY;
            }}
            if (document.body) {{
                document.body._scrollLeft = newX;
                document.body._scrollTop = newY;
            }}
        }}
        if (changed && !suppressEvent) {{
            if (typeof globalThis._dispatchInternalEvent === 'function') {{
                try {{ globalThis._dispatchInternalEvent(-1, 'scroll', {{ bubbles: false, cancelable: false, eventType: 'ui' }}); }} catch(e) {{}}
            }} else if (typeof globalThis.dispatchEvent === 'function') {{
                try {{
                    var ev = (typeof globalThis.Event === 'function') ? new globalThis.Event('scroll') : {{ type: 'scroll' }};
                    globalThis.dispatchEvent(ev);
                }} catch(e) {{}}
            }}
        }}
    }}

    Object.defineProperty(globalThis, 'scrollX', {{
        get: function() {{ return _winScrollX; }},
        set: function(v) {{ _setWinScroll(v, _winScrollY, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'scrollY', {{
        get: function() {{ return _winScrollY; }},
        set: function(v) {{ _setWinScroll(_winScrollX, v, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'pageXOffset', {{
        get: function() {{ return _winScrollX; }},
        set: function(v) {{ _setWinScroll(v, _winScrollY, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'pageYOffset', {{
        get: function() {{ return _winScrollY; }},
        set: function(v) {{ _setWinScroll(_winScrollX, v, true); }},
        configurable: true
    }});

    function _animateWinScroll(targetX, targetY) {{
        var startX = _winScrollX;
        var startY = _winScrollY;
        var startTime = Date.now();
        var duration = 300;
        function step() {{
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            var curX = startX + (targetX - startX) * ease;
            var curY = startY + (targetY - startY) * ease;
            _setWinScroll(curX, curY, false);
            if (progress < 1.0) {{
                if (typeof globalThis.requestAnimationFrame === 'function') {{
                    globalThis.requestAnimationFrame(step);
                }} else if (typeof setTimeout === 'function') {{
                    setTimeout(step, 16);
                }}
            }}
        }}
        step();
    }}

    globalThis.scrollTo = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var tx = (x.left !== undefined) ? x.left : _winScrollX;
            var ty = (x.top !== undefined) ? x.top : _winScrollY;
            if (x.behavior === 'smooth') {{
                _animateWinScroll(tx, ty);
            }} else {{
                _setWinScroll(tx, ty, false);
            }}
        }} else {{
            var tx = (x !== undefined) ? x : _winScrollX;
            var ty = (y !== undefined) ? y : _winScrollY;
            _setWinScroll(tx, ty, false);
        }}
    }};
    globalThis.scrollBy = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var dx = (x.left !== undefined) ? x.left : 0;
            var dy = (x.top !== undefined) ? x.top : 0;
            if (x.behavior === 'smooth') {{
                _animateWinScroll(_winScrollX + dx, _winScrollY + dy);
            }} else {{
                _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
            }}
        }} else {{
            var dx = (x !== undefined) ? x : 0;
            var dy = (y !== undefined) ? y : 0;
            _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
        }}
    }};
    globalThis.scroll = globalThis.scrollTo;

    if (globalThis.window) {{
        globalThis.window.scrollTo = globalThis.scrollTo;
        globalThis.window.scrollBy = globalThis.scrollBy;
        globalThis.window.scroll = globalThis.scroll;
        try {{
            Object.defineProperty(globalThis.window, 'scrollX', {{ get: function() {{ return _winScrollX; }}, set: function(v) {{ _setWinScroll(v, _winScrollY, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'scrollY', {{ get: function() {{ return _winScrollY; }}, set: function(v) {{ _setWinScroll(_winScrollX, v, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'pageXOffset', {{ get: function() {{ return _winScrollX; }}, set: function(v) {{ _setWinScroll(v, _winScrollY, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'pageYOffset', {{ get: function() {{ return _winScrollY; }}, set: function(v) {{ _setWinScroll(_winScrollX, v, true); }}, configurable: true }});
        }} catch(e) {{}}
    }}
    globalThis.window.devicePixelRatio = 1;
    globalThis.window.open      = function(url) {{ if (typeof _mangoNavigate === 'function') _mangoNavigate(String(url || '')); return null; }};
    globalThis.window.close     = function() {{}};
    globalThis.window.focus     = function() {{}};
    globalThis.window.blur      = function() {{}};
    globalThis.window.print     = function() {{}};
    globalThis.window.confirm   = function(msg) {{ return false; }};
    globalThis.window.prompt    = function(msg, def) {{ return def || null; }};
    globalThis.window.getSelection = function() {{ return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null; }};

    // Shady DOM compatibility shims (used by Polymer / Lit)
    ['addEventListener','removeEventListener','dispatchEvent'].forEach(function(m) {{
        var nativeProp = '__shady_native_' + m;
        Object.defineProperty(globalThis, nativeProp, {{
            get: function() {{ return this['_'+nativeProp] || globalThis[m]; }},
            set: function(v) {{ this['_'+nativeProp] = v; }},
            configurable: true
        }});
    }});

    // ── 24. Other DOM types ───────────────────────────────────────────────────
    globalThis.DocumentFragment = function() {{}};
    globalThis.DocumentFragment.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentFragment.prototype.append = globalThis.Element.prototype.append;
    globalThis.DocumentFragment.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.DocumentFragment.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;

    globalThis.Document.prototype.append = globalThis.Element.prototype.append;
    globalThis.Document.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.Document.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;

    globalThis.ShadowRoot = function() {{}};
    globalThis.ShadowRoot.prototype = Object.create(globalThis.DocumentFragment.prototype);
    globalThis.DOMImplementation = function() {{}};
    globalThis.CharacterData = function() {{}};
    globalThis.CharacterData.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Text = function() {{}};
    globalThis.Text.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Comment = function() {{}};
    globalThis.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.CDATASection = function() {{}};
    globalThis.CDATASection.prototype = Object.create(globalThis.Text.prototype);
    globalThis.ProcessingInstruction = function() {{}};
    globalThis.ProcessingInstruction.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.DocumentType = function() {{}};
    globalThis.DocumentType.prototype = Object.create(globalThis.Node.prototype);
    globalThis.XMLDocument = function() {{}};
    globalThis.XMLDocument.prototype = Object.create(globalThis.Document.prototype);
    globalThis.Attr = function() {{}};
    globalThis.HTMLCollection = function() {{}};
    globalThis.HTMLCollection.prototype = Object.create(Object.prototype);
    globalThis.HTMLCollection.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    globalThis.HTMLCollection.prototype.namedItem = function(name) {{
        var str = String(name);
        for (var i = 0; i < this.length; i++) {{
            var el = this[i];
            if (el && (el.id === str || (el.getAttribute && el.getAttribute('name') === str))) {{
                return el;
            }}
        }}
        return null;
    }};
    globalThis.HTMLCollection.prototype[Symbol.iterator] = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: self[i++], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }}
        }};
    }};
    globalThis.HTMLCollection.prototype[Symbol.toStringTag] = 'HTMLCollection';

    globalThis.NodeList = function() {{}};
    globalThis.NodeList.prototype = Object.create(Object.prototype);
    globalThis.NodeList.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    globalThis.NodeList.prototype.forEach = function(cb, thisArg) {{
        for (var i = 0; i < this.length; i++) {{
            cb.call(thisArg, this[i], i, this);
        }}
    }};
    globalThis.NodeList.prototype.entries = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: [i, self[i++]], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype.keys = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: i++, done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype.values = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: self[i++], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype[Symbol.iterator] = function() {{
        return this.values();
    }};
    globalThis.NodeList.prototype[Symbol.toStringTag] = 'NodeList';
    globalThis.NodeFilter = {{
        FILTER_ACCEPT:1, FILTER_REJECT:2, FILTER_SKIP:3,
        SHOW_ALL:-1, SHOW_ELEMENT:1, SHOW_ATTRIBUTE:2, SHOW_TEXT:4,
        SHOW_COMMENT:128, SHOW_DOCUMENT:256, SHOW_DOCUMENT_FRAGMENT:1024
    }};
    function DOMRectReadOnly(x, y, width, height) {{
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }}
    Object.defineProperties(DOMRectReadOnly.prototype, {{
        top:    {{ get: function() {{ return this.y; }}, enumerable: true }},
        left:   {{ get: function() {{ return this.x; }}, enumerable: true }},
        right:  {{ get: function() {{ return this.x + this.width; }}, enumerable: true }},
        bottom: {{ get: function() {{ return this.y + this.height; }}, enumerable: true }}
    }});
    DOMRectReadOnly.prototype.toJSON = function() {{
        return {{ x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, left: this.left, right: this.right, bottom: this.bottom }};
    }};
    DOMRectReadOnly.fromRect = function(other) {{
        return new DOMRectReadOnly(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    }};
    DOMRectReadOnly.prototype[Symbol.toStringTag] = 'DOMRectReadOnly';
    globalThis.DOMRectReadOnly = DOMRectReadOnly;

    function DOMRect(x, y, width, height) {{
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }}
    DOMRect.prototype = Object.create(DOMRectReadOnly.prototype);
    DOMRect.prototype.constructor = DOMRect;
    DOMRect.fromRect = function(other) {{
        return new DOMRect(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    }};
    DOMRect.prototype[Symbol.toStringTag] = 'DOMRect';
    globalThis.DOMRect = DOMRect;

    function DOMRectList(rects) {{
        var arr = rects || [];
        this.length = arr.length;
        for (var i = 0; i < arr.length; i++) {{
            this[i] = arr[i];
        }}
    }}
    DOMRectList.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    DOMRectList.prototype[Symbol.iterator] = function() {{
        var self = this;
        var idx = 0;
        return {{
            next: function() {{
                if (idx < self.length) {{
                    return {{ value: self[idx++], done: false }};
                }} else {{
                    return {{ value: undefined, done: true }};
                }}
            }}
        }};
    }};
    DOMRectList.prototype[Symbol.toStringTag] = 'DOMRectList';
    globalThis.DOMRectList = DOMRectList;
    globalThis.DOMPoint = function(x,y,z,w) {{ this.x=x||0; this.y=y||0; this.z=z||0; this.w=w!==undefined?w:1; }};
    globalThis.DOMMatrix = function() {{
        this.a=1; this.b=0; this.c=0; this.d=1; this.e=0; this.f=0;
        this.is2D=true; this.isIdentity=true;
        this.transformPoint=function(p){{ return p; }};
        this.multiply=function(){{ return this; }};
        this.inverse=function(){{ return this; }};
    }};

    // ── 25. Image / Audio constructors ────────────────────────────────────────
    globalThis.Image = function(width, height) {{
        var img = (typeof document !== 'undefined' && document.createElement) ? document.createElement('img') : {{ tagName:'IMG', style:{{}} }};
        if (width  !== undefined) img.width  = width;
        if (height !== undefined) img.height = height;
        return img;
    }};
    globalThis.HTMLImageElement = globalThis.Image;
    globalThis.Audio = function(src) {{
        var audio = (typeof document !== 'undefined' && document.createElement) ? document.createElement('audio') : Object.create(globalThis.HTMLAudioElement.prototype);
        if (src !== undefined) audio.src = src;
        return audio;
    }};
    globalThis.Audio.prototype = globalThis.HTMLAudioElement.prototype;

    // ── 26. Observers ─────────────────────────────────────────────────────────
    function MutationRecord(type, target) {{
        this.type = type;
        this.target = target;
        this.addedNodes = [];
        this.removedNodes = [];
        this.previousSibling = null;
        this.nextSibling = null;
        this.attributeName = null;
        this.attributeNamespace = null;
        this.oldValue = null;
    }}
    MutationRecord.prototype[Symbol.toStringTag] = 'MutationRecord';
    globalThis.MutationRecord = MutationRecord;

    var _activeMutationObservers = [];

    function MutationObserver(cb) {{
        if (typeof cb !== 'function') throw new TypeError("MutationObserver callback must be a function");
        this._cb = cb;
        this._observations = [];
        this._records = [];
        this._scheduled = false;
        _activeMutationObservers.push(this);
    }}
    MutationObserver.prototype.observe = function(target, options) {{
        if (!target) return;
        var opts = options || {{}};
        this._observations = this._observations.filter(function(o) {{ return o.target !== target; }});
        this._observations.push({{ target: target, options: opts }});
    }};
    MutationObserver.prototype.disconnect = function() {{
        this._observations = [];
        this._records = [];
        var self = this;
        _activeMutationObservers = _activeMutationObservers.filter(function(o) {{ return o !== self; }});
    }};
    MutationObserver.prototype.takeRecords = function() {{
        var recs = this._records.slice();
        this._records = [];
        return recs;
    }};
    MutationObserver.prototype._queueRecord = function(record) {{
        this._records.push(record);
        if (this._scheduled) return;
        this._scheduled = true;
        var self = this;
        var deliver = function() {{
            self._scheduled = false;
            var records = self.takeRecords();
            if (records.length > 0) {{
                try {{
                    self._cb(records, self);
                }} catch(e) {{
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }}
            }}
        }};
        if (typeof queueMicrotask === 'function') {{
            queueMicrotask(deliver);
        }} else if (typeof Promise !== 'undefined') {{
            Promise.resolve().then(deliver);
        }} else if (typeof setTimeout !== 'undefined') {{
            setTimeout(deliver, 0);
        }} else {{
            deliver();
        }}
    }};
    MutationObserver.prototype[Symbol.toStringTag] = 'MutationObserver';
    globalThis.MutationObserver = MutationObserver;

    globalThis._reportDOMMutation = function(detail) {{
        if (_activeMutationObservers.length === 0) return;
        for (var i = 0; i < _activeMutationObservers.length; i++) {{
            var obs = _activeMutationObservers[i];
            for (var j = 0; j < obs._observations.length; j++) {{
                var ob = obs._observations[j];
                var matchesTarget = (ob.target === detail.target);
                if (!matchesTarget && ob.options.subtree && ob.target.contains) {{
                    matchesTarget = ob.target.contains(detail.target);
                }}
                if (!matchesTarget) continue;

                var opt = ob.options;
                if (detail.type === 'childList' && opt.childList) {{
                    var rec = new MutationRecord('childList', detail.target);
                    rec.addedNodes = detail.addedNodes || [];
                    rec.removedNodes = detail.removedNodes || [];
                    rec.previousSibling = detail.previousSibling || null;
                    rec.nextSibling = detail.nextSibling || null;
                    obs._queueRecord(rec);
                }} else if (detail.type === 'attributes' && (opt.attributes || opt.attributeFilter || opt.attributeOldValue)) {{
                    if (opt.attributeFilter && Array.isArray(opt.attributeFilter) && opt.attributeFilter.indexOf(detail.attributeName) === -1) {{
                        continue;
                    }}
                    var rec = new MutationRecord('attributes', detail.target);
                    rec.attributeName = detail.attributeName;
                    rec.attributeNamespace = detail.attributeNamespace || null;
                    if (opt.attributeOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                }} else if (detail.type === 'characterData' && (opt.characterData || opt.characterDataOldValue)) {{
                    var rec = new MutationRecord('characterData', detail.target);
                    if (opt.characterDataOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                }}
            }}
        }}
    }};

    function IntersectionObserverEntry(target, rootBounds, boundingClientRect, intersectionRect, isIntersecting, intersectionRatio, time) {{
        this.target = target;
        this.rootBounds = rootBounds;
        this.boundingClientRect = boundingClientRect;
        this.intersectionRect = intersectionRect;
        this.isIntersecting = isIntersecting;
        this.intersectionRatio = intersectionRatio;
        this.time = time || (typeof performance !== 'undefined' && performance.now ? performance.now() : Date.now());
        this.isVisible = false;
    }}
    IntersectionObserverEntry.prototype[Symbol.toStringTag] = 'IntersectionObserverEntry';
    globalThis.IntersectionObserverEntry = IntersectionObserverEntry;

    var _activeIntersectionObservers = [];

    function IntersectionObserver(cb, options) {{
        if (typeof cb !== 'function') throw new TypeError("IntersectionObserver callback must be a function");
        this._cb = cb;
        this._options = options || {{}};
        this.root = this._options.root || null;
        this.rootMargin = this._options.rootMargin || "0px 0px 0px 0px";
        this.thresholds = Array.isArray(this._options.threshold) ? this._options.threshold : [this._options.threshold !== undefined ? Number(this._options.threshold) : 0];
        this._targets = [];
        this._records = [];
        this._scheduled = false;
        _activeIntersectionObservers.push(this);
    }}
    IntersectionObserver.prototype.observe = function(target) {{
        if (!target) return;
        if (this._targets.indexOf(target) === -1) {{
            this._targets.push(target);
        }}
        this._computeAndSchedule();
    }};
    IntersectionObserver.prototype.unobserve = function(target) {{
        this._targets = this._targets.filter(function(t) {{ return t !== target; }});
    }};
    IntersectionObserver.prototype.disconnect = function() {{
        this._targets = [];
        this._records = [];
        var self = this;
        _activeIntersectionObservers = _activeIntersectionObservers.filter(function(o) {{ return o !== self; }});
    }};
    IntersectionObserver.prototype.takeRecords = function() {{
        var recs = this._records.slice();
        this._records = [];
        return recs;
    }};
    IntersectionObserver.prototype._computeAndSchedule = function() {{
        var self = this;
        var rootRect;
        if (self.root && typeof self.root.getBoundingClientRect === 'function') {{
            rootRect = self.root.getBoundingClientRect();
        }} else {{
            var vw = (typeof window !== 'undefined' && window.innerWidth) || 800;
            var vh = (typeof window !== 'undefined' && window.innerHeight) || 600;
            rootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(0, 0, vw, vh) : {{ x:0, y:0, top:0, left:0, width:vw, height:vh, right:vw, bottom:vh }};
        }}

        var mTop = 0, mRight = 0, mBottom = 0, mLeft = 0;
        if (self.rootMargin) {{
            var parts = String(self.rootMargin).trim().split(/\s+/).map(function(p) {{ return parseFloat(p) || 0; }});
            if (parts.length === 1) {{ mTop = mRight = mBottom = mLeft = parts[0]; }}
            else if (parts.length === 2) {{ mTop = mBottom = parts[0]; mRight = mLeft = parts[1]; }}
            else if (parts.length === 4) {{ mTop = parts[0]; mRight = parts[1]; mBottom = parts[2]; mLeft = parts[3]; }}
        }}
        var rLeft = rootRect.left - mLeft;
        var rTop = rootRect.top - mTop;
        var rRight = rootRect.right + mRight;
        var rBottom = rootRect.bottom + mBottom;
        var rWidth = rRight - rLeft;
        var rHeight = rBottom - rTop;
        var expandedRootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(rLeft, rTop, rWidth, rHeight) : {{ x:rLeft, y:rTop, top:rTop, left:rLeft, width:rWidth, height:rHeight, right:rRight, bottom:rBottom }};

        for (var i = 0; i < self._targets.length; i++) {{
            var target = self._targets[i];
            var bRect = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : {{ x:0, y:0, top:0, left:0, width:0, height:0, right:0, bottom:0 }};
            var iLeft = Math.max(bRect.left, rLeft);
            var iTop = Math.max(bRect.top, rTop);
            var iRight = Math.min(bRect.right, rRight);
            var iBottom = Math.min(bRect.bottom, rBottom);

            var isIntersecting = (iRight > iLeft && iBottom > iTop) ||
                (bRect.width === 0 && bRect.height === 0 && bRect.left >= rLeft && bRect.right <= rRight && bRect.top >= rTop && bRect.bottom <= rBottom);
            var iWidth = isIntersecting ? Math.max(0, iRight - iLeft) : 0;
            var iHeight = isIntersecting ? Math.max(0, iBottom - iTop) : 0;
            var intersectRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(isIntersecting ? iLeft : 0, isIntersecting ? iTop : 0, iWidth, iHeight) : {{ x: isIntersecting ? iLeft : 0, y: isIntersecting ? iTop : 0, top: isIntersecting ? iTop : 0, left: isIntersecting ? iLeft : 0, width: iWidth, height: iHeight, right: isIntersecting ? iRight : 0, bottom: isIntersecting ? iBottom : 0 }};

            var targetArea = (bRect.width || 0) * (bRect.height || 0);
            var intersectArea = iWidth * iHeight;
            var ratio = 0;
            if (targetArea > 0) {{
                ratio = intersectArea / targetArea;
            }} else if (isIntersecting) {{
                ratio = 1;
            }}

            var entry = new IntersectionObserverEntry(target, expandedRootRect, bRect, intersectRect, isIntersecting, ratio);
            self._records.push(entry);
        }}

        if (!self._scheduled) {{
            self._scheduled = true;
            var deliver = function() {{
                self._scheduled = false;
                var records = self.takeRecords();
                if (records.length > 0) {{
                    try {{
                        self._cb(records, self);
                    }} catch(e) {{
                        if (typeof console !== 'undefined' && console.error) console.error(e);
                    }}
                }}
            }};
            if (typeof queueMicrotask === 'function') {{
                queueMicrotask(deliver);
            }} else if (typeof Promise !== 'undefined') {{
                Promise.resolve().then(deliver);
            }} else if (typeof setTimeout !== 'undefined') {{
                setTimeout(deliver, 0);
            }} else {{
                deliver();
            }}
        }}
    }};
    IntersectionObserver.prototype[Symbol.toStringTag] = 'IntersectionObserver';
    globalThis.IntersectionObserver = IntersectionObserver;

    // ── Lazy Loading with IntersectionObserver (loading="lazy") ────────────────
    var _lazyLoadObserver = null;
    function _getLazyObserver() {{
        if (!_lazyLoadObserver && typeof IntersectionObserver === 'function') {{
            _lazyLoadObserver = new IntersectionObserver(function(entries) {{
                entries.forEach(function(entry) {{
                    if (entry.isIntersecting) {{
                        var el = entry.target;
                        if (el) {{
                            var dataSrc = el.getAttribute ? el.getAttribute('data-src') : null;
                            if (dataSrc) {{
                                if (el.setAttribute) el.setAttribute('src', dataSrc);
                                else el.src = dataSrc;
                                if (el.removeAttribute) el.removeAttribute('data-src');
                            }}
                            el._lazyLoaded = true;
                            if (_lazyLoadObserver) _lazyLoadObserver.unobserve(el);
                        }}
                    }}
                }});
            }}, {{ rootMargin: '200px 0px' }});
        }}
        return _lazyLoadObserver;
    }}

    function _setupLazyLoadingProperty(proto) {{
        if (!proto) return;
        Object.defineProperty(proto, 'loading', {{
            get: function() {{ return this.getAttribute ? (this.getAttribute('loading') || 'eager') : 'eager'; }},
            set: function(val) {{
                var s = String(val).toLowerCase();
                if (this.setAttribute) this.setAttribute('loading', s);
                if (s === 'lazy') {{
                    var obs = _getLazyObserver();
                    if (obs) obs.observe(this);
                }}
            }},
            configurable: true
        }});
    }}
    _setupLazyLoadingProperty(globalThis.HTMLElement ? globalThis.HTMLElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLImageElement ? globalThis.HTMLImageElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLIFrameElement ? globalThis.HTMLIFrameElement.prototype : null);

    function ResizeObserverEntry(target) {{
        this.target = target;
        var r = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : {{ x:0, y:0, width:0, height:0 }};
        var w = r.width || 0;
        var h = r.height || 0;
        this.contentRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(r.x, r.y, w, h) : r;
        this.borderBoxSize = [{{ inlineSize: w, blockSize: h }}];
        this.contentBoxSize = [{{ inlineSize: w, blockSize: h }}];
        this.devicePixelContentBoxSize = [{{ inlineSize: w, blockSize: h }}];
    }}
    ResizeObserverEntry.prototype[Symbol.toStringTag] = 'ResizeObserverEntry';
    globalThis.ResizeObserverEntry = ResizeObserverEntry;

    var _activeResizeObservers = [];

    function ResizeObserver(cb) {{
        if (typeof cb !== 'function') throw new TypeError("ResizeObserver callback must be a function");
        this._cb = cb;
        this._targets = [];
        this._scheduled = false;
        _activeResizeObservers.push(this);
    }}
    ResizeObserver.prototype.observe = function(target, options) {{
        if (!target) return;
        var idx = this._targets.findIndex(function(t) {{ return t.target === target; }});
        if (idx === -1) {{
            this._targets.push({{ target: target, options: options || {{}} }});
        }}
        this._scheduleDelivery();
    }};
    ResizeObserver.prototype.unobserve = function(target) {{
        this._targets = this._targets.filter(function(t) {{ return t.target !== target; }});
    }};
    ResizeObserver.prototype.disconnect = function() {{
        this._targets = [];
        var self = this;
        _activeResizeObservers = _activeResizeObservers.filter(function(o) {{ return o !== self; }});
    }};
    ResizeObserver.prototype._scheduleDelivery = function() {{
        if (this._scheduled) return;
        this._scheduled = true;
        var self = this;
        var deliver = function() {{
            self._scheduled = false;
            if (self._targets.length === 0) return;
            var entries = self._targets.map(function(t) {{
                return new ResizeObserverEntry(t.target);
            }});
            try {{
                self._cb(entries, self);
            }} catch(e) {{
                if (typeof console !== 'undefined' && console.error) console.error(e);
            }}
        }};
        if (typeof queueMicrotask === 'function') {{
            queueMicrotask(deliver);
        }} else if (typeof Promise !== 'undefined') {{
            Promise.resolve().then(deliver);
        }} else if (typeof setTimeout !== 'undefined') {{
            setTimeout(deliver, 0);
        }} else {{
            deliver();
        }}
    }};
    ResizeObserver.prototype[Symbol.toStringTag] = 'ResizeObserver';
    globalThis.ResizeObserver = ResizeObserver;
    globalThis.PerformanceObserver = function(cb) {{
        this._cb = cb;
        this.observe    = function() {{}};
        this.disconnect = function() {{}};
        this.takeRecords = function() {{ return []; }};
    }};

    // ── 27. Event classes ─────────────────────────────────────────────────────
    globalThis.Event = function(type, dict) {{
        this.type              = String(type);
        this.bubbles           = !!(dict && dict.bubbles);
        this.cancelable        = (dict && dict.cancelable !== undefined) ? !!dict.cancelable : true;
        this.composed          = !!(dict && dict.composed);
        this.defaultPrevented  = false;
        this.cancelBubble      = false;
        this.timeStamp         = Date.now();
        this.eventPhase        = 0;
        this.isTrusted         = false;
        this.target            = null;
        this.currentTarget     = null;
        this._stopped          = false;
        this._stopImmediate    = false;
        this._currentPassive   = false;
        this.preventDefault    = function() {{
            if (!this._currentPassive && this.cancelable) {{
                this.defaultPrevented = true;
            }}
        }};
        this.stopPropagation   = function() {{
            this._stopped = true;
            this.cancelBubble = true;
        }};
        this.stopImmediatePropagation = function() {{
            this._stopped = true;
            this._stopImmediate = true;
            this.cancelBubble = true;
        }};
        this.composedPath      = function() {{ return []; }};
    }};
    globalThis.Event.NONE = 0;
    globalThis.Event.CAPTURING_PHASE = 1;
    globalThis.Event.AT_TARGET = 2;
    globalThis.Event.BUBBLING_PHASE = 3;
    globalThis.Event.prototype.preventDefault = function() {{
        if (!this._currentPassive && this.cancelable) {{
            this.defaultPrevented = true;
        }}
    }};
    globalThis.Event.prototype.stopPropagation = function() {{
        this._stopped = true;
        this.cancelBubble = true;
    }};
    globalThis.Event.prototype.stopImmediatePropagation = function() {{
        this._stopped = true;
        this._stopImmediate = true;
        this.cancelBubble = true;
    }};
    globalThis.Event.prototype.composedPath = function() {{ return []; }};

    globalThis.UIEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.view   = (dict && dict.view)   || globalThis.window;
        this.detail = (dict && dict.detail) || 0;
    }};
    globalThis.UIEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.MouseEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.clientX  = dict.clientX  || 0; this.clientY  = dict.clientY  || 0;
        this.screenX  = dict.screenX  || 0; this.screenY  = dict.screenY  || 0;
        this.pageX    = dict.pageX    || 0; this.pageY    = dict.pageY    || 0;
        this.offsetX  = dict.offsetX  || 0; this.offsetY  = dict.offsetY  || 0;
        this.button   = dict.button   || 0; this.buttons  = dict.buttons  || 0;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.relatedTarget = dict.relatedTarget || null;
        this.getModifierState = function(key) {{ return false; }};
    }};
    globalThis.MouseEvent.prototype = Object.create(globalThis.UIEvent.prototype);

    globalThis.PointerEvent = function(type, dict) {{
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {{}};
        this.pointerId   = dict.pointerId   || 0;
        this.pointerType = dict.pointerType || 'mouse';
        this.isPrimary   = dict.isPrimary   !== false;
        this.pressure    = dict.pressure    || 0;
        this.width       = dict.width       || 1;
        this.height      = dict.height      || 1;
    }};
    globalThis.PointerEvent.prototype = Object.create(globalThis.MouseEvent.prototype);

    globalThis.KeyboardEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.key      = dict.key      || '';   this.code     = dict.code     || '';
        this.keyCode  = dict.keyCode  || 0;    this.charCode = dict.charCode || 0;
        this.which    = dict.which    || 0;    this.location = dict.location || 0;
        this.repeat   = !!dict.repeat;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.getModifierState = function(k) {{ return false; }};
    }};
    globalThis.KeyboardEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_STANDARD = 0;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_LEFT     = 1;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_RIGHT    = 2;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_NUMPAD   = 3;

    globalThis.FocusEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        this.relatedTarget = (dict && dict.relatedTarget) || null;
    }};
    globalThis.FocusEvent.prototype = Object.create(globalThis.UIEvent.prototype);

    globalThis.InputEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.data           = dict.data           || null;
        this.inputType      = dict.inputType      || '';
        this.isComposing    = !!dict.isComposing;
        this.dataTransfer   = dict.dataTransfer   || null;
    }};
    globalThis.InputEvent.prototype = Object.create(globalThis.UIEvent.prototype);

    globalThis.WheelEvent = function(type, dict) {{
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {{}};
        this.deltaX    = dict.deltaX    || 0;
        this.deltaY    = dict.deltaY    || 0;
        this.deltaZ    = dict.deltaZ    || 0;
        this.deltaMode = dict.deltaMode || 0;
    }};
    globalThis.WheelEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.WheelEvent.DOM_DELTA_PIXEL = 0;
    globalThis.WheelEvent.DOM_DELTA_LINE  = 1;
    globalThis.WheelEvent.DOM_DELTA_PAGE  = 2;

    globalThis.TouchEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.touches        = dict.touches        || [];
        this.targetTouches  = dict.targetTouches  || [];
        this.changedTouches = dict.changedTouches || [];
    }};
    globalThis.TouchEvent.prototype = Object.create(globalThis.UIEvent.prototype);

    globalThis.CustomEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.detail = (dict && dict.detail !== undefined) ? dict.detail : null;
        this.initCustomEvent = function(t, b, c, d) {{ this.type=t; this.bubbles=b; this.cancelable=c; this.detail=d; }};
    }};
    globalThis.CustomEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.HashChangeEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.oldURL = (dict && dict.oldURL) || '';
        this.newURL = (dict && dict.newURL) || '';
    }};
    globalThis.HashChangeEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.PopStateEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.state = (dict && dict.state) || null;
    }};
    globalThis.PopStateEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.ErrorEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.message  = dict.message  || '';
        this.filename = dict.filename || '';
        this.lineno   = dict.lineno   || 0;
        this.colno    = dict.colno    || 0;
        this.error    = dict.error    || null;
    }};
    globalThis.ErrorEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.StorageEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.key         = dict.key         || null;
        this.oldValue    = dict.oldValue    || null;
        this.newValue    = dict.newValue    || null;
        this.url         = dict.url         || '';
        this.storageArea = dict.storageArea || null;
    }};
    globalThis.StorageEvent.prototype = Object.create(globalThis.Event.prototype);

    // TransitionEvent / AnimationEvent (for CSS animation callbacks)
    globalThis.TransitionEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.propertyName = dict.propertyName || '';
        this.elapsedTime  = dict.elapsedTime  || 0;
        this.pseudoElement= dict.pseudoElement|| '';
    }};
    globalThis.TransitionEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.AnimationEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.animationName = dict.animationName || '';
        this.elapsedTime   = dict.elapsedTime   || 0;
        this.pseudoElement = dict.pseudoElement || '';
    }};
    globalThis.AnimationEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.SubmitEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.submitter = dict.submitter || null;
    }};
    globalThis.SubmitEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.FormDataEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.formData = dict.formData || null;
    }};
    globalThis.FormDataEvent.prototype = Object.create(globalThis.Event.prototype);

    globalThis.PageTransitionEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.persisted = !!dict.persisted;
    }};
    globalThis.PageTransitionEvent.prototype = Object.create(globalThis.Event.prototype);

    // ── 28. Web Components ────────────────────────────────────────────────────
    globalThis.WebComponents = {{ ready: true }};
    globalThis.CustomElementRegistry = function() {{ this._registry = {{}}; }};
    globalThis.CustomElementRegistry.prototype.define = function(name, ctor, opts) {{
        name = String(name).toLowerCase();
        this._registry[name] = ctor;
        // Run upgrades for any already-parsed elements with this tag name
        if (typeof document !== 'undefined' && document.querySelectorAll) {{
            try {{
                var els = document.querySelectorAll(name);
                for (var i = 0; i < els.length; i++) {{
                    var el = els[i];
                    if (ctor && ctor.prototype) Object.setPrototypeOf(el, ctor.prototype);
                    if (typeof el.connectedCallback === 'function') {{
                        try {{ el.connectedCallback(); }} catch(e) {{}}
                    }}
                }}
            }} catch(e) {{}}
        }}
    }};
    globalThis.CustomElementRegistry.prototype.get          = function(n) {{ return this._registry[n]; }};
    globalThis.CustomElementRegistry.prototype.upgrade      = function(root) {{}};
    globalThis.CustomElementRegistry.prototype.whenDefined   = function(n) {{ return Promise.resolve(); }};
    globalThis.CustomElementRegistry.prototype.getName       = function(ctor) {{ return Object.keys(this._registry).find(function(k) {{ return this._registry[k] === ctor; }}, this) || null; }};
    if (!globalThis.customElements) {{
        globalThis.customElements = new globalThis.CustomElementRegistry();
    }}

    // ── 29. Blob / File / FileReader ──────────────────────────────────────────
    // (Full standard implementations of Blob, File, and FileReader are defined above in section 19)

    // ── 30. Promise (ensure existence — Boa has built-in Promise) ────────────
    if (typeof Promise === 'undefined') {{
        // Minimal synchronous Promise polyfill as last-resort fallback
        globalThis.Promise = function(executor) {{
            this._state = 'pending'; this._val = undefined; this._cb = [];
            var self = this;
            function resolve(v) {{ if (self._state !== 'pending') return; self._state = 'fulfilled'; self._val = v; self._cb.forEach(function(c) {{ if (c.onFulfilled) try {{ c.onFulfilled(v); }} catch(e) {{}} }}); }}
            function reject(r)  {{ if (self._state !== 'pending') return; self._state = 'rejected';  self._val = r; self._cb.forEach(function(c) {{ if (c.onRejected)  try {{ c.onRejected(r);  }} catch(e) {{}} }}); }}
            try {{ executor(resolve, reject); }} catch(e) {{ reject(e); }}
        }};
        globalThis.Promise.prototype.then = function(onF, onR) {{
            var self = this;
            return new Promise(function(res, rej) {{
                self._cb.push({{ onFulfilled: onF ? function(v) {{ try {{ res(onF(v)); }} catch(e) {{ rej(e); }} }} : res, onRejected: onR ? function(r) {{ try {{ res(onR(r)); }} catch(e) {{ rej(e); }} }} : rej }});
                if (self._state === 'fulfilled' && onF) try {{ res(onF(self._val)); }} catch(e) {{ rej(e); }}
                if (self._state === 'rejected'  && onR) try {{ res(onR(self._val)); }} catch(e) {{ rej(e); }}
            }});
        }};
        globalThis.Promise.prototype.catch  = function(fn) {{ return this.then(undefined, fn); }};
        globalThis.Promise.prototype.finally= function(fn) {{ return this.then(function(v){{ fn(); return v; }}, function(r){{ fn(); throw r; }}); }};
        globalThis.Promise.resolve = function(v) {{ return new Promise(function(res) {{ res(v); }}); }};
        globalThis.Promise.reject  = function(r) {{ return new Promise(function(_, rej) {{ rej(r); }}); }};
        globalThis.Promise.all     = function(ps) {{ return new Promise(function(res, rej) {{ if (!ps.length) {{ res([]); return; }} var out=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(function(v){{ out[i]=v; if(++done===ps.length) res(out); }},rej); }}); }}); }};
        globalThis.Promise.allSettled = function(ps) {{ return new Promise(function(res) {{ if (!ps.length) {{ res([]); return; }} var out=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(function(v){{ out[i]={{status:'fulfilled',value:v}};  if(++done===ps.length) res(out); }},function(r){{ out[i]={{status:'rejected',reason:r}}; if(++done===ps.length) res(out); }}); }}); }}); }};
        globalThis.Promise.race    = function(ps) {{ return new Promise(function(res,rej) {{ ps.forEach(function(p){{ Promise.resolve(p).then(res,rej); }}); }}); }};
        globalThis.Promise.any     = function(ps) {{ return new Promise(function(res,rej) {{ var errs=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(res,function(r){{ errs[i]=r; if(++done===ps.length) rej(new Error('All promises rejected')); }}); }}); }}); }};
    }}

    // ── 31. queueMicrotask ────────────────────────────────────────────────────
    globalThis.queueMicrotask = function(fn) {{
        Promise.resolve().then(function() {{ try {{ fn(); }} catch(e) {{}} }});
    }};

    // ── 32. Web Worker & Service Worker (GAP-015) ──────────────────────────
    function Worker(scriptURL, options) {{
        this.scriptURL = String(scriptURL);
        this.options = options || {{}};
        this.onmessage = null;
        this.onerror = null;
        this.onmessageerror = null;
        this._listeners = {{}};
        this._terminated = false;

        var self = this;
        this.addEventListener = function(type, fn) {{
            if (!self._listeners[type]) self._listeners[type] = [];
            self._listeners[type].push(fn);
        }};
        this.removeEventListener = function(type, fn) {{
            if (!self._listeners[type]) return;
            self._listeners[type] = self._listeners[type].filter(function(f) {{ return f !== fn; }});
        }};
        this.dispatchEvent = function(evt) {{
            var type = evt.type;
            if (type === 'message' && typeof self.onmessage === 'function') self.onmessage(evt);
            if (type === 'error' && typeof self.onerror === 'function') self.onerror(evt);
            var list = self._listeners[type] || [];
            for (var i = 0; i < list.length; i++) {{
                try {{ list[i].call(self, evt); }} catch(e) {{}}
            }}
            return !evt.defaultPrevented;
        }};
        this.postMessage = function(message, transfer) {{
            if (self._terminated) return;
            setTimeout(function() {{
                if (self._terminated) return;
                self.dispatchEvent({{ type: 'message', data: message, origin: '', lastEventId: '', source: self }});
            }}, 0);
        }};
        this.terminate = function() {{
            self._terminated = true;
        }};
    }}
    globalThis.Worker = Worker;

    // ServiceWorker interfaces
    function ServiceWorker(scriptURL) {{
        this.scriptURL = scriptURL || '';
        this.state = 'activated';
        this.onstatechange = null;
        this.onerror = null;
        this.postMessage = function(msg) {{}};
    }}
    globalThis.ServiceWorker = ServiceWorker;

    function ServiceWorkerRegistration(scope, sw) {{
        this.scope = scope || '/';
        this.installing = null;
        this.waiting = null;
        this.active = sw || new ServiceWorker();
        this.update = function() {{ return Promise.resolve(this); }};
        this.unregister = function() {{ return Promise.resolve(true); }};
    }}
    globalThis.ServiceWorkerRegistration = ServiceWorkerRegistration;

    function ServiceWorkerContainer() {{
        var self = this;
        this.controller = new ServiceWorker('/');
        this.ready = Promise.resolve(new ServiceWorkerRegistration('/', self.controller));
        this.oncontrollerchange = null;
        this.onmessage = null;
        this.register = function(scriptURL, options) {{
            var scope = (options && options.scope) ? options.scope : '/';
            var reg = new ServiceWorkerRegistration(scope, new ServiceWorker(scriptURL));
            self.controller = reg.active;
            return Promise.resolve(reg);
        }};
        this.getRegistration = function(clientURL) {{
            return Promise.resolve(new ServiceWorkerRegistration('/', self.controller));
        }};
        this.getRegistrations = function() {{
            return Promise.resolve([new ServiceWorkerRegistration('/', self.controller)]);
        }};
        this.startMessages = function() {{}};
    }}
    globalThis.ServiceWorkerContainer = ServiceWorkerContainer;

    if (globalThis.navigator) {{
        globalThis.navigator.serviceWorker = new ServiceWorkerContainer();
    }}

    // ── 14. IndexedDB API (W3C Indexed Database API) ──────────────────────────
    function DOMStringList(arr) {{
        this._list = arr || [];
        for (var i = 0; i < this._list.length; i++) {{
            this[i] = this._list[i];
        }}
    }}
    DOMStringList.prototype.contains = function(str) {{
        return this._list.indexOf(str) !== -1;
    }};
    DOMStringList.prototype.item = function(index) {{
        return this._list[index] || null;
    }};
    Object.defineProperty(DOMStringList.prototype, 'length', {{
        get: function() {{ return this._list.length; }}
    }});
    globalThis.DOMStringList = DOMStringList;

    function IDBRequest() {{
        this.result = undefined;
        this.error = null;
        this.source = null;
        this.transaction = null;
        this.readyState = 'pending';
        this._onsuccess = null;
        this._onerror = null;
        this._listeners = {{}};
        this._successDispatched = false;
        this._errorDispatched = false;
    }}
    Object.defineProperty(IDBRequest.prototype, 'onsuccess', {{
        get: function() {{ return this._onsuccess; }},
        set: function(fn) {{
            this._onsuccess = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && !this.error) {{
                if (this._needsUpgrade && !this._upgradeDispatched) {{
                    if (typeof this._onupgradeneeded === 'function') {{
                        this._upgradeDispatched = true;
                        try {{ this._onupgradeneeded.call(this, this._upgradeEvent); }} catch (e) {{}}
                    }}
                }}
                if (!this._successDispatched) {{
                    this._successDispatched = true;
                    try {{ fn.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBRequest.prototype, 'onerror', {{
        get: function() {{ return this._onerror; }},
        set: function(fn) {{
            this._onerror = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && this.error) {{
                if (!this._errorDispatched) {{
                    this._errorDispatched = true;
                    try {{ fn.call(this, {{ type: 'error', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    IDBRequest.prototype.addEventListener = function(type, fn) {{
        if (!this._listeners[type]) this._listeners[type] = [];
        this._listeners[type].push(fn);
        if (type === 'upgradeneeded' && this._needsUpgrade && !this._upgradeDispatched) {{
            this._upgradeDispatched = true;
            try {{ fn.call(this, this._upgradeEvent); }} catch (e) {{}}
            if (typeof this._onsuccess === 'function' && !this._successDispatched) {{
                this._successDispatched = true;
                try {{ this._onsuccess.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
            }}
        }} else if (type === 'success' && this.readyState === 'done' && !this.error) {{
            if (!this._needsUpgrade || this._upgradeDispatched) {{
                try {{ fn.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
            }}
        }} else if (type === 'error' && this.readyState === 'done' && this.error) {{
            try {{ fn.call(this, {{ type: 'error', target: this }}); }} catch (e) {{}}
        }}
    }};
    IDBRequest.prototype.removeEventListener = function(type, fn) {{
        if (this._listeners[type]) {{
            this._listeners[type] = this._listeners[type].filter(function(f) {{ return f !== fn; }});
        }}
    }};
    IDBRequest.prototype.dispatchEvent = function(evt) {{
        var t = evt ? evt.type : 'success';
        if (t === 'success' && typeof this._onsuccess === 'function') {{
            try {{ this._onsuccess.call(this, evt); }} catch (e) {{}}
        }} else if (t === 'error' && typeof this._onerror === 'function') {{
            try {{ this._onerror.call(this, evt); }} catch (e) {{}}
        }} else if (t === 'upgradeneeded' && typeof this._onupgradeneeded === 'function') {{
            try {{ this._onupgradeneeded.call(this, evt); }} catch (e) {{}}
        }}
        if (this._listeners[t]) {{
            for (var i = 0; i < this._listeners[t].length; i++) {{
                try {{ this._listeners[t][i].call(this, evt); }} catch (e) {{}}
            }}
        }}
        return true;
    }};
    globalThis.IDBRequest = IDBRequest;

    function IDBOpenDBRequest() {{
        IDBRequest.call(this);
        this._onupgradeneeded = null;
        this._onblocked = null;
        this._needsUpgrade = false;
        this._upgradeDispatched = false;
        this._upgradeEvent = null;
    }}
    IDBOpenDBRequest.prototype = Object.create(IDBRequest.prototype);
    IDBOpenDBRequest.prototype.constructor = IDBOpenDBRequest;

    Object.defineProperty(IDBOpenDBRequest.prototype, 'onupgradeneeded', {{
        get: function() {{ return this._onupgradeneeded; }},
        set: function(fn) {{
            this._onupgradeneeded = fn;
            if (typeof fn === 'function' && this._needsUpgrade && !this._upgradeDispatched) {{
                this._upgradeDispatched = true;
                try {{ fn.call(this, this._upgradeEvent); }} catch (e) {{}}
                if (typeof this._onsuccess === 'function' && !this._successDispatched) {{
                    this._successDispatched = true;
                    try {{ this._onsuccess.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBOpenDBRequest.prototype, 'onblocked', {{
        get: function() {{ return this._onblocked; }},
        set: function(fn) {{ this._onblocked = fn; }},
        configurable: true,
        enumerable: true
    }});
    globalThis.IDBOpenDBRequest = IDBOpenDBRequest;

    function IDBVersionChangeEvent(type, dict) {{
        this.type = type;
        this.oldVersion = (dict && dict.oldVersion !== undefined) ? dict.oldVersion : 0;
        this.newVersion = (dict && dict.newVersion !== undefined) ? dict.newVersion : null;
        this.target = null;
    }}
    globalThis.IDBVersionChangeEvent = IDBVersionChangeEvent;

    function IDBKeyRange(lower, upper, lowerOpen, upperOpen) {{
        this.lower = lower;
        this.upper = upper;
        this.lowerOpen = !!lowerOpen;
        this.upperOpen = !!upperOpen;
    }}
    IDBKeyRange.only = function(val) {{
        return new IDBKeyRange(val, val, false, false);
    }};
    IDBKeyRange.lowerBound = function(lower, open) {{
        return new IDBKeyRange(lower, undefined, open, true);
    }};
    IDBKeyRange.upperBound = function(upper, open) {{
        return new IDBKeyRange(undefined, upper, true, open);
    }};
    IDBKeyRange.bound = function(lower, upper, lowerOpen, upperOpen) {{
        return new IDBKeyRange(lower, upper, lowerOpen, upperOpen);
    }};
    IDBKeyRange.prototype.includes = function(key) {{
        if (this.lower !== undefined) {{
            if (this.lowerOpen ? key <= this.lower : key < this.lower) return false;
        }}
        if (this.upper !== undefined) {{
            if (this.upperOpen ? key >= this.upper : key > this.upper) return false;
        }}
        return true;
    }};
    globalThis.IDBKeyRange = IDBKeyRange;

    function IDBTransaction(db, storeNames, mode) {{
        this.db = db;
        this.mode = mode || 'readonly';
        this.error = null;
        this._oncomplete = null;
        this._onerror = null;
        this._onabort = null;
        this._storeNames = Array.isArray(storeNames) ? storeNames : [storeNames];
        this._active = true;
        this._listeners = {{}};
        this._completeDispatched = false;
    }}
    Object.defineProperty(IDBTransaction.prototype, 'oncomplete', {{
        get: function() {{ return this._oncomplete; }},
        set: function(fn) {{
            this._oncomplete = fn;
            if (typeof fn === 'function' && !this._completeDispatched) {{
                this._completeDispatched = true;
                try {{ fn.call(this, {{ type: 'complete', target: this }}); }} catch (e) {{}}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBTransaction.prototype, 'onerror', {{
        get: function() {{ return this._onerror; }},
        set: function(fn) {{ this._onerror = fn; }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBTransaction.prototype, 'onabort', {{
        get: function() {{ return this._onabort; }},
        set: function(fn) {{ this._onabort = fn; }},
        configurable: true,
        enumerable: true
    }});
    IDBTransaction.prototype.objectStore = function(name) {{
        if (!this.db._data.stores[name]) {{
            throw new Error("NotFoundError: The specified object store was not found: " + name);
        }}
        return new IDBObjectStore(name, this.db, this);
    }};
    IDBTransaction.prototype.abort = function() {{
        this._active = false;
        if (typeof this._onabort === 'function') this._onabort({{ type: 'abort', target: this }});
        this.dispatchEvent({{ type: 'abort', target: this }});
    }};
    IDBTransaction.prototype.commit = function() {{
        this._active = false;
        if (typeof this._oncomplete === 'function') this._oncomplete({{ type: 'complete', target: this }});
        this.dispatchEvent({{ type: 'complete', target: this }});
    }};
    IDBTransaction.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBTransaction.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBTransaction.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBTransaction = IDBTransaction;

    function IDBObjectStore(name, db, tx) {{
        this.name = name;
        this.transaction = tx;
        this._db = db;
        var sData = db._data.stores[name] || {{ keyPath: null, autoIncrement: false, indexes: {{}}, records: [], nextId: 1 }};
        this.keyPath = sData.keyPath;
        this.autoIncrement = sData.autoIncrement;
        this.indexNames = new DOMStringList(Object.keys(sData.indexes || {{}}));
    }}
    IDBObjectStore.prototype._save = function() {{
        this._db._persist();
    }};
    IDBObjectStore.prototype._findRecordIndex = function(key) {{
        var sData = this._db._data.stores[this.name];
        if (!sData) return -1;
        for (var i = 0; i < sData.records.length; i++) {{
            if (sData.records[i].key === key) return i;
        }}
        return -1;
    }};
    IDBObjectStore.prototype.get = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var idx = this._findRecordIndex(key);
        var res = idx !== -1 ? this._db._data.stores[this.name].records[idx].value : undefined;
        req.result = res;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.getAll = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                if (!query || (query.includes ? query.includes(rec.key) : rec.key === query)) {{
                    all.push(rec.value);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.getAllKeys = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                if (!query || (query.includes ? query.includes(rec.key) : rec.key === query)) {{
                    all.push(rec.key);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.put = function(value, key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath && value && typeof value === 'object') {{
            recKey = value[this.keyPath];
        }}
        if (recKey === undefined && this.autoIncrement) {{
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object') {{
                value[this.keyPath] = recKey;
            }}
        }}
        if (recKey === undefined) {{
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            return req;
        }}
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {{
            sData.records[idx].value = value;
        }} else {{
            sData.records.push({{ key: recKey, value: value }});
        }}
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.add = function(value, key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath && value && typeof value === 'object') {{
            recKey = value[this.keyPath];
        }}
        if (recKey === undefined && this.autoIncrement) {{
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object') {{
                value[this.keyPath] = recKey;
            }}
        }}
        if (recKey === undefined) {{
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            return req;
        }}
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {{
            req.error = new Error("ConstraintError: Key already exists in object store");
            req.readyState = 'done';
            return req;
        }}
        sData.records.push({{ key: recKey, value: value }});
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.delete = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var idx = this._findRecordIndex(key);
        if (idx !== -1) {{
            sData.records.splice(idx, 1);
            this._save();
        }}
        req.result = undefined;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.clear = function() {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        if (sData) {{
            sData.records = [];
            this._save();
        }}
        req.result = undefined;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.count = function(query) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var c = 0;
        if (sData) {{
            if (!query) {{
                c = sData.records.length;
            }} else {{
                for (var i = 0; i < sData.records.length; i++) {{
                    var k = sData.records[i].key;
                    if (query.includes ? query.includes(k) : k === query) c++;
                }}
            }}
        }}
        req.result = c;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.createIndex = function(name, keyPath, options) {{
        var sData = this._db._data.stores[this.name];
        sData.indexes[name] = {{
            name: name,
            keyPath: keyPath,
            unique: !!(options && options.unique),
            multiEntry: !!(options && options.multiEntry)
        }};
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
        return new IDBIndex(name, this);
    }};
    IDBObjectStore.prototype.index = function(name) {{
        var sData = this._db._data.stores[this.name];
        if (!sData.indexes[name]) {{
            throw new Error("NotFoundError: Index not found: " + name);
        }}
        return new IDBIndex(name, this);
    }};
    IDBObjectStore.prototype.deleteIndex = function(name) {{
        var sData = this._db._data.stores[this.name];
        delete sData.indexes[name];
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
    }};
    IDBObjectStore.prototype.openCursor = function(query, direction) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var records = sData ? sData.records.slice() : [];
        if (direction === 'prev' || direction === 'prevunique') {{
            records.reverse();
        }}
        var matched = [];
        for (var i = 0; i < records.length; i++) {{
            var k = records[i].key;
            if (!query || (query.includes ? query.includes(k) : k === query)) {{
                matched.push(records[i]);
            }}
        }}
        if (matched.length > 0) {{
            var cursor = new IDBCursorWithValue(this, direction || 'next', matched, 0, req);
            req.result = cursor;
        }} else {{
            req.result = null;
        }}
        req.readyState = 'done';
        return req;
    }};
    globalThis.IDBObjectStore = IDBObjectStore;

    function IDBCursor(source, direction, records, index, req) {{
        this.source = source;
        this.direction = direction || 'next';
        this._records = records;
        this._index = index;
        this._req = req;
        var cur = records[index];
        this.key = cur ? cur.key : undefined;
        this.primaryKey = cur ? cur.key : undefined;
    }}
    IDBCursor.prototype.continue = function() {{
        this._index++;
        if (this._index < this._records.length) {{
            var cur = this._records[this._index];
            this.key = cur.key;
            this.primaryKey = cur.key;
            if (this.value !== undefined) this.value = cur.value;
            this._req.result = this;
        }} else {{
            this._req.result = null;
        }}
        this._req._successDispatched = false;
        if (typeof this._req._onsuccess === 'function') {{
            this._req._successDispatched = true;
            try {{ this._req._onsuccess.call(this._req, {{ type: 'success', target: this._req }}); }} catch (e) {{}}
        }}
        this._req.dispatchEvent({{ type: 'success', target: this._req }});
    }};
    IDBCursor.prototype.advance = function(count) {{
        this._index += (count - 1);
        this.continue();
    }};
    globalThis.IDBCursor = IDBCursor;

    function IDBCursorWithValue(source, direction, records, index, req) {{
        IDBCursor.call(this, source, direction, records, index, req);
        var cur = records[index];
        this.value = cur ? cur.value : undefined;
    }}
    IDBCursorWithValue.prototype = Object.create(IDBCursor.prototype);
    globalThis.IDBCursorWithValue = IDBCursorWithValue;

    function IDBIndex(name, store) {{
        this.name = name;
        this.objectStore = store;
        var idxData = store._db._data.stores[store.name].indexes[name];
        this.keyPath = idxData.keyPath;
        this.unique = idxData.unique;
        this.multiEntry = idxData.multiEntry;
    }}
    IDBIndex.prototype.get = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var found = undefined;
        if (sData) {{
            for (var i = 0; i < sData.records.length; i++) {{
                var rec = sData.records[i];
                var val = rec.value;
                var idxKey = val && typeof val === 'object' ? val[this.keyPath] : undefined;
                if (idxKey === key) {{
                    found = val;
                    break;
                }}
            }}
        }}
        req.result = found;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.getAll = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                var val = rec.value;
                var idxKey = val && typeof val === 'object' ? val[this.keyPath] : undefined;
                if (!query || (query.includes ? query.includes(idxKey) : idxKey === query)) {{
                    all.push(val);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.count = function(query) {{
        return this.objectStore.count(query);
    }};
    globalThis.IDBIndex = IDBIndex;

    function IDBDatabase(name, version, data) {{
        this.name = name;
        this.version = version;
        this._data = data;
        this.objectStoreNames = new DOMStringList(Object.keys(data.stores));
        this._listeners = {{}};
    }}
    IDBDatabase.prototype._persist = function() {{
        _mangoIdbSave(this.name, JSON.stringify(this._data));
    }};
    IDBDatabase.prototype.createObjectStore = function(name, options) {{
        if (this._data.stores[name]) {{
            throw new Error("ConstraintError: An object store with the specified name already exists");
        }}
        this._data.stores[name] = {{
            name: name,
            keyPath: options && options.keyPath !== undefined ? options.keyPath : null,
            autoIncrement: !!(options && options.autoIncrement),
            indexes: {{}},
            records: [],
            nextId: 1
        }};
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
        return new IDBObjectStore(name, this, null);
    }};
    IDBDatabase.prototype.deleteObjectStore = function(name) {{
        if (!this._data.stores[name]) {{
            throw new Error("NotFoundError: The specified object store was not found");
        }}
        delete this._data.stores[name];
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
    }};
    IDBDatabase.prototype.transaction = function(storeNames, mode) {{
        return new IDBTransaction(this, storeNames, mode);
    }};
    IDBDatabase.prototype.close = function() {{}};
    IDBDatabase.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBDatabase.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBDatabase.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBDatabase = IDBDatabase;

    function IDBFactory() {{}}
    IDBFactory.prototype.open = function(name, version) {{
        var req = new IDBOpenDBRequest();
        var raw = _mangoIdbLoad(name);
        var dbData = null;
        if (raw) {{
            try {{ dbData = JSON.parse(raw); }} catch (e) {{}}
        }}
        var isNew = !dbData;
        if (isNew) {{
            dbData = {{ name: name, version: 0, stores: {{}} }};
        }}
        var currentVersion = dbData.version || 0;
        var targetVersion = version !== undefined ? version : (currentVersion || 1);
        if (targetVersion < currentVersion) {{
            req.error = new Error("VersionError: Requested version is lower than current version");
            req.readyState = 'done';
            return req;
        }}
        var db = new IDBDatabase(name, targetVersion, dbData);
        req.result = db;
        req.readyState = 'done';
        if (targetVersion > currentVersion) {{
            var oldVersion = currentVersion;
            dbData.version = targetVersion;
            db.version = targetVersion;
            db._persist();
            var tx = new IDBTransaction(db, Object.keys(dbData.stores), 'versionchange');
            req.transaction = tx;
            var upEvt = new IDBVersionChangeEvent('upgradeneeded', {{
                oldVersion: oldVersion,
                newVersion: targetVersion
            }});
            upEvt.target = req;
            req._needsUpgrade = true;
            req._upgradeEvent = upEvt;
        }}
        return req;
    }};
    IDBFactory.prototype.deleteDatabase = function(name) {{
        var req = new IDBOpenDBRequest();
        _mangoIdbDelete(name);
        req.result = undefined;
        req.readyState = 'done';
        return req;
    }};
    IDBFactory.prototype.databases = function() {{
        var list = _mangoIdbList() || [];
        var res = [];
        for (var i = 0; i < list.length; i++) {{
            var raw = _mangoIdbLoad(list[i]);
            var ver = 1;
            if (raw) {{
                try {{ ver = JSON.parse(raw).version || 1; }} catch (e) {{}}
            }}
            res.push({{ name: list[i], version: ver }});
        }}
        return Promise.resolve(res);
    }};
    IDBFactory.prototype.cmp = function(a, b) {{
        if (a < b) return -1;
        if (a > b) return 1;
        return 0;
    }};
    globalThis.IDBFactory = IDBFactory;
    globalThis.IDBDatabase = IDBDatabase;
    globalThis.IDBObjectStore = IDBObjectStore;
    globalThis.IDBIndex = IDBIndex;
    globalThis.IDBCursor = IDBCursor;
    globalThis.IDBCursorWithValue = IDBCursorWithValue;
    globalThis.indexedDB = new IDBFactory();

    // ── 34. Intl API (ECMA-402) ────────────────────────────────────────────────
    (function() {{
        var Intl = globalThis.Intl || {{}};

        // 1. NumberFormat
        function NumberFormat(locales, options) {{
            options = options || {{}};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._style = options.style || 'decimal';
            this._currency = options.currency || 'USD';
            this._minimumFractionDigits = options.minimumFractionDigits !== undefined ? options.minimumFractionDigits : (this._style === 'currency' ? 2 : 0);
            this._maximumFractionDigits = options.maximumFractionDigits !== undefined ? options.maximumFractionDigits : (this._style === 'currency' ? 2 : 3);
            this._notation = options.notation || 'standard';
            this._useGrouping = options.useGrouping !== false;
        }}
        NumberFormat.prototype.resolvedOptions = function() {{
            return {{
                locale: this._locale,
                numberingSystem: 'latn',
                style: this._style,
                currency: this._currency,
                minimumFractionDigits: this._minimumFractionDigits,
                maximumFractionDigits: this._maximumFractionDigits,
                useGrouping: this._useGrouping,
                notation: this._notation
            }};
        }};
        NumberFormat.prototype.format = function(num) {{
            var n = Number(num);
            if (isNaN(n)) return 'NaN';
            if (this._notation === 'compact') {{
                var abs = Math.abs(n);
                var sign = n < 0 ? '-' : '';
                if (abs >= 1e9) return sign + (abs / 1e9).toFixed(1).replace(/\.0$/, '') + 'B';
                if (abs >= 1e6) return sign + (abs / 1e6).toFixed(1).replace(/\.0$/, '') + 'M';
                if (abs >= 1e3) return sign + (abs / 1e3).toFixed(1).replace(/\.0$/, '') + 'K';
                return sign + abs.toString();
            }}
            var fixed = n.toFixed(this._maximumFractionDigits);
            if (this._minimumFractionDigits < this._maximumFractionDigits) {{
                var parts = fixed.split('.');
                if (parts.length > 1) {{
                    var dec = parts[1];
                    while (dec.length > this._minimumFractionDigits && dec.endsWith('0')) {{
                        dec = dec.slice(0, -1);
                    }}
                    fixed = dec.length > 0 ? parts[0] + '.' + dec : parts[0];
                }}
            }}
            if (this._useGrouping) {{
                var p = fixed.split('.');
                p[0] = p[0].replace(/\B(?=(\d{{3}})+(?!\d))/g, ',');
                fixed = p.join('.');
            }}
            if (this._style === 'currency') {{
                var sym = (this._currency === 'EUR') ? '€' : (this._currency === 'GBP') ? '£' : (this._currency === 'JPY') ? '¥' : '$';
                return sym + fixed;
            }} else if (this._style === 'percent') {{
                return (n * 100).toFixed(this._maximumFractionDigits) + '%';
            }}
            return fixed;
        }};
        NumberFormat.prototype.formatToParts = function(num) {{
            return [{{ type: 'integer', value: this.format(num) }}];
        }};
        NumberFormat.supportedLocalesOf = function(locales) {{
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        }};
        Intl.NumberFormat = NumberFormat;

        // 2. DateTimeFormat
        function DateTimeFormat(locales, options) {{
            options = options || {{}};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options;
        }}
        DateTimeFormat.prototype.resolvedOptions = function() {{
            return Object.assign({{
                locale: this._locale,
                calendar: 'gregory',
                numberingSystem: 'latn',
                timeZone: 'UTC'
            }}, this._options);
        }};
        DateTimeFormat.prototype.format = function(date) {{
            var d = (date === undefined) ? new Date() : (date instanceof Date ? date : new Date(date));
            if (isNaN(d.getTime())) return 'Invalid Date';
            var opts = this._options;
            if (opts.year && opts.month && opts.day) {{
                return (d.getMonth() + 1) + '/' + d.getDate() + '/' + d.getFullYear();
            }}
            if (opts.hour || opts.minute) {{
                return d.toLocaleTimeString ? d.toLocaleTimeString() : d.toTimeString().split(' ')[0];
            }}
            return d.toLocaleDateString ? d.toLocaleDateString() : d.toDateString();
        }};
        DateTimeFormat.prototype.formatToParts = function(date) {{
            return [{{ type: 'literal', value: this.format(date) }}];
        }};
        DateTimeFormat.prototype.formatRange = function(start, end) {{
            return this.format(start) + ' – ' + this.format(end);
        }};
        DateTimeFormat.supportedLocalesOf = function(locales) {{
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        }};
        Intl.DateTimeFormat = DateTimeFormat;

        // 3. Collator
        function Collator(locales, options) {{
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options || {{}};
        }}
        Collator.prototype.resolvedOptions = function() {{
            return {{ locale: this._locale, usage: 'sort', sensitivity: 'variant' }};
        }};
        Collator.prototype.compare = function(a, b) {{
            var sa = String(a), sb = String(b);
            return sa.localeCompare ? sa.localeCompare(sb) : (sa < sb ? -1 : (sa > sb ? 1 : 0));
        }};
        Collator.supportedLocalesOf = function(locales) {{ return [locales || 'en-US']; }};
        Intl.Collator = Collator;

        // 4. PluralRules
        function PluralRules(locales, options) {{
            this._locale = locales || 'en-US';
        }}
        PluralRules.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: 'cardinal' }}; }};
        PluralRules.prototype.select = function(n) {{ return Number(n) === 1 ? 'one' : 'other'; }};
        Intl.PluralRules = PluralRules;

        // 5. RelativeTimeFormat
        function RelativeTimeFormat(locales, options) {{
            this._locale = locales || 'en-US';
            this._numeric = (options && options.numeric) || 'always';
        }}
        RelativeTimeFormat.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, numeric: this._numeric }}; }};
        RelativeTimeFormat.prototype.format = function(value, unit) {{
            var v = Math.round(Number(value));
            var u = String(unit).toLowerCase();
            if (v === 0 && this._numeric === 'auto') {{
                if (u === 'day') return 'today';
                if (u === 'hour') return 'this hour';
            }}
            var pluralUnit = (Math.abs(v) === 1) ? u : (u.endsWith('s') ? u : u + 's');
            if (v < 0) return Math.abs(v) + ' ' + pluralUnit + ' ago';
            return 'in ' + v + ' ' + pluralUnit;
        }};
        RelativeTimeFormat.prototype.formatToParts = function(v, u) {{
            return [{{ type: 'literal', value: this.format(v, u) }}];
        }};
        Intl.RelativeTimeFormat = RelativeTimeFormat;

        // 6. DisplayNames
        var _displayNamesDict = {{
            'en': 'English', 'es': 'Spanish', 'fr': 'French', 'de': 'German',
            'zh': 'Chinese', 'ja': 'Japanese', 'ko': 'Korean', 'ru': 'Russian',
            'pt': 'Portuguese', 'it': 'Italian', 'ar': 'Arabic', 'hi': 'Hindi',
            'US': 'United States', 'GB': 'United Kingdom', 'CA': 'Canada',
            'FR': 'France', 'DE': 'Germany', 'JP': 'Japan', 'CN': 'China',
            'USD': 'US Dollar', 'EUR': 'Euro', 'GBP': 'British Pound', 'JPY': 'Japanese Yen'
        }};
        function DisplayNames(locales, options) {{
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'language';
        }}
        DisplayNames.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: this._type }}; }};
        DisplayNames.prototype.of = function(code) {{
            var c = String(code);
            return _displayNamesDict[c] || _displayNamesDict[c.toLowerCase()] || c;
        }};
        Intl.DisplayNames = DisplayNames;

        // 7. ListFormat
        function ListFormat(locales, options) {{
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'conjunction';
        }}
        ListFormat.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: this._type }}; }};
        ListFormat.prototype.format = function(list) {{
            if (!Array.isArray(list)) return '';
            if (list.length === 0) return '';
            if (list.length === 1) return String(list[0]);
            if (list.length === 2) return list[0] + (this._type === 'disjunction' ? ' or ' : ' and ') + list[1];
            var last = list[list.length - 1];
            var rest = list.slice(0, -1).join(', ');
            return rest + (this._type === 'disjunction' ? ', or ' : ', and ') + last;
        }};
        ListFormat.prototype.formatToParts = function(list) {{
            return [{{ type: 'element', value: this.format(list) }}];
        }};
        Intl.ListFormat = ListFormat;

        // 8. Segmenter
        function Segmenter(locales, options) {{
            this._locale = locales || 'en-US';
        }}
        Segmenter.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, granularity: 'grapheme' }}; }};
        Segmenter.prototype.segment = function(input) {{
            var str = String(input);
            var chars = Array.from(str);
            var idx = 0;
            var segments = chars.map(function(ch) {{
                var seg = {{ segment: ch, index: idx, input: str, isWordLike: /\\w/.test(ch) }};
                idx += ch.length;
                return seg;
            }});
            return {{
                [Symbol.iterator]: function() {{
                    var i = 0;
                    return {{
                        next: function() {{
                            return i < segments.length ? {{ value: segments[i++], done: false }} : {{ done: true }};
                        }}
                    }};
                }}
            }};
        }};
        Intl.Segmenter = Segmenter;

        globalThis.Intl = Intl;
    }})();

}})();
"##
    );

    if let Err(e) = context.eval(boa_engine::Source::from_bytes(shim.as_bytes())) {
        log::error!("Failed to install browser Web API shims: {}", e);
        #[cfg(test)]
        panic!("Failed to install browser Web API shims: {}", e);
    }
}
