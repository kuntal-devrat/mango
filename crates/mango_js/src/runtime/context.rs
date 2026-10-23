use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use mango_html::dom::Document;

use crate::console::{self, ConsoleBuffer};
use crate::dom_bindings::{self, DomDirtyFlag, SharedDocument};
use crate::event_loop::EventLoop;
use crate::web_apis::{self, SharedEventLoop, SharedLocalStorage, SharedStatusText};

/// Keyboard / mouse event modifier flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl EventModifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
}

/// RAII guard ensuring the V8 isolate is entered for the duration of a JS operation,
/// and exited when idle so multiple isolates on the same thread (e.g. browser tabs)
/// can be entered, dropped, or garbage-collected in any arbitrary order.
pub(crate) struct IsolateScopeGuard(*mut deno_core::v8::Isolate);

impl IsolateScopeGuard {
    #[inline]
    pub(crate) fn enter(isolate: &mut deno_core::v8::Isolate) -> Self {
        unsafe {
            isolate.enter();
        }
        Self(isolate as *mut _)
    }
}

impl Drop for IsolateScopeGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (*self.0).exit();
        }
    }
}

/// RAII guard that only exits an already-entered isolate on drop.
///
/// `deno_core::JsRuntime::new` leaves its isolate entered; constructor-time
/// setup must balance that single enter even if a shim panics.
pub(crate) struct IsolateExitGuard(*mut deno_core::v8::Isolate);

impl IsolateExitGuard {
    #[inline]
    pub(crate) fn new(isolate: &mut deno_core::v8::Isolate) -> Self {
        Self(isolate as *mut _)
    }
}

impl Drop for IsolateExitGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (*self.0).exit();
        }
    }
}

/// The Mango JavaScript runtime — one per page.
pub struct JsRuntime {
    pub(crate) runtime: deno_core::JsRuntime,
    pub(crate) event_loop: SharedEventLoop,
    pub(crate) console_buffer: ConsoleBuffer,
    pub(crate) document: SharedDocument,
    pub(crate) dom_dirty: DomDirtyFlag,
    pub(crate) status_text: SharedStatusText,
    pub(crate) pending_nav: web_apis::SharedPendingNav,
    pub(crate) pending_open: web_apis::SharedPendingOpenWindow,
    /// Shared localStorage backing store — the browser may persist this to disk.
    pub local_storage: SharedLocalStorage,
    /// Shared sessionStorage backing store — tab-scoped, in-memory.
    pub session_storage: web_apis::SharedSessionStorage,
    /// Layout geometry published by the browser for measurement APIs.
    pub(crate) layout_bounds: dom_bindings::LayoutBounds,
    /// Shared cookie jar backing `document.cookie` — owned by the browser so
    /// it can be persisted to the profile.
    pub(crate) cookie_jar: dom_bindings::CookieStore,
    /// Set of DOM nodes currently in the `:hover` chain (target and its ancestors).
    pub(crate) hovered_nodes: std::collections::HashSet<mango_html::dom::NodeId>,
    /// Set of DOM nodes currently in the `:active` chain (target and its ancestors).
    pub(crate) active_nodes: std::collections::HashSet<mango_html::dom::NodeId>,
    /// Thread-safe manager for Web Workers.
    pub worker_manager: crate::worker::WorkerManager,
    /// Thread-safe manager for WebSockets.
    pub ws_manager: mango_net::WebSocketManager,
    /// Thread-safe broker for BroadcastChannel instances.
    pub broadcast_channel_hub: crate::broadcast_channel::BroadcastChannelHub,
    /// Thread-safe manager for SharedWorker instances.
    pub shared_worker_hub: crate::shared_worker::SharedWorkerHub,
    /// Thread-safe coordinator for Web Locks API.
    pub web_locks_engine: crate::web_locks::WebLocksEngine,
    /// Registered import maps for ES module resolution.
    pub(crate) import_maps: Arc<Mutex<HashMap<String, String>>>,
    /// Once exceeded, page script execution is disabled for this document.
    /// This prevents one page's JS graph from taking down the browser.
    pub(crate) execution_disabled: bool,
}

static TOKIO_RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

thread_local! {
    static CURRENT_GUARD: std::cell::RefCell<Option<tokio::runtime::EnterGuard<'static>>> = const { std::cell::RefCell::new(None) };
}

/// Ensures that a Tokio runtime context is active for the current thread so that
/// V8's timer/delayed tasks can be scheduled.
pub fn ensure_tokio_runtime() {
    if tokio::runtime::Handle::try_current().is_err() {
        let rt = TOKIO_RT.get_or_init(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to initialize tokio runtime for JS")
        });
        CURRENT_GUARD.with(|cell| {
            let mut guard = cell.borrow_mut();
            if guard.is_none() {
                *guard = Some(rt.enter());
            }
        });
    }
}

impl JsRuntime {
    #[inline]
    pub(crate) fn enter_isolate(&mut self) -> IsolateScopeGuard {
        IsolateScopeGuard::enter(self.runtime.v8_isolate())
    }

    /// Creates a new JS runtime with a fresh V8 runtime, registers all
    /// DOM bindings, console, and Web API globals.
    pub fn new(document: Document, viewport_width: f32, viewport_height: f32) -> Self {
        Self::new_with_url(document, viewport_width, viewport_height, "about:blank")
    }

    /// Creates a new JS runtime initialized with the target page URL.
    pub fn new_with_url(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
    ) -> Self {
        Self::new_with_url_and_storage(
            document,
            viewport_width,
            viewport_height,
            page_url,
            Arc::new(Mutex::new(HashMap::new())),
        )
    }

    /// Creates a new JS runtime with a shared localStorage backing store.
    ///
    /// Pass a pre-populated map to restore localStorage across page loads.
    pub fn new_with_url_and_storage(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
    ) -> Self {
        Self::new_with_url_storage_and_cookies(
            document,
            viewport_width,
            viewport_height,
            page_url,
            local_storage,
            Arc::new(Mutex::new(mango_net::CookieJar::new())),
        )
    }

    /// Creates a new JS runtime with shared localStorage **and** a shared cookie jar.
    ///
    /// The browser owns the cookie jar so it can be flushed to the profile file;
    /// sharing it means `document.cookie` writes are picked up by the very next
    /// `HttpClient` request without an explicit copy step.
    pub fn new_with_url_storage_and_cookies(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
        cookie_jar: dom_bindings::CookieStore,
    ) -> Self {
        Self::new_with_url_storage_session_and_cookies(
            document,
            viewport_width,
            viewport_height,
            page_url,
            local_storage,
            Arc::new(Mutex::new(HashMap::new())),
            cookie_jar,
        )
    }

    /// Creates a new JS runtime with shared localStorage, tab-scoped sessionStorage, and cookie jar.
    pub fn new_with_url_storage_session_and_cookies(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
        session_storage: web_apis::SharedSessionStorage,
        cookie_jar: dom_bindings::CookieStore,
    ) -> Self {
        Self::new_with_full_context(
            document,
            viewport_width,
            viewport_height,
            page_url,
            local_storage,
            session_storage,
            cookie_jar,
            None,
        )
    }

    /// Creates a JS runtime with shared caches, storage, and preloaded background resources.
    pub fn new_with_full_context(
        document: Document,
        viewport_width: f32,
        viewport_height: f32,
        page_url: &str,
        local_storage: SharedLocalStorage,
        session_storage: web_apis::SharedSessionStorage,
        cookie_jar: dom_bindings::CookieStore,
        resource_cache: Option<Arc<mango_net::ResourceCache>>,
    ) -> Self {
        ensure_tokio_runtime();

        let shared_doc: SharedDocument = Rc::new(RefCell::new(document));
        let dom_dirty: DomDirtyFlag = Rc::new(RefCell::new(false));
        let event_loop: SharedEventLoop = Rc::new(RefCell::new(EventLoop::new()));
        let console_buffer = console::new_console_buffer();
        let status_text: SharedStatusText = Rc::new(RefCell::new(String::new()));
        let pending_nav: web_apis::SharedPendingNav = Rc::new(RefCell::new(None));
        let pending_open: web_apis::SharedPendingOpenWindow = Rc::new(RefCell::new(None));
        let layout_bounds: dom_bindings::LayoutBounds =
            Rc::new(RefCell::new(std::collections::HashMap::new()));
        let worker_manager = crate::worker::WorkerManager::new();
        let ws_manager = mango_net::WebSocketManager::new();
        let broadcast_channel_hub = crate::broadcast_channel::BroadcastChannelHub::new();
        let shared_worker_hub = crate::shared_worker::SharedWorkerHub::new();
        let web_locks_engine = crate::web_locks::WebLocksEngine::new();
        let import_maps = Arc::new(Mutex::new(HashMap::new()));

        static V8_INIT: std::sync::Once = std::sync::Once::new();
        V8_INIT.call_once(|| {
            deno_core::v8::V8::set_flags_from_string(
                "--max-old-space-size=1024 --optimize-for-size --expose-gc",
            );
        });

        let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
            extensions: vec![crate::ops::init_extension()],
            // A page isolate is a bounded execution resource. Without an
            // isolate heap ceiling, a broken/unsupported module graph can
            // grow V8 past the browser process's memory budget before Mango
            // gets a chance to reclaim anything.
            create_params: Some(
                deno_core::v8::Isolate::create_params().heap_limits(0, 768 * 1024 * 1024),
            ),
            module_loader: Some(Rc::new(
                crate::runtime::module_loader::MangoModuleLoader::with_resource_cache(
                    page_url,
                    import_maps.clone(),
                    resource_cache,
                ),
            )),
            ..Default::default()
        });

        {
            let op_state_rc = runtime.op_state();
            let mut state = op_state_rc.borrow_mut();
            state.put(shared_doc.clone());
            state.put(dom_dirty.clone());
            state.put(event_loop.clone());
            state.put(console_buffer.clone());
            state.put(status_text.clone());
            state.put(pending_nav.clone());
            state.put(pending_open.clone());
            state.put(crate::ops::web::OpLocalStorage(local_storage.clone()));
            state.put(crate::ops::web::OpSessionStorage(session_storage.clone()));
            state.put(layout_bounds.clone());
            state.put(cookie_jar.clone());
            state.put(worker_manager.clone());
            state.put(ws_manager.clone());
            state.put(broadcast_channel_hub.clone());
            state.put(shared_worker_hub.clone());
            state.put(web_locks_engine.clone());
            state.put(crate::ops::dom::PageUrl(page_url.to_string()));
        }

        // Install prelude, DOM shim, and Web APIs shim. `JsRuntime::new`
        // leaves the isolate entered; keep that balanced even on panic.
        {
            let _isolate_guard = IsolateExitGuard::new(runtime.v8_isolate());
            if let Err(e) = runtime.execute_script("<prelude>", crate::ops::PRELUDE_JS) {
                eprintln!("Failed to install Mango prelude: {e}");
                panic!("Failed to install Mango prelude: {e}");
            }
            if let Err(e) = runtime.execute_script("<dom_shim>", crate::dom_bindings::DOM_SHIM) {
                eprintln!("Failed to install Mango DOM shim: {e}");
                panic!("Failed to install Mango DOM shim: {e}");
            }
            let web_shim = crate::web_apis::build_web_apis_shim(page_url);
            if let Err(e) = runtime.execute_script("<web_apis>", web_shim) {
                eprintln!("Failed to install Mango Web APIs shim: {e}");
                panic!("Failed to install Mango Web APIs shim: {e}");
            }

            // Set viewport width and height
            let _ = runtime.execute_script(
                "<viewport>",
                format!(
                    "globalThis.innerWidth = {viewport_width}; globalThis.innerHeight = {viewport_height}; if (typeof globalThis.window !== 'undefined') {{ globalThis.window.innerWidth = {viewport_width}; globalThis.window.innerHeight = {viewport_height}; }}"
                ),
            );
        }

        Self {
            runtime,
            event_loop,
            console_buffer,
            document: shared_doc,
            dom_dirty,
            status_text,
            pending_nav,
            pending_open,
            local_storage,
            session_storage,
            layout_bounds,
            cookie_jar,
            hovered_nodes: std::collections::HashSet::new(),
            active_nodes: std::collections::HashSet::new(),
            worker_manager,
            ws_manager,
            broadcast_channel_hub,
            shared_worker_hub,
            web_locks_engine,
            import_maps,
            execution_disabled: false,
        }
    }

    /// Registers mappings from a `<script type="importmap">` JSON string.
    pub fn register_import_map(&mut self, json_str: &str) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(imports) = val.get("imports").and_then(|v| v.as_object()) {
                if let Ok(mut map) = self.import_maps.lock() {
                    for (k, v) in imports {
                        if let Some(target) = v.as_str() {
                            map.insert(k.clone(), target.to_string());
                        }
                    }
                }
            }
        }
    }

    /// Returns the Web Worker manager handle.
    pub fn worker_manager(&self) -> crate::worker::WorkerManager {
        self.worker_manager.clone()
    }

    /// Returns the WebSocket manager handle.
    pub fn ws_manager(&self) -> mango_net::WebSocketManager {
        self.ws_manager.clone()
    }

    /// Returns the BroadcastChannel hub handle.
    pub fn broadcast_channel_hub(&self) -> crate::broadcast_channel::BroadcastChannelHub {
        self.broadcast_channel_hub.clone()
    }

    /// Returns the SharedWorker hub handle.
    pub fn shared_worker_hub(&self) -> crate::shared_worker::SharedWorkerHub {
        self.shared_worker_hub.clone()
    }

    /// Returns the WebLocks engine handle.
    pub fn web_locks_engine(&self) -> crate::web_locks::WebLocksEngine {
        self.web_locks_engine.clone()
    }
}

impl Drop for JsRuntime {
    fn drop(&mut self) {
        self.clear_timers();
        // Canvas backing stores are process-global; release every store owned
        // by this document even if the browser did not navigate away first.
        if let Ok(doc) = self.document.try_borrow() {
            mango_render::remove_canvases_for_document(doc.id());
        }
        self.worker_manager.terminate_all();
        unsafe {
            let isolate = self.runtime.v8_isolate();
            isolate.enter();
            isolate.low_memory_notification();
        }
    }
}
