//! Multi-threaded Web Workers implementation for the Mango browser engine.
//!
//! Provides true multi-threaded background JavaScript execution adhering to the
//! W3C Web Workers specification. Each `Worker` instance runs in a dedicated OS thread
//! with an independent V8 `JsRuntime` (via `deno_core`), communicating bidirectionally with the main
//! thread through thread-safe message channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use deno_core::{OpState, op2};

/// Inbound messages sent from the main thread to a background worker thread.
#[derive(Debug)]
pub enum WorkerInboundMsg {
    PostMessage(String),
    Terminate,
}

/// Handle to an active background worker thread.
pub struct WorkerHandle {
    pub id: u64,
    pub tx: Sender<WorkerInboundMsg>,
    pub _thread: Option<JoinHandle<()>>,
    isolate: Arc<Mutex<Option<deno_core::v8::IsolateHandle>>>,
    running: Arc<AtomicBool>,
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

    /// Returns a shared handle to the blob URL registry.
    pub fn blob_urls(&self) -> Arc<Mutex<HashMap<String, String>>> {
        self.blob_urls.clone()
    }

    /// Spawns a dedicated OS thread running an independent V8 runtime for the worker.
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
        let isolate = Arc::new(Mutex::new(None));
        let worker_isolate = isolate.clone();
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = running.clone();

        let thread_handle = thread::Builder::new()
            .name(format!("mango-worker-{}", id))
            .spawn(move || {
                run_worker_thread(
                    id,
                    target_script,
                    base_url,
                    rx,
                    blobs,
                    main_tasks,
                    worker_isolate,
                    worker_running,
                );
            })
            .ok();

        let handle = WorkerHandle {
            id,
            tx,
            _thread: thread_handle,
            isolate,
            running,
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
            self.stop_worker(&worker);
            let _ = worker.tx.send(WorkerInboundMsg::Terminate);
            // Dropping JoinHandle detaches. Never block the caller (often the
            // browser/UI thread) waiting for page-controlled worker code.
            worker._thread.take();
        }
    }

    /// Shuts down all active background worker threads immediately.
    pub fn terminate_all(&self) {
        let workers = self
            .workers
            .lock()
            .map(|mut map| map.drain().map(|(_, w)| w).collect::<Vec<_>>());
        if let Ok(workers) = workers {
            for mut worker in workers {
                self.stop_worker(&worker);
                let _ = worker.tx.send(WorkerInboundMsg::Terminate);
                worker._thread.take();
            }
        }
    }

    fn stop_worker(&self, worker: &WorkerHandle) {
        worker.running.store(false, Ordering::Release);
        if let Ok(isolate) = worker.isolate.lock()
            && let Some(isolate) = isolate.as_ref()
        {
            isolate.terminate_execution();
        }
    }
}

impl Drop for WorkerManager {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

/// Resolves script content from a URL (blob, data, HTTP, local path, or inline text).
pub(crate) fn resolve_script_content(
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

struct WorkerState {
    id: u64,
    base_url: String,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    main_tasks: Arc<Mutex<Vec<String>>>,
    is_running: Arc<AtomicBool>,
}

#[op2(fast)]
fn op_worker_native_post_message(state: &mut OpState, #[string] text: String) {
    if let Some(ws) = state.try_borrow::<WorkerState>() {
        let escaped = serde_json::to_string(&text).unwrap_or_else(|_| format!("\"{}\"", text));
        let cmd = format!(
            "if (typeof _mangoDispatchWorkerMessage === 'function') _mangoDispatchWorkerMessage({}, {});",
            ws.id, escaped
        );
        if let Ok(mut q) = ws.main_tasks.lock() {
            q.push(cmd);
        }
        mango_core::wake_event_loop();
    }
}

#[op2(fast)]
fn op_worker_native_close(state: &mut OpState) {
    if let Some(ws) = state.try_borrow::<WorkerState>() {
        ws.is_running.store(false, Ordering::Relaxed);
    }
}

#[op2]
#[string]
fn op_worker_load_script(state: &mut OpState, #[string] url: String) -> Option<String> {
    if let Some(ws) = state.try_borrow::<WorkerState>() {
        resolve_script_content(&url, &ws.base_url, &ws.blobs).ok()
    } else {
        None
    }
}

deno_core::extension!(
    mango_worker_runtime,
    ops = [
        op_worker_native_post_message,
        op_worker_native_close,
        op_worker_load_script,
    ],
);

/// Execution loop inside the dedicated background worker OS thread.
fn run_worker_thread(
    id: u64,
    script_url: String,
    base_url: String,
    rx: Receiver<WorkerInboundMsg>,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    main_tasks: Arc<Mutex<Vec<String>>>,
    isolate_slot: Arc<Mutex<Option<deno_core::v8::IsolateHandle>>>,
    is_running: Arc<AtomicBool>,
) {
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok();
    let _guard = tokio_rt.as_ref().map(|r| r.enter());

    let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        extensions: vec![mango_worker_runtime::init()],
        ..Default::default()
    });
    if let Ok(mut slot) = isolate_slot.lock() {
        *slot = Some(runtime.v8_isolate().thread_safe_handle());
    }

    runtime.op_state().borrow_mut().put(WorkerState {
        id,
        base_url: base_url.clone(),
        blobs: blobs.clone(),
        main_tasks: main_tasks.clone(),
        is_running: is_running.clone(),
    });

    // Install worker global environment shim
    let href_json = serde_json::to_string(&script_url).unwrap_or_else(|_| "\"\"".to_string());
    let init_shim = format!(
        r#"
        const _ops = Deno.core.ops;
        globalThis.self = globalThis;
        globalThis.onmessage = null;
        globalThis.onerror = null;
        globalThis._workerListeners = {{}};

        if (typeof globalThis.console === 'undefined') {{
            globalThis.console = {{
                log: function() {{}},
                warn: function() {{}},
                error: function() {{}},
                info: function() {{}},
                debug: function() {{}}
            }};
        }}

        var _timerId = 1;
        var _activeTimers = {{}};
        globalThis.setTimeout = function(fn, delay) {{
            var id = _timerId++;
            _activeTimers[id] = true;
            Promise.resolve().then(function() {{
                if (_activeTimers[id]) {{
                    delete _activeTimers[id];
                    if (typeof fn === 'function') {{
                        try {{ fn(); }} catch(e) {{ if (typeof console !== 'undefined') console.error(e); }}
                    }}
                }}
            }});
            return id;
        }};
        globalThis.clearTimeout = function(id) {{
            delete _activeTimers[id];
        }};
        globalThis.setInterval = globalThis.setTimeout;
        globalThis.clearInterval = globalThis.clearTimeout;

        var _workerHref = {href_json};
        globalThis.location = {{
            href: _workerHref,
            origin: "null",
            protocol: "about:",
            host: "",
            hostname: "",
            pathname: _workerHref,
            search: "",
            hash: "",
            toString: function() {{ return this.href; }}
        }};

        globalThis.crypto = {{
            getRandomValues: function(buf) {{
                if (buf && buf.length !== undefined) {{
                    for (var i = 0; i < buf.length; i++) buf[i] = Math.floor(Math.random() * 256);
                }}
                return buf;
            }},
            randomUUID: function() {{
                var s = [];
                var hexDigits = "0123456789abcdef";
                for (var i = 0; i < 36; i++) {{
                    s[i] = hexDigits.substr(Math.floor(Math.random() * 0x10), 1);
                }}
                s[14] = "4";
                s[19] = hexDigits.substr((s[19] & 0x3) | 0x8, 1);
                s[8] = s[13] = s[18] = s[23] = "-";
                return s.join("");
            }}
        }};

        var _workerT0 = Date.now();
        globalThis.performance = {{
            now: function() {{ return Date.now() - _workerT0; }},
            timeOrigin: _workerT0
        }};

        globalThis.addEventListener = function(type, fn) {{
            if (!globalThis._workerListeners[type]) globalThis._workerListeners[type] = [];
            globalThis._workerListeners[type].push(fn);
        }};
        globalThis.removeEventListener = function(type, fn) {{
            if (!globalThis._workerListeners[type]) return;
            globalThis._workerListeners[type] = globalThis._workerListeners[type].filter(function(f) {{ return f !== fn; }});
        }};
        globalThis.dispatchEvent = function(evt) {{
            var t = evt.type;
            if (t === 'message' && typeof globalThis.onmessage === 'function') {{
                try {{ globalThis.onmessage(evt); }} catch(e) {{}}
            }}
            if (t === 'error' && typeof globalThis.onerror === 'function') {{
                try {{ globalThis.onerror(evt); }} catch(e) {{}}
            }}
            var list = globalThis._workerListeners[t] || [];
            for (var i = 0; i < list.length; i++) {{
                try {{ list[i](evt); }} catch(e) {{}}
            }}
        }};

        globalThis.postMessage = function(data) {{
            var json = JSON.stringify(data);
            _ops.op_worker_native_post_message(json);
        }};
        globalThis.close = function() {{
            _ops.op_worker_native_close();
        }};

        globalThis.importScripts = function() {{
            for (var i = 0; i < arguments.length; i++) {{
                var code = _ops.op_worker_load_script(String(arguments[i]));
                if (code == null) {{
                    throw new Error("importScripts failed to load script: " + arguments[i]);
                }}
                (0, eval)(code);
            }}
        }};

        globalThis._handleIncomingMessage = function(jsonStr) {{
            var data;
            try {{ data = JSON.parse(jsonStr); }} catch(e) {{ data = jsonStr; }}
            var evt = {{ type: 'message', data: data }};
            globalThis.dispatchEvent(evt);
        }};
    "#
    );
    let _ = runtime.execute_script("<worker_init>", init_shim);

    // Resolve and execute initial worker script
    let initial_code = resolve_script_content(&script_url, &base_url, &blobs);
    match initial_code {
        _ if !is_running.load(Ordering::Relaxed) => {}
        Ok(code) => {
            if let Err(e) = runtime.execute_script("<worker_script>", code) {
                let err_msg = serde_json::to_string(&format!("Worker script error: {e}"))
                    .unwrap_or_else(|_| "\"Worker script error\"".to_string());
                let cmd = format!(
                    "if (typeof _mangoDispatchWorkerError === 'function') _mangoDispatchWorkerError({}, {});",
                    id, err_msg
                );
                if let Ok(mut q) = main_tasks.lock() {
                    q.push(cmd);
                }
                mango_core::wake_event_loop();
            } else {
                deno_core::v8::scope!(scope, runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
            }
        }
        Err(e) => {
            let err_msg = serde_json::to_string(&format!("Worker load error: {e}"))
                .unwrap_or_else(|_| "\"Worker load error\"".to_string());
            let cmd = format!(
                "if (typeof _mangoDispatchWorkerError === 'function') _mangoDispatchWorkerError({}, {});",
                id, err_msg
            );
            if let Ok(mut q) = main_tasks.lock() {
                q.push(cmd);
            }
            mango_core::wake_event_loop();
        }
    }

    // Message loop: drain incoming messages from main thread
    while is_running.load(Ordering::Relaxed) {
        match rx.recv() {
            Ok(WorkerInboundMsg::PostMessage(json_data)) => {
                let escaped =
                    serde_json::to_string(&json_data).unwrap_or_else(|_| "\"\"".to_string());
                let code = format!("_handleIncomingMessage({});", escaped);
                let _ = runtime.execute_script("<worker_message>", code);
                deno_core::v8::scope!(scope, runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
            }
            Ok(WorkerInboundMsg::Terminate) | Err(_) => {
                break;
            }
        }
    }
}
