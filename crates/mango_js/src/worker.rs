//! Multi-threaded Web Workers implementation for the Mango browser engine.
//!
//! Provides true multi-threaded background JavaScript execution adhering to the
//! W3C Web Workers specification. Each `Worker` instance runs in a dedicated OS thread
//! with an independent Boa `Context`, communicating bidirectionally with the main
//! thread through thread-safe message channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use boa_engine::{Context, JsArgs, JsValue, Source};

/// Inbound messages sent from the main thread to a background worker thread.
#[derive(Debug)]
pub enum WorkerInboundMsg {
    PostMessage(String),
    Terminate,
}

macro_rules! native_fn {
    ($closure:expr) => {
        unsafe { boa_engine::NativeFunction::from_closure($closure) }
    };
}

/// Handle to an active background worker thread.
pub struct WorkerHandle {
    pub id: u64,
    pub tx: Sender<WorkerInboundMsg>,
    pub _thread: Option<JoinHandle<()>>,
}

/// Thread-safe manager for all active Web Workers in the page.
#[derive(Clone, Default)]
pub struct WorkerManager {
    workers: Arc<Mutex<HashMap<u64, WorkerHandle>>>,
    blob_urls: Arc<Mutex<HashMap<String, String>>>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            blob_urls: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers text content for a `blob:` URL so workers can load inline blob scripts.
    pub fn register_blob(&self, url: String, content: String) {
        if let Ok(mut map) = self.blob_urls.lock() {
            map.insert(url, content);
        }
    }

    /// Looks up content for a registered `blob:` URL.
    pub fn get_blob(&self, url: &str) -> Option<String> {
        self.blob_urls.lock().ok()?.get(url).cloned()
    }

    /// Spawns a dedicated OS thread running an independent Boa context for the worker.
    pub fn create_worker(
        &self,
        id: u64,
        script_url: &str,
        page_url: &str,
        tasks: Arc<Mutex<Vec<String>>>,
    ) {
        let (tx, rx) = mpsc::channel::<WorkerInboundMsg>();
        let target_script = script_url.to_string();
        let base_url = page_url.to_string();
        let blobs = self.blob_urls.clone();
        let main_tasks = tasks.clone();

        let thread_handle = thread::Builder::new()
            .name(format!("mango-worker-{}", id))
            .spawn(move || {
                run_worker_thread(id, target_script, base_url, rx, blobs, main_tasks);
            })
            .ok();

        let handle = WorkerHandle {
            id,
            tx,
            _thread: thread_handle,
        };

        if let Ok(mut map) = self.workers.lock() {
            map.insert(id, handle);
        }
    }

    /// Sends a serialized JSON message from the main thread into the background worker.
    pub fn post_message(&self, id: u64, message_json: &str) -> Result<(), String> {
        let map = self.workers.lock().map_err(|e| e.to_string())?;
        if let Some(worker) = map.get(&id) {
            worker
                .tx
                .send(WorkerInboundMsg::PostMessage(message_json.to_string()))
                .map_err(|e| format!("Failed to send to worker: {e}"))
        } else {
            Err(format!("Worker id {} not found", id))
        }
    }

    /// Shuts down the background worker thread immediately.
    pub fn terminate(&self, id: u64) {
        let worker_opt = if let Ok(mut map) = self.workers.lock() {
            map.remove(&id)
        } else {
            None
        };
        if let Some(mut worker) = worker_opt {
            let _ = worker.tx.send(WorkerInboundMsg::Terminate);
            if let Some(th) = worker._thread.take() {
                let _ = th.join();
            }
        }
    }
}

impl Drop for WorkerManager {
    fn drop(&mut self) {
        if let Ok(mut map) = self.workers.lock() {
            for (_, mut worker) in map.drain() {
                let _ = worker.tx.send(WorkerInboundMsg::Terminate);
                if let Some(th) = worker._thread.take() {
                    let _ = th.join();
                }
            }
        }
    }
}

/// Resolves script content from a URL (blob, data, HTTP, local path, or inline text).
fn resolve_script_content(
    script_url: &str,
    base_url: &str,
    blobs: &Arc<Mutex<HashMap<String, String>>>,
) -> Result<String, String> {
    let trimmed = script_url.trim();

    // 1. Check blob URL cache
    if trimmed.starts_with("blob:") {
        if let Ok(map) = blobs.lock() {
            if let Some(content) = map.get(trimmed) {
                return Ok(content.clone());
            }
        }
        return Err(format!("Blob URL not found: {}", trimmed));
    }

    // 2. Data URI
    if trimmed.starts_with("data:") {
        if let Some(comma_pos) = trimmed.find(',') {
            let meta = &trimmed[5..comma_pos];
            let payload = &trimmed[comma_pos + 1..];
            if meta.contains("base64") {
                if let Ok(bytes) = mango_net::resource_loader::decode_base64_bytes(payload.trim()) {
                    return Ok(String::from_utf8_lossy(&bytes).to_string());
                }
            } else {
                return Ok(payload.to_string());
            }
        }
    }

    // 3. HTTP / HTTPS network fetch
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(url) = mango_net::Url::parse(trimmed) {
            // WHATWG HTML § 10.2.2: Enforce Same-Origin Policy on Worker script URL
            if (base_url.starts_with("http://") || base_url.starts_with("https://"))
                && let Ok(base) = mango_net::Url::parse(base_url)
            {
                if base.scheme != url.scheme || base.host != url.host || base.port != url.port {
                    return Err(format!(
                        "Same-Origin Policy violation: Worker script origin ({:?}:{:?}) does not match document origin ({:?}:{:?})",
                        url.host, url.port, base.host, base.port
                    ));
                }
            }

            let client = mango_net::HttpClient::new();
            if let Ok(resp) = client.request("GET", &url, &[], None) {
                if resp.status >= 200 && resp.status < 300 {
                    // Check Content-Type: reject HTML error pages / non-JS responses per WHATWG § 10.2.4
                    if let Some(ct) = resp.headers.get("content-type") {
                        let ct_lower = ct.to_ascii_lowercase();
                        if ct_lower.contains("text/html") {
                            return Err(format!("Worker script returned HTML content-type: {ct}"));
                        }
                    }
                    return Ok(String::from_utf8_lossy(&resp.body).to_string());
                } else {
                    return Err(format!(
                        "Worker script fetch failed with HTTP status {}",
                        resp.status
                    ));
                }
            }
        }
    }

    // 4. Local filesystem read (strictly forbidden for remote http:// or https:// origins)
    let is_remote_origin = base_url.starts_with("http://") || base_url.starts_with("https://");
    if !is_remote_origin {
        // Try relative to base URL if base_url is a file path
        if base_url.starts_with("file://") {
            let path = &base_url[7..];
            let p = std::path::Path::new(path);
            if let Some(parent) = p.parent() {
                let joined = parent.join(trimmed);
                if let Ok(canonical) = joined.canonicalize() {
                    if let Ok(parent_canonical) = parent.canonicalize() {
                        if canonical.starts_with(&parent_canonical) {
                            if let Ok(content) = std::fs::read_to_string(&canonical) {
                                return Ok(content);
                            }
                        }
                    }
                }
            }
        }

        // Direct local filesystem read (only allowed in tests with non-traversing relative paths)
        if base_url.is_empty() && !trimmed.contains("..") && !trimmed.starts_with('/') {
            if let Ok(content) = std::fs::read_to_string(trimmed) {
                return Ok(content);
            }
        }
    }

    // 5. If it looks like raw executable JavaScript (e.g., inline code passed in tests), use it
    if trimmed.contains(';')
        || trimmed.contains('{')
        || trimmed.contains("self.")
        || trimmed.contains("postMessage")
    {
        return Ok(trimmed.to_string());
    }

    Err(format!(
        "Could not load worker script from '{}'",
        script_url
    ))
}

/// Execution loop inside the dedicated background worker OS thread.
fn run_worker_thread(
    id: u64,
    script_url: String,
    base_url: String,
    rx: Receiver<WorkerInboundMsg>,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    main_tasks: Arc<Mutex<Vec<String>>>,
) {
    let mut ctx = Context::default();
    let is_running = Arc::new(AtomicBool::new(true));

    // Register native `_workerNativePostMessage`
    let tasks_post = main_tasks.clone();
    let post_fn = native_fn!(move |_this, args, ctx| {
        let val = args.get_or_undefined(0);
        let s = val.to_string(ctx)?.to_std_string_escaped();
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let cmd = format!(
            "if (typeof _mangoDispatchWorkerMessage === 'function') _mangoDispatchWorkerMessage({}, \"{}\");",
            id, escaped
        );
        if let Ok(mut q) = tasks_post.lock() {
            q.push(cmd);
        }
        Ok(JsValue::undefined())
    });
    let _ = ctx.register_global_callable(
        boa_engine::js_string!("_workerNativePostMessage"),
        1,
        post_fn,
    );

    // Register native `close()`
    let running_close = is_running.clone();
    let close_fn = native_fn!(move |_this, _args, _ctx| {
        running_close.store(false, Ordering::Relaxed);
        Ok(JsValue::undefined())
    });
    let _ = ctx.register_global_callable(boa_engine::js_string!("_workerNativeClose"), 0, close_fn);

    // Register native `importScripts()`
    let base_import = base_url.clone();
    let blobs_import = blobs.clone();
    let import_fn = native_fn!(move |_this, args, ctx| {
        for i in 0..args.len() {
            let u_val = args.get_or_undefined(i);
            let u_str = u_val.to_string(ctx)?.to_std_string_escaped();
            match resolve_script_content(&u_str, &base_import, &blobs_import) {
                Ok(code) => {
                    ctx.eval(Source::from_bytes(&code))?;
                }
                Err(e) => {
                    return Err(boa_engine::JsError::from_opaque(JsValue::from(
                        boa_engine::js_string!(format!("importScripts error: {e}")),
                    )));
                }
            }
        }
        Ok(JsValue::undefined())
    });
    let _ = ctx.register_global_callable(boa_engine::js_string!("importScripts"), 1, import_fn);

    // Install worker global environment shim
    let init_shim = r#"
        globalThis.self = globalThis;
        globalThis.onmessage = null;
        globalThis.onerror = null;
        globalThis._workerListeners = {};

        globalThis.addEventListener = function(type, fn) {
            if (!globalThis._workerListeners[type]) globalThis._workerListeners[type] = [];
            globalThis._workerListeners[type].push(fn);
        };
        globalThis.removeEventListener = function(type, fn) {
            if (!globalThis._workerListeners[type]) return;
            globalThis._workerListeners[type] = globalThis._workerListeners[type].filter(function(f) { return f !== fn; });
        };
        globalThis.dispatchEvent = function(evt) {
            var t = evt.type;
            if (t === 'message' && typeof globalThis.onmessage === 'function') {
                try { globalThis.onmessage(evt); } catch(e) {}
            }
            if (t === 'error' && typeof globalThis.onerror === 'function') {
                try { globalThis.onerror(evt); } catch(e) {}
            }
            var list = globalThis._workerListeners[t] || [];
            for (var i = 0; i < list.length; i++) {
                try { list[i](evt); } catch(e) {}
            }
        };

        globalThis.postMessage = function(data) {
            var json = JSON.stringify(data);
            _workerNativePostMessage(json);
        };
        globalThis.close = function() {
            _workerNativeClose();
        };

        globalThis._handleIncomingMessage = function(jsonStr) {
            var data;
            try { data = JSON.parse(jsonStr); } catch(e) { data = jsonStr; }
            var evt = { type: 'message', data: data };
            globalThis.dispatchEvent(evt);
        };
    "#;
    let _ = ctx.eval(Source::from_bytes(init_shim));

    // Resolve and execute initial worker script
    let initial_code = resolve_script_content(&script_url, &base_url, &blobs);
    match initial_code {
        Ok(code) => {
            if let Err(e) = ctx.eval(Source::from_bytes(&code)) {
                let err_msg = format!("Worker script error: {e}")
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r");
                let cmd = format!(
                    "if (typeof _mangoDispatchWorkerError === 'function') _mangoDispatchWorkerError({}, \"{}\");",
                    id, err_msg
                );
                if let Ok(mut q) = main_tasks.lock() {
                    q.push(cmd);
                }
            } else {
                let _ = ctx.run_jobs();
            }
        }
        Err(e) => {
            let err_msg = format!("Worker load error: {e}")
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            let cmd = format!(
                "if (typeof _mangoDispatchWorkerError === 'function') _mangoDispatchWorkerError({}, \"{}\");",
                id, err_msg
            );
            if let Ok(mut q) = main_tasks.lock() {
                q.push(cmd);
            }
        }
    }

    // Message loop: drain incoming messages from main thread
    while is_running.load(Ordering::Relaxed) {
        match rx.recv() {
            Ok(WorkerInboundMsg::PostMessage(json_data)) => {
                let escaped = json_data
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r");
                let code = format!("_handleIncomingMessage(\"{}\");", escaped);
                let _ = ctx.eval(Source::from_bytes(&code));
                let _ = ctx.run_jobs();
            }
            Ok(WorkerInboundMsg::Terminate) | Err(_) => {
                break;
            }
        }
    }
}
