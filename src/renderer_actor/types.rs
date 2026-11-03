use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use mango_core::Color;
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::dom::{Document, NodeId};
use mango_ipc::{IpcEndpoint, IpcMessage};
use mango_layout::box_tree::LayoutBox;
use mango_net::{CookieJar, ResourceCache, Url};
use mango_platform::input::{KeyEvent, KeyState, MouseButton};
use mango_render::display_list::DisplayList;

use crate::browser::types::ScriptToRun;

/// Payload for initializing and loading a document into the isolated renderer.
pub struct LoadDocumentPayload {
    pub url: String,
    pub doc: Document,
    pub html: String,
    pub stylesheets: Vec<Stylesheet>,
    pub preloaded_scripts: Vec<ScriptToRun>,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub base_url: Option<Url>,
    pub generation: u64,
    pub local_storage: mango_js::web_apis::SharedLocalStorage,
    pub session_storage: mango_js::web_apis::SharedSessionStorage,
    pub cookie_jar: Arc<std::sync::Mutex<CookieJar>>,
    pub cache: Option<Arc<ResourceCache>>,
}

/// Commands sent from the Browser UI Process -> Isolated Renderer Actor.
pub enum RendererCommand {
    /// Load a new document into this renderer process.
    Load(Box<LoadDocumentPayload>),
    /// Execute an arbitrary JavaScript string.
    ExecuteScript { code: String },
    /// Asynchronous notification of compositor scroll position for window.scrollY / scroll events.
    ScrollOffset { scroll_y: f32 },
    /// Viewport size change.
    ResizeViewport { width: f32, height: f32 },
    /// Input click event routing.
    InputClick {
        x: f32,
        y: f32,
        button: MouseButton,
        state: KeyState,
    },
    /// Keyboard event routing.
    InputKey { event: KeyEvent },
    /// Cleanly terminate the isolated renderer process.
    Shutdown,
}

/// Rendered frame payload streamed from Isolated Renderer Actor -> Browser UI Process.
pub struct RenderFramePayload {
    pub frame_id: u64,
    pub generation: u64,
    pub display_list: DisplayList,
    pub scrollable_height: f32,
    pub title: Option<String>,
    pub canvas_bg: Color,
    pub root_box: Option<LayoutBox>,
    pub style_snapshot: HashMap<u32, Arc<ComputedStyle>>,
    pub focused_control: Option<NodeId>,
    pub doc_mirror: Option<Document>,
}

/// Results emitted from Isolated Renderer Actor -> Browser UI Process.
pub enum RendererResult {
    /// Rendered layout frame ready for GPU compositor presentation.
    FrameReady(Box<RenderFramePayload>),
    /// Page title was modified by script (e.g. `document.title = ...`).
    TitleChanged(String),
    /// Script requested `window.open(url, features)`.
    OpenWindow { url: String, features: String },
    /// JS console log message.
    ConsoleMessage {
        level: crate::devtools::ConsoleLevel,
        text: String,
    },
    /// Navigation initiated by script (e.g. `window.location.href = ...`).
    NavigationRequested { url: String },
    /// Non-fatal script or execution warning.
    Error { message: String },
}

/// Handle to an isolated Out-of-Process Renderer managed by the Browser Process.
pub struct RendererActorHandle {
    pub process_id: u32,
    pub(crate) cmd_tx: Sender<RendererCommand>,
    pub result_rx: Receiver<RendererResult>,
    pub ipc_endpoint: IpcEndpoint,
    pub(crate) is_busy: Arc<AtomicBool>,
    pub(crate) worker: Option<JoinHandle<()>>,
}

impl RendererActorHandle {
    /// Spawns a dedicated OS thread running the sandboxed Renderer process communicating via IPC.
    pub fn spawn(process_id: u32) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<RendererCommand>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<RendererResult>();
        let (browser_ep, renderer_ep) = IpcEndpoint::create_pair();
        let is_busy = Arc::new(AtomicBool::new(false));
        let worker_busy = is_busy.clone();

        let thread_name = format!("mango-renderer-{process_id}");
        let worker = std::thread::Builder::new()
            .name(thread_name)
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                super::worker::run_renderer_loop(
                    process_id,
                    cmd_rx,
                    result_tx,
                    renderer_ep,
                    worker_busy,
                );
            })
            .expect("failed to spawn isolated renderer thread");

        Self {
            process_id,
            cmd_tx,
            result_rx,
            ipc_endpoint: browser_ep,
            is_busy,
            worker: Some(worker),
        }
    }

    /// Dispatches a document load request to the isolated renderer.
    pub fn load_document(&self, payload: LoadDocumentPayload) {
        let _ = self.cmd_tx.send(RendererCommand::Load(Box::new(payload)));
    }

    /// Dispatches a script execution request to the isolated renderer.
    pub fn execute_script(&self, code: String) {
        let _ = self.cmd_tx.send(RendererCommand::ExecuteScript { code });
    }

    /// Updates the compositor scroll offset asynchronously without blocking the UI thread.
    pub fn update_scroll(&self, scroll_y: f32) {
        let _ = self.cmd_tx.send(RendererCommand::ScrollOffset { scroll_y });
        let _ = self
            .ipc_endpoint
            .send(&IpcMessage::ScrollOffset { scroll_y });
    }

    /// Notifies the isolated renderer of viewport dimensions change.
    pub fn resize(&self, width: f32, height: f32) {
        let _ = self
            .cmd_tx
            .send(RendererCommand::ResizeViewport { width, height });
        let _ = self
            .ipc_endpoint
            .send(&IpcMessage::ViewportResize { width, height });
    }

    /// Dispatches an input click event to the isolated renderer.
    pub fn input_click(&self, x: f32, y: f32, button: MouseButton, state: KeyState) {
        let _ = self.cmd_tx.send(RendererCommand::InputClick {
            x,
            y,
            button,
            state,
        });
    }

    /// Dispatches a keyboard event to the isolated renderer.
    pub fn input_key(&self, event: KeyEvent) {
        let _ = self.cmd_tx.send(RendererCommand::InputKey { event });
    }

    /// True if the renderer thread is currently executing JavaScript or computing layout.
    pub fn is_busy(&self) -> bool {
        self.is_busy.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Gracefully terminates the isolated renderer process.
    pub fn shutdown(&mut self) {
        let _ = self.cmd_tx.send(RendererCommand::Shutdown);
        self.ipc_endpoint.close();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for RendererActorHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Helper function to calculate proportional watchdog timeouts for script execution.
/// Aligns with Chromium's 30-second unresponsive execution watchdog.
pub fn script_timeout(code_len: usize) -> Duration {
    let extra_secs = (code_len / 1_000_000) as u64 * 5;
    Duration::from_secs(30 + extra_secs).min(Duration::from_secs(60))
}
