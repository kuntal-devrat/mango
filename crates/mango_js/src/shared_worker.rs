//! Multi-context Shared Workers implementation (WHATWG HTML § 10.1.2).
//!
//! Provides singleton background JavaScript worker threads shared across multiple
//! browsing contexts (tabs, windows, iframes) of the same origin.
//! Communication is mediated through entangled `MessagePort` instances passed
//! via the `connect` event.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use deno_core::{OpState, op2};

static NEXT_PORT_ID: AtomicU64 = AtomicU64::new(1);

/// Inbound messages to the shared worker thread.
#[derive(Debug)]
pub enum SharedWorkerInboundMsg {
    ConnectPort(u64),
    PostMessage { port_id: u64, message: String },
    ClosePort(u64),
    Terminate,
}

/// Key that uniquely identifies a singleton SharedWorker.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SharedWorkerKey {
    pub origin: String,
    pub script_url: String,
    pub name: String,
}

impl SharedWorkerKey {
    pub fn new(
        origin: impl Into<String>,
        script_url: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            origin: origin.into(),
            script_url: script_url.into(),
            name: name.into(),
        }
    }
}

/// Active handle to a shared worker background thread.
pub struct SharedWorkerHandle {
    pub tx: Sender<SharedWorkerInboundMsg>,
    pub thread: Option<JoinHandle<()>>,
    pub ports: Vec<u64>,
}

/// Hub managing all active singleton SharedWorker threads across the browser.
#[derive(Clone, Default)]
pub struct SharedWorkerHub {
    workers: Arc<Mutex<HashMap<SharedWorkerKey, SharedWorkerHandle>>>,
    port_to_key: Arc<Mutex<HashMap<u64, SharedWorkerKey>>>,
    controls: Arc<Mutex<HashMap<SharedWorkerKey, Arc<SharedWorkerControl>>>>,
}

struct SharedWorkerControl {
    running: Arc<AtomicBool>,
    isolate: Mutex<Option<deno_core::v8::IsolateHandle>>,
}

impl SharedWorkerHub {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            port_to_key: Arc::new(Mutex::new(HashMap::new())),
            controls: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Allocates a unique ID for a new MessagePort connection.
    pub fn next_port_id(&self) -> u64 {
        NEXT_PORT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Connects to an existing shared worker or spawns a new dedicated thread.
    pub fn connect(
        &self,
        key: SharedWorkerKey,
        port_id: u64,
        task_queue: Arc<Mutex<Vec<String>>>,
        blobs: Arc<Mutex<HashMap<String, String>>>,
    ) -> Result<(), String> {
        let mut workers = self.workers.lock().map_err(|e| e.to_string())?;

        // Track port to worker mapping
        if let Ok(mut map) = self.port_to_key.lock() {
            map.insert(port_id, key.clone());
        }

        if let Some(handle) = workers.get_mut(&key) {
            handle.ports.push(port_id);
            handle
                .tx
                .send(SharedWorkerInboundMsg::ConnectPort(port_id))
                .map_err(|e| format!("Failed to send connect to shared worker: {e}"))?;
            return Ok(());
        }

        // Spawn new shared worker OS thread
        let (tx, rx) = mpsc::channel::<SharedWorkerInboundMsg>();
        let worker_key = key.clone();
        let target_script = key.script_url.clone();
        let origin = key.origin.clone();
        let name = key.name.clone();
        let control = Arc::new(SharedWorkerControl {
            running: Arc::new(AtomicBool::new(true)),
            isolate: Mutex::new(None),
        });
        if let Ok(mut controls) = self.controls.lock() {
            controls.insert(worker_key.clone(), control.clone());
        }
        let worker_control = control;

        let thread_handle = thread::Builder::new()
            .name(format!("mango-shared-worker-{}", key.name))
            .spawn(move || {
                run_shared_worker_thread(
                    name,
                    target_script,
                    origin,
                    rx,
                    blobs,
                    task_queue,
                    worker_control,
                );
            })
            .ok();

        // Send initial connect
        let _ = tx.send(SharedWorkerInboundMsg::ConnectPort(port_id));

        let handle = SharedWorkerHandle {
            tx,
            thread: thread_handle,
            ports: vec![port_id],
        };
        workers.insert(worker_key, handle);
        Ok(())
    }

    /// Posts a message from a client MessagePort into the shared worker thread.
    pub fn post_message(&self, port_id: u64, message: &str) -> Result<(), String> {
        let key = {
            let map = self.port_to_key.lock().map_err(|e| e.to_string())?;
            map.get(&port_id)
                .cloned()
                .ok_or_else(|| format!("Unknown port {}", port_id))?
        };

        let workers = self.workers.lock().map_err(|e| e.to_string())?;
        if let Some(handle) = workers.get(&key) {
            handle
                .tx
                .send(SharedWorkerInboundMsg::PostMessage {
                    port_id,
                    message: message.to_string(),
                })
                .map_err(|e| format!("Failed to post to shared worker: {e}"))
        } else {
            Err("Shared worker not found".to_string())
        }
    }

    /// Closes a port connection to a shared worker.
    pub fn close_port(&self, port_id: u64) {
        let key_opt = if let Ok(mut map) = self.port_to_key.lock() {
            map.remove(&port_id)
        } else {
            None
        };

        if let Some(key) = key_opt
            && let Ok(mut workers) = self.workers.lock()
            && let Some(handle) = workers.get_mut(&key)
        {
            let _ = handle.tx.send(SharedWorkerInboundMsg::ClosePort(port_id));
            handle.ports.retain(|&p| p != port_id);
            if handle.ports.is_empty() {
                // Remove under the map lock, then interrupt outside it. A
                // worker may be running arbitrary page code and must never
                // make the caller (often the UI thread) wait in join().
                if let Some(mut handle) = workers.remove(&key) {
                    let control = self
                        .controls
                        .lock()
                        .ok()
                        .and_then(|mut map| map.remove(&key));
                    if let Some(control) = control {
                        stop_shared_worker(&control);
                    }
                    let _ = handle.tx.send(SharedWorkerInboundMsg::Terminate);
                    handle.thread.take();
                }
            }
        }
    }

    /// Shuts down all active shared workers immediately.
    pub fn terminate_all(&self) {
        let workers = self
            .workers
            .lock()
            .map(|mut workers| workers.drain().collect::<Vec<_>>());
        if let Ok(workers) = workers {
            let controls = self
                .controls
                .lock()
                .map(|mut controls| std::mem::take(&mut *controls))
                .unwrap_or_default();
            if let Ok(mut ports) = self.port_to_key.lock() {
                ports.clear();
            }
            for (key, mut handle) in workers {
                if let Some(control) = controls.get(&key) {
                    stop_shared_worker(control);
                }
                let _ = handle.tx.send(SharedWorkerInboundMsg::Terminate);
                handle.thread.take();
            }
        }
    }

    /// Returns the number of currently active shared workers.
    pub fn active_count(&self) -> usize {
        self.workers.lock().map(|w| w.len()).unwrap_or(0)
    }
}

impl Drop for SharedWorkerHub {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

// ── Shared Worker V8 Thread Runner ───────────────────────────────────────────

struct SharedWorkerState {
    _name: String,
    base_url: String,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    main_tasks: Arc<Mutex<Vec<String>>>,
    is_running: Arc<AtomicBool>,
}

#[op2(fast)]
fn op_shared_worker_post_port_message(state: &mut OpState, port_id: u32, #[string] text: String) {
    if let Some(ws) = state.try_borrow::<SharedWorkerState>() {
        let escaped = serde_json::to_string(&text).unwrap_or_else(|_| format!("\"{}\"", text));
        let cmd = format!(
            "if (typeof _mangoDispatchSharedPortMessage === 'function') {{ \
                _mangoDispatchSharedPortMessage({}, {}); \
            }}",
            port_id, escaped
        );
        if let Ok(mut q) = ws.main_tasks.lock() {
            q.push(cmd);
        }
        mango_core::wake_event_loop();
    }
}

#[op2(fast)]
fn op_shared_worker_close(state: &mut OpState) {
    if let Some(ws) = state.try_borrow::<SharedWorkerState>() {
        ws.is_running.store(false, Ordering::Relaxed);
    }
}

#[op2]
#[string]
fn op_shared_worker_load_script(state: &mut OpState, #[string] url: String) -> Option<String> {
    if let Some(ws) = state.try_borrow::<SharedWorkerState>() {
        crate::worker::resolve_script_content(&url, &ws.base_url, &ws.blobs).ok()
    } else {
        None
    }
}

deno_core::extension!(
    mango_shared_worker_runtime,
    ops = [
        op_shared_worker_post_port_message,
        op_shared_worker_close,
        op_shared_worker_load_script,
    ],
);

fn run_shared_worker_thread(
    name: String,
    script_url: String,
    base_url: String,
    rx: Receiver<SharedWorkerInboundMsg>,
    blobs: Arc<Mutex<HashMap<String, String>>>,
    main_tasks: Arc<Mutex<Vec<String>>>,
    control: Arc<SharedWorkerControl>,
) {
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok();
    let _guard = tokio_rt.as_ref().map(|r| r.enter());

    let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        extensions: vec![mango_shared_worker_runtime::init()],
        ..Default::default()
    });
    if let Ok(mut isolate) = control.isolate.lock() {
        *isolate = Some(runtime.v8_isolate().thread_safe_handle());
    }

    runtime.op_state().borrow_mut().put(SharedWorkerState {
        _name: name.clone(),
        base_url: base_url.clone(),
        blobs: blobs.clone(),
        main_tasks: main_tasks.clone(),
        is_running: control.running.clone(),
    });

    // Install SharedWorkerGlobalScope shim
    let init_shim = format!(
        r#"
        const _ops = Deno.core.ops;
        globalThis.self = globalThis;
        globalThis.name = {:?};
        globalThis.onconnect = null;
        globalThis._ports = {{}};
        globalThis._listeners = {{}};

        globalThis.addEventListener = function(type, fn) {{
            if (!this._listeners[type]) this._listeners[type] = [];
            this._listeners[type].push(fn);
        }};
        globalThis.removeEventListener = function(type, fn) {{
            if (!this._listeners[type]) return;
            this._listeners[type] = this._listeners[type].filter(f => f !== fn);
        }};
        globalThis.dispatchEvent = function(evt) {{
            if (evt.type === 'connect' && typeof this.onconnect === 'function') {{
                try {{ this.onconnect(evt); }} catch(e) {{}}
            }}
            const list = this._listeners[evt.type] || [];
            for (const f of list) {{
                try {{ f.call(this, evt); }} catch(e) {{}}
            }}
            return true;
        }};

        function SharedWorkerPort(portId) {{
            this._portId = portId;
            this.onmessage = null;
            this._listeners = {{}};
            this._started = false;
            this._queue = [];
        }}
        SharedWorkerPort.prototype.start = function() {{
            this._started = true;
            while (this._queue.length > 0) {{
                const msg = this._queue.shift();
                this._dispatch(msg);
            }}
        }};
        SharedWorkerPort.prototype.postMessage = function(data) {{
            _ops.op_shared_worker_post_port_message(this._portId, JSON.stringify(data));
        }};
        SharedWorkerPort.prototype.addEventListener = function(t, fn) {{
            if (!this._listeners[t]) this._listeners[t] = [];
            this._listeners[t].push(fn);
        }};
        SharedWorkerPort.prototype._dispatch = function(data) {{
            const ev = {{ type: 'message', data: data }};
            if (typeof this.onmessage === 'function') {{
                try {{ this.onmessage(ev); }} catch(e) {{}}
            }}
            const list = this._listeners['message'] || [];
            for (const f of list) {{
                try {{ f.call(this, ev); }} catch(e) {{}}
            }}
        }};

        globalThis._mangoConnectPort = function(portId) {{
            const port = new SharedWorkerPort(portId);
            globalThis._ports[portId] = port;
            const evt = {{
                type: 'connect',
                ports: [port]
            }};
            globalThis.dispatchEvent(evt);
        }};

        globalThis._mangoDeliverPortMessage = function(portId, data) {{
            const port = globalThis._ports[portId];
            if (port) {{
                if (port._started || port.onmessage) {{
                    port._dispatch(data);
                }} else {{
                    port._queue.push(data);
                }}
            }}
        }};

        globalThis.close = function() {{
            _ops.op_shared_worker_close();
        }};
        "#,
        name
    );

    if !control.running.load(Ordering::Acquire) {
        return;
    }
    let _ = runtime.execute_script("<shared-worker-init>", init_shim);

    // Fetch and execute the worker script
    let script_source = crate::worker::resolve_script_content(&script_url, &base_url, &blobs)
        .unwrap_or_else(|e| format!("console.error('Failed to load shared worker: {}');", e));

    if control.running.load(Ordering::Acquire) {
        let _ = runtime.execute_script(script_url, script_source);
    }

    // Event pump
    while control.running.load(Ordering::Acquire) {
        match rx.recv_timeout(std::time::Duration::from_millis(5)) {
            Ok(SharedWorkerInboundMsg::ConnectPort(port_id)) => {
                let code = format!("globalThis._mangoConnectPort({});", port_id);
                let _ = runtime.execute_script("<connect>", code);
            }
            Ok(SharedWorkerInboundMsg::PostMessage { port_id, message }) => {
                let code = format!(
                    "globalThis._mangoDeliverPortMessage({}, {});",
                    port_id, message
                );
                let _ = runtime.execute_script("<message>", code);
            }
            Ok(SharedWorkerInboundMsg::ClosePort(port_id)) => {
                let code = format!("delete globalThis._ports[{}];", port_id);
                let _ = runtime.execute_script("<close>", code);
            }
            Ok(SharedWorkerInboundMsg::Terminate) => {
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Run pending async jobs
        if control.running.load(Ordering::Acquire) {
            let _ = pollster::block_on(
                runtime.run_event_loop(deno_core::PollEventLoopOptions::default()),
            );
        }
    }
}

fn stop_shared_worker(control: &SharedWorkerControl) {
    control.running.store(false, Ordering::Release);
    if let Ok(isolate) = control.isolate.lock()
        && let Some(isolate) = isolate.as_ref()
    {
        isolate.terminate_execution();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_worker_key_equality() {
        let k1 = SharedWorkerKey::new("https://example.com", "/worker.js", "hub");
        let k2 = SharedWorkerKey::new("https://example.com", "/worker.js", "hub");
        let k3 = SharedWorkerKey::new("https://other.com", "/worker.js", "hub");
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
