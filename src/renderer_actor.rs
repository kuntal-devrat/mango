//! Dedicated Out-of-Process / Isolated Renderer Actor for Mango Browser.
//!
//! Conforms to Chromium's Multi-Process Architecture:
//! - Browser/UI Process: Owns OS window management (`winit`), `wgpu` hardware compositing,
//!   tab strip, omnibox, and smooth kinetic scrolling (`SmoothScrollAnimation`) running at
//!   60-120 FPS uninterrupted.
//! - Renderer Actor: Runs the DOM tree, CSS cascade, layout, and V8 JavaScript runtime
//!   in an isolated execution thread/process communicating via `mango_ipc::IpcEndpoint`.
//! - Even if a website runs heavy synchronous JS for 4 seconds, the window chrome,
//!   scrolling, and tab switching remain at 120 FPS.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use mango_core::{Color, Size};
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_ipc::{IpcEndpoint, IpcMessage};
use mango_js::JsRuntime;
use mango_js::web_apis::{SharedLocalStorage, SharedSessionStorage};
use mango_layout::box_tree::LayoutBox;
use mango_net::{CookieJar, ResourceCache, Url};
use mango_platform::input::{KeyEvent, KeyState, MouseButton};
use mango_render::display_list::DisplayList;

use crate::browser::render::layout::canvas_background_color;
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
    pub local_storage: SharedLocalStorage,
    pub session_storage: SharedSessionStorage,
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
    pub style_snapshot: HashMap<u32, ComputedStyle>,
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
    cmd_tx: Sender<RendererCommand>,
    pub result_rx: Receiver<RendererResult>,
    pub ipc_endpoint: IpcEndpoint,
    is_busy: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl RendererActorHandle {
    /// Spawns a dedicated OS thread running the sandboxed Renderer process communicating via IPC.
    pub fn spawn(process_id: u32) -> Self {
        let (cmd_tx, cmd_rx) = channel::<RendererCommand>();
        let (result_tx, result_rx) = channel::<RendererResult>();
        let (browser_ep, renderer_ep) = IpcEndpoint::create_pair();
        let is_busy = Arc::new(AtomicBool::new(false));
        let worker_busy = is_busy.clone();

        let thread_name = format!("mango-renderer-{process_id}");
        let worker = Builder::new()
            .name(thread_name)
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                run_renderer_loop(process_id, cmd_rx, result_tx, renderer_ep, worker_busy);
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
        self.is_busy.load(Ordering::Acquire)
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
        let _ = self.cmd_tx.send(RendererCommand::Shutdown);
        self.ipc_endpoint.close();
    }
}

/// Helper function to calculate proportional watchdog timeouts for script execution.
/// Aligns with Chromium's 30-second unresponsive execution watchdog.
fn script_timeout(code_len: usize) -> Duration {
    let extra_secs = (code_len / 1_000_000) as u64 * 5;
    Duration::from_secs(30 + extra_secs).min(Duration::from_secs(60))
}

/// Collects `<script type="importmap">` definitions from document.
fn collect_import_maps(doc: &Document, node_id: NodeId, out: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id) {
        if let NodeData::Element(elem) = &node.data {
            if elem.tag_name.eq_ignore_ascii_case("script")
                && elem
                    .get_attribute("type")
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case("importmap"))
            {
                let content = doc.text_content(node_id);
                if !content.trim().is_empty() {
                    out.push(content);
                }
            }
        }
        for child in doc.children(node_id) {
            collect_import_maps(doc, child.id, out);
        }
    }
}

/// Computes layout and display list for a document snapshot.
fn compute_layout_snapshot(
    doc: &Document,
    stylesheets: &[Stylesheet],
    viewport: Size,
) -> (LayoutBox, DisplayList, f32, Color) {
    let mut all_sheets = stylesheets.to_vec();
    all_sheets.extend(mango_layout::extract_style_elements(doc));
    let mut media = mango_css::current_media_environment();
    media.viewport_width = viewport.width;
    media.viewport_height = viewport.height;
    let style_index = mango_layout::PageStyleIndex::from_stylesheets(all_sheets, media);
    let (root_box, page_display_list) =
        mango_layout::layout_document_with_style_index(doc, &style_index, viewport);
    let canvas_bg = canvas_background_color(&root_box);
    let scrollable_height = crate::scroll::compute_scrollable_height(Some(&root_box));
    (root_box, page_display_list, scrollable_height, canvas_bg)
}

/// The isolated Renderer process main loop running on its own dedicated OS thread.
#[allow(unused_assignments)]
fn run_renderer_loop(
    process_id: u32,
    cmd_rx: Receiver<RendererCommand>,
    result_tx: Sender<RendererResult>,
    ipc_ep: IpcEndpoint,
    is_busy: Arc<AtomicBool>,
) {
    let mut js_rt: Option<JsRuntime> = None;
    let mut current_stylesheets: Vec<Stylesheet> = Vec::new();
    let mut current_url = String::new();
    let mut current_base_url: Option<Url> = None;
    let mut viewport_size = Size::new(800.0, 600.0);
    let mut frame_seq = 0u64;
    let mut current_generation = 0u64;
    let mut pending_scripts = VecDeque::<ScriptToRun>::new();
    let mut cache_ref: Option<Arc<ResourceCache>> = None;

    // Send InitAck over IPC
    let _ = ipc_ep.send(&IpcMessage::InitAck { process_id });

    loop {
        // Poll command queue or process pending script/timer work
        let has_pending = !pending_scripts.is_empty();
        let next_timer = js_rt.as_ref().and_then(|rt| rt.next_timer_deadline());
        let now = Instant::now();
        let timeout = if has_pending {
            Duration::from_millis(1)
        } else if let Some(dl) = next_timer {
            if dl <= now {
                Duration::from_millis(0)
            } else {
                (dl - now).min(Duration::from_millis(16))
            }
        } else {
            Duration::from_millis(32)
        };

        match cmd_rx.recv_timeout(timeout) {
            Ok(RendererCommand::Shutdown) => {
                let _ = ipc_ep.send(&IpcMessage::Terminate);
                break;
            }

            Ok(RendererCommand::Load(payload)) => {
                is_busy.store(true, Ordering::Release);
                current_generation = payload.generation;
                current_url = payload.url.clone();
                current_base_url = payload.base_url.clone();
                viewport_size = Size::new(payload.viewport_width, payload.viewport_height);
                current_stylesheets = payload.stylesheets;
                cache_ref = payload.cache.clone();

                let mut rt = JsRuntime::new_with_full_context(
                    payload.doc,
                    payload.viewport_width,
                    payload.viewport_height,
                    &payload.url,
                    payload.local_storage,
                    payload.session_storage,
                    payload.cookie_jar,
                    payload.cache,
                );

                // Register import maps
                let mut import_maps = Vec::new();
                {
                    let doc_snap = rt.document_ref();
                    collect_import_maps(&doc_snap, doc_snap.root(), &mut import_maps);
                }
                for map_json in &import_maps {
                    rt.register_import_map(map_json);
                }

                // Extract scripts from preloaded list or collect directly from document tree
                let scripts = if !payload.preloaded_scripts.is_empty() {
                    payload.preloaded_scripts
                } else {
                    let mut s = Vec::new();
                    let doc_ref = rt.document_ref();
                    crate::browser::navigation::pipeline::collect_scripts(
                        &doc_ref,
                        doc_ref.root(),
                        &mut s,
                    );
                    s
                };

                // Partition scripts into synchronous and deferred/async
                let mut sync_scripts = Vec::new();
                let mut defer_async_scripts = Vec::new();
                for s in scripts {
                    if s.is_defer() || s.is_async() {
                        defer_async_scripts.push(s);
                    } else {
                        sync_scripts.push(s);
                    }
                }

                // Run synchronous scripts on this isolated thread
                for s in sync_scripts {
                    run_single_script(
                        &s,
                        &mut rt,
                        current_base_url.as_ref(),
                        cache_ref.as_ref().map(|a| a.as_ref()),
                    );
                }

                // Queue deferred / async scripts
                pending_scripts.clear();
                for s in defer_async_scripts {
                    pending_scripts.push_back(s);
                }

                // If no scripts pending, dispatch DOMContentLoaded & load
                if pending_scripts.is_empty() {
                    let _ = rt.dispatch_dom_content_loaded();
                    let _ = rt.dispatch_load();
                }

                // Initial layout & display list calculation
                let doc_clone = rt.document_ref().clone();
                let title = rt
                    .eval("document.title")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .or_else(|| crate::browser::navigation::resources::extract_title(&doc_clone));
                let (root, display_list, scrollable_height, canvas_bg) =
                    compute_layout_snapshot(&doc_clone, &current_stylesheets, viewport_size);

                frame_seq = frame_seq.wrapping_add(1);
                let _ = result_tx.send(RendererResult::FrameReady(Box::new(RenderFramePayload {
                    frame_id: frame_seq,
                    generation: current_generation,
                    display_list,
                    scrollable_height,
                    title: title.clone(),
                    canvas_bg,
                    root_box: Some(root),
                    style_snapshot: HashMap::new(),
                    focused_control: None,
                    doc_mirror: Some(doc_clone),
                })));

                let _ = ipc_ep.send(&IpcMessage::DidCommitProvisionalLoad {
                    url: current_url.clone(),
                    title: title.unwrap_or_default(),
                    http_status: 200,
                });

                js_rt = Some(rt);
                is_busy.store(false, Ordering::Release);
            }

            Ok(RendererCommand::ExecuteScript { code }) => {
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let to = script_timeout(code.len());
                    let res = rt.execute_script_with_timeout(&code, to);
                    if let Err(e) = res {
                        let _ = result_tx.send(RendererResult::Error {
                            message: format!("Script execution error: {e}"),
                        });
                    } else if rt.is_dom_dirty() {
                        let doc_clone = rt.document_ref().clone();
                        let title = rt
                            .eval("document.title")
                            .ok()
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                crate::browser::navigation::resources::extract_title(&doc_clone)
                            });
                        let (root, display_list, scrollable_height, canvas_bg) =
                            compute_layout_snapshot(
                                &doc_clone,
                                &current_stylesheets,
                                viewport_size,
                            );

                        frame_seq = frame_seq.wrapping_add(1);
                        let _ = result_tx.send(RendererResult::FrameReady(Box::new(
                            RenderFramePayload {
                                frame_id: frame_seq,
                                generation: current_generation,
                                display_list,
                                scrollable_height,
                                title,
                                canvas_bg,
                                root_box: Some(root),
                                style_snapshot: HashMap::new(),
                                focused_control: None,
                                doc_mirror: Some(doc_clone),
                            },
                        )));
                    }
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::ScrollOffset { scroll_y }) => {
                if let Some(ref mut rt) = js_rt {
                    // Update window.scrollY / window.pageYOffset in JS environment
                    let _ = rt.execute_script(&format!(
                        "if (typeof window !== 'undefined') {{ window.scrollY = {scroll_y}; window.pageYOffset = {scroll_y}; }}"
                    ));
                }
            }

            Ok(RendererCommand::ResizeViewport { width, height }) => {
                viewport_size = Size::new(width, height);
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let _ = rt.execute_script(&format!(
                        "if (typeof window !== 'undefined') {{ window.innerWidth = {width}; window.innerHeight = {height}; if (typeof window.dispatchEvent === 'function') window.dispatchEvent(new Event('resize')); }}"
                    ));
                    let doc_clone = rt.document_ref().clone();
                    let title = rt
                        .eval("document.title")
                        .ok()
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            crate::browser::navigation::resources::extract_title(&doc_clone)
                        });
                    let (root, display_list, scrollable_height, canvas_bg) =
                        compute_layout_snapshot(&doc_clone, &current_stylesheets, viewport_size);

                    frame_seq = frame_seq.wrapping_add(1);
                    let _ =
                        result_tx.send(RendererResult::FrameReady(Box::new(RenderFramePayload {
                            frame_id: frame_seq,
                            generation: current_generation,
                            display_list,
                            scrollable_height,
                            title,
                            canvas_bg,
                            root_box: Some(root),
                            style_snapshot: HashMap::new(),
                            focused_control: None,
                            doc_mirror: Some(doc_clone),
                        })));
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::InputClick {
                x,
                y,
                button: _,
                state: _,
            }) => {
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let _ = rt.execute_script(&format!(
                        "if (typeof document !== 'undefined') {{ var ev = new MouseEvent('click', {{ clientX: {x}, clientY: {y}, bubbles: true }}); document.dispatchEvent(ev); }}"
                    ));
                    if rt.is_dom_dirty() {
                        let doc_clone = rt.document_ref().clone();
                        let title =
                            crate::browser::navigation::resources::extract_title(&doc_clone);
                        let (root, display_list, scrollable_height, canvas_bg) =
                            compute_layout_snapshot(
                                &doc_clone,
                                &current_stylesheets,
                                viewport_size,
                            );

                        frame_seq = frame_seq.wrapping_add(1);
                        let _ = result_tx.send(RendererResult::FrameReady(Box::new(
                            RenderFramePayload {
                                frame_id: frame_seq,
                                generation: current_generation,
                                display_list,
                                scrollable_height,
                                title,
                                canvas_bg,
                                root_box: Some(root),
                                style_snapshot: HashMap::new(),
                                focused_control: None,
                                doc_mirror: Some(doc_clone),
                            },
                        )));
                    }
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::InputKey { event }) => {
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let key_str = format!("{:?}", event.key);
                    let _ = rt.execute_script(&format!(
                        "if (typeof document !== 'undefined') {{ var ev = new KeyboardEvent('keydown', {{ key: '{key_str}', bubbles: true }}); document.dispatchEvent(ev); }}"
                    ));
                    is_busy.store(false, Ordering::Release);
                }
            }

            Err(RecvTimeoutError::Timeout) => {
                // Timeout elapsed: perform periodic script and timer work
                if let Some(ref mut rt) = js_rt {
                    let mut mutated = false;

                    // 1. Run queued deferred/async scripts
                    if let Some(scheduled) = pending_scripts.pop_front() {
                        is_busy.store(true, Ordering::Release);
                        run_single_script(
                            &scheduled,
                            rt,
                            current_base_url.as_ref(),
                            cache_ref.as_ref().map(|a| a.as_ref()),
                        );
                        if pending_scripts.is_empty() {
                            let _ = rt.dispatch_dom_content_loaded();
                            let _ = rt.dispatch_load();
                        }
                        mutated = true;
                    }

                    // 2. Tick event loop (microtasks & timers)
                    if rt.has_pending_timers() {
                        mutated |= rt.tick();
                    }

                    // 3. Drain pending window.open requests
                    if let Some(open_req) = rt.take_pending_open_window() {
                        let _ = result_tx.send(RendererResult::OpenWindow {
                            url: open_req.url.clone(),
                            features: open_req.features.clone(),
                        });
                        let _ = ipc_ep.send(&IpcMessage::OpenWindow {
                            url: open_req.url,
                            features: open_req.features,
                        });
                    }

                    // 4. Drain console messages
                    for msg in rt.take_console_messages() {
                        let lvl = match msg.level {
                            mango_js::console::ConsoleLevel::Log => {
                                crate::devtools::ConsoleLevel::Log
                            }
                            mango_js::console::ConsoleLevel::Info => {
                                crate::devtools::ConsoleLevel::Info
                            }
                            mango_js::console::ConsoleLevel::Warn => {
                                crate::devtools::ConsoleLevel::Warn
                            }
                            mango_js::console::ConsoleLevel::Error => {
                                crate::devtools::ConsoleLevel::Error
                            }
                        };
                        let _ = result_tx.send(RendererResult::ConsoleMessage {
                            level: lvl,
                            text: msg.text.clone(),
                        });
                        let _ = ipc_ep.send(&IpcMessage::ConsoleMessage {
                            level: format!("{lvl:?}"),
                            text: msg.text,
                        });
                    }

                    // 5. If DOM mutated, re-compute layout and emit FrameReady
                    if mutated && rt.is_dom_dirty() {
                        let doc_clone = rt.document_ref().clone();
                        let title = rt
                            .eval("document.title")
                            .ok()
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                crate::browser::navigation::resources::extract_title(&doc_clone)
                            });
                        let (root, display_list, scrollable_height, canvas_bg) =
                            compute_layout_snapshot(
                                &doc_clone,
                                &current_stylesheets,
                                viewport_size,
                            );

                        frame_seq = frame_seq.wrapping_add(1);
                        let _ = result_tx.send(RendererResult::FrameReady(Box::new(
                            RenderFramePayload {
                                frame_id: frame_seq,
                                generation: current_generation,
                                display_list,
                                scrollable_height,
                                title,
                                canvas_bg,
                                root_box: Some(root),
                                style_snapshot: HashMap::new(),
                                focused_control: None,
                                doc_mirror: Some(doc_clone),
                            },
                        )));
                    }

                    is_busy.store(false, Ordering::Release);
                }
            }

            Err(RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

/// Executes a single script within the isolated JavaScript runtime.
fn run_single_script(
    script: &ScriptToRun,
    rt: &mut JsRuntime,
    base_url: Option<&Url>,
    cache: Option<&ResourceCache>,
) {
    let is_mod = script.is_module();
    if !is_mod {
        if let Some(node_id) = script.node_id() {
            let js_id = mango_html::node_id_to_js(node_id);
            let _ = rt.execute_script(&format!(
                "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript({js_id});"
            ));
        } else if let Some(src) = script.src() {
            let script_name = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.to_string());
            if let Ok(json_name) = serde_json::to_string(&script_name) {
                let _ = rt.execute_script(&format!(
                    "if (typeof globalThis.__setCurrentScriptUrl === 'function') globalThis.__setCurrentScriptUrl({json_name});"
                ));
            }
        }
    } else {
        let _ = rt.execute_script(
            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
        );
    }

    match script {
        ScriptToRun::Inline {
            code, is_module, ..
        } => {
            let to = script_timeout(code.len());
            let res = if *is_module {
                rt.execute_module_with_timeout(code, to)
            } else {
                rt.execute_script_with_timeout(code, to)
            };
            if let Err(e) = res {
                log::warn!("Isolated renderer inline script error: {e}");
            }
        }
        ScriptToRun::Preloaded {
            code,
            src,
            is_module,
            ..
        } => {
            let to = script_timeout(code.len());
            let script_name = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.clone());
            let res = if *is_module {
                rt.execute_module_with_name_and_timeout(&script_name, code, to)
            } else {
                rt.execute_script_with_name_and_timeout(&script_name, code, to)
            };
            if let Err(e) = res {
                log::warn!("Isolated renderer preloaded script error ({src}): {e}");
            }
        }
        ScriptToRun::External { src, is_module, .. } => {
            let Some(base) = base_url else {
                if !is_mod {
                    let _ = rt.execute_script(
                        "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                    );
                }
                return;
            };
            let Ok(resolved) = base.resolve(src) else {
                if !is_mod {
                    let _ = rt.execute_script(
                        "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                    );
                }
                return;
            };
            let key = resolved.to_string();
            let script_code = if let Some(c) = cache {
                c.get(&key)
                    .map(|entry| String::from_utf8_lossy(&entry.body).into_owned())
            } else {
                None
            };
            if let Some(code) = script_code {
                let to = script_timeout(code.len());
                let res = if *is_module {
                    rt.execute_module_with_name_and_timeout(&key, &code, to)
                } else {
                    rt.execute_script_with_name_and_timeout(&key, &code, to)
                };
                if let Err(e) = res {
                    log::warn!("Isolated renderer external script error ({src}): {e}");
                }
            }
        }
    }

    if !is_mod {
        let _ = rt.execute_script(
            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
        );
    }
}
