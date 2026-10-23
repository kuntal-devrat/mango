//! Shared data types, constants, and UI thread safety guards for the browser.

use mango_core::Rect;
use mango_css::Stylesheet;
use mango_html::dom::Document;
use mango_net::Url;

// Maximum concurrent network fetches for any background fan-out.
pub const MAX_CONCURRENT_FETCHES: usize = 16;
// Per-fetch timeout for stylesheets, scripts, fonts, and images.
pub const SUBRESOURCE_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
// Bounded wait for synchronous navigation bridges (tests/headless).
pub const SYNC_NAVIGATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// UI-thread marker: the thread that constructed the first `BrowserChrome`.
pub static UI_THREAD_ID: std::sync::OnceLock<std::thread::ThreadId> = std::sync::OnceLock::new();

/// Records the calling thread as the UI thread. Called from `BrowserChrome::new`.
pub fn mark_ui_thread() {
    let _ = UI_THREAD_ID.set(std::thread::current().id());
}

/// Returns true when called on the recorded UI thread.
pub fn is_ui_thread() -> bool {
    UI_THREAD_ID
        .get()
        .is_some_and(|id| *id == std::thread::current().id())
}

/// Debug-only guard against blocking network fetches on the UI thread.
pub fn assert_not_on_ui_thread(context: &'static str) {
    if is_ui_thread() {
        log::warn!(
            "Blocking network fetch on UI thread at {context}; use the NavigationActor/background worker instead"
        );
        debug_assert!(
            false,
            "Blocking network fetch on UI thread at {context}: dispatch via NavigationActor"
        );
    }
}

static FETCH_POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();

fn get_fetch_pool() -> &'static rayon::ThreadPool {
    FETCH_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_CONCURRENT_FETCHES)
            .thread_name(|i| format!("mango-fetch-pool-{i}"))
            .build()
            .expect("failed to initialize fetch thread pool")
    })
}

/// Queues a background fetch/decode task on the bounded fetch pool.
pub fn spawn_fetch_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    get_fetch_pool().spawn(task);
}

/// Runs a blocking fetch closure bounded by `timeout` using a bounded thread pool.
pub fn fetch_with_timeout<T, E, F>(
    fetch: F,
    timeout: std::time::Duration,
) -> Result<Result<T, E>, String>
where
    T: Send + 'static,
    E: Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    get_fetch_pool().spawn(move || {
        let _ = tx.send(fetch());
    });
    rx.recv_timeout(timeout).map_err(|e| match e {
        std::sync::mpsc::RecvTimeoutError::Timeout => {
            format!("subresource fetch timed out after {:?}", timeout)
        }
        std::sync::mpsc::RecvTimeoutError::Disconnected => {
            "subresource fetch worker disconnected".to_string()
        }
    })
}

/// A borrowed view of the active DOM.
pub enum DocumentView<'a> {
    Runtime(std::cell::Ref<'a, Document>),
    Cached(&'a Document),
    Parsed(Box<Document>),
}

impl std::ops::Deref for DocumentView<'_> {
    type Target = Document;

    fn deref(&self) -> &Document {
        match self {
            DocumentView::Runtime(doc) => doc,
            DocumentView::Cached(doc) => doc,
            DocumentView::Parsed(doc) => doc,
        }
    }
}

/// Script execution candidate parsed from DOM or preload scanner.
#[derive(Debug, Clone)]
pub enum ScriptToRun {
    Inline {
        code: String,
        is_module: bool,
    },
    External {
        src: String,
        is_defer: bool,
        is_async: bool,
        is_module: bool,
        nonce: Option<String>,
    },
    Preloaded {
        src: String,
        code: String,
        is_defer: bool,
        is_async: bool,
        is_module: bool,
        nonce: Option<String>,
    },
}

impl ScriptToRun {
    pub fn is_defer(&self) -> bool {
        match self {
            ScriptToRun::External {
                is_defer,
                is_async,
                is_module,
                ..
            }
            | ScriptToRun::Preloaded {
                is_defer,
                is_async,
                is_module,
                ..
            } => *is_defer || (*is_module && !*is_async),
            ScriptToRun::Inline { is_module, .. } => *is_module,
        }
    }
    pub fn is_async(&self) -> bool {
        match self {
            ScriptToRun::External { is_async, .. } | ScriptToRun::Preloaded { is_async, .. } => {
                *is_async
            }
            _ => false,
        }
    }
    pub fn is_module(&self) -> bool {
        match self {
            ScriptToRun::Inline { is_module, .. }
            | ScriptToRun::External { is_module, .. }
            | ScriptToRun::Preloaded { is_module, .. } => *is_module,
        }
    }
}

/// Prepared document payload from background navigation pipeline.
pub struct PreparedDocument {
    pub url: Url,
    pub doc: Document,
    pub html: String,
    pub status: u16,
    pub content_type: String,
    pub encoding: mango_net::encoding::Encoding,
    pub cached_stylesheets: Vec<Stylesheet>,
    pub preloaded_scripts: Vec<ScriptToRun>,
    pub image_sources: Vec<String>,
}

/// Find-in-page state for interactive page search and highlighting.
#[derive(Debug, Clone, Default)]
pub struct FindInPage {
    pub active: bool,
    pub query: String,
    pub matches: Vec<Rect>,
    pub current_match: usize,
}
