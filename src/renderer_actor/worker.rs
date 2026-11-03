use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use mango_core::Size;
use mango_css::Stylesheet;
use mango_ipc::{IpcEndpoint, IpcMessage};
use mango_js::JsRuntime;
use mango_net::{ResourceCache, Url};

use super::incremental::{
    IncrementalRendererState, collect_import_maps, drain_renderer_js_effects, emit_frame,
};
use super::script::run_single_script;
use super::types::{RendererCommand, RendererResult, script_timeout};
use crate::browser::types::ScriptToRun;

/// The isolated Renderer process main loop running on its own dedicated OS thread.
#[allow(unused_assignments)]
pub fn run_renderer_loop(
    _process_id: u32,
    cmd_rx: Receiver<RendererCommand>,
    result_tx: Sender<RendererResult>,
    ipc_ep: IpcEndpoint,
    is_busy: Arc<AtomicBool>,
) {
    let (mut js_rt, mut current_stylesheets) = (None::<JsRuntime>, Vec::<Stylesheet>::new());
    let (mut current_url, mut current_base_url) = (String::new(), None::<Url>);
    let (mut frame_seq, mut current_generation) = (0u64, 0u64);
    let mut viewport_size = Size::new(800.0, 600.0);
    let mut pending_scripts = VecDeque::<ScriptToRun>::new();
    let mut cache_ref: Option<Arc<ResourceCache>> = None;
    let mut incremental_state = IncrementalRendererState::new();

    loop {
        let timeout = if !pending_scripts.is_empty() {
            Duration::from_millis(1)
        } else if let Some(dl) = js_rt.as_ref().and_then(|rt| rt.next_timer_deadline()) {
            let now = Instant::now();
            if dl <= now {
                Duration::from_millis(1)
            } else {
                (dl - now).clamp(Duration::from_millis(1), Duration::from_millis(16))
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
                collect_import_maps(
                    &rt.document_ref(),
                    rt.document_ref().root(),
                    &mut import_maps,
                );
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
                let (defer_async_scripts, sync_scripts): (Vec<_>, Vec<_>) = scripts
                    .into_iter()
                    .partition(|s| s.is_defer() || s.is_async());

                // Run synchronous scripts on this isolated thread
                for s in sync_scripts {
                    run_single_script(
                        &s,
                        &mut rt,
                        current_base_url.as_ref(),
                        cache_ref.as_ref().map(|a| a.as_ref()),
                    );
                }

                drain_renderer_js_effects(&mut rt, current_base_url.as_ref(), &result_tx, &ipc_ep);
                pending_scripts = defer_async_scripts.into();

                // If no scripts pending, dispatch DOMContentLoaded & load
                if pending_scripts.is_empty() {
                    let _ = rt.dispatch_dom_content_loaded();
                    let _ = rt.dispatch_load();
                    drain_renderer_js_effects(
                        &mut rt,
                        current_base_url.as_ref(),
                        &result_tx,
                        &ipc_ep,
                    );
                }

                // Initial layout & display list calculation
                incremental_state = IncrementalRendererState::new();
                let title = emit_frame(
                    &mut rt,
                    &mut incremental_state,
                    &current_stylesheets,
                    viewport_size,
                    current_generation,
                    &mut frame_seq,
                    &result_tx,
                );

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
                    }
                    drain_renderer_js_effects(rt, current_base_url.as_ref(), &result_tx, &ipc_ep);
                    if rt.is_dom_dirty() {
                        emit_frame(
                            rt,
                            &mut incremental_state,
                            &current_stylesheets,
                            viewport_size,
                            current_generation,
                            &mut frame_seq,
                            &result_tx,
                        );
                    }
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::ScrollOffset { scroll_y }) => {
                if let Some(ref mut rt) = js_rt {
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
                    emit_frame(
                        rt,
                        &mut incremental_state,
                        &current_stylesheets,
                        viewport_size,
                        current_generation,
                        &mut frame_seq,
                        &result_tx,
                    );
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::InputClick {
                x,
                y,
                button,
                state,
            }) => {
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let btn_num = match button {
                        mango_platform::input::MouseButton::Left => 0,
                        mango_platform::input::MouseButton::Middle => 1,
                        mango_platform::input::MouseButton::Right => 2,
                        mango_platform::input::MouseButton::Other(_) => 0,
                    };
                    let ev_type = match state {
                        mango_platform::input::KeyState::Pressed => "mousedown",
                        mango_platform::input::KeyState::Released => "mouseup",
                    };
                    let _ = rt.execute_script(&format!(
                        "if (typeof document !== 'undefined') {{ var el = document.elementFromPoint({x}, {y}); if (el) {{ el.dispatchEvent(new MouseEvent('{ev_type}', {{ clientX: {x}, clientY: {y}, button: {btn_num}, bubbles: true, cancelable: true }})); if ('{ev_type}' === 'mouseup') {{ el.dispatchEvent(new MouseEvent('click', {{ clientX: {x}, clientY: {y}, button: {btn_num}, bubbles: true, cancelable: true }})); if (typeof el.click === 'function') el.click(); }} }} }}"
                    ));
                    drain_renderer_js_effects(rt, current_base_url.as_ref(), &result_tx, &ipc_ep);
                    if rt.is_dom_dirty() {
                        emit_frame(
                            rt,
                            &mut incremental_state,
                            &current_stylesheets,
                            viewport_size,
                            current_generation,
                            &mut frame_seq,
                            &result_tx,
                        );
                    }
                    is_busy.store(false, Ordering::Release);
                }
            }

            Ok(RendererCommand::InputKey { event }) => {
                if let Some(ref mut rt) = js_rt {
                    is_busy.store(true, Ordering::Release);
                    let key_json = serde_json::to_string(&event.key.to_w3c_key())
                        .unwrap_or_else(|_| "\"Unidentified\"".to_string());
                    let _ = rt.execute_script(&format!(
                        "if (typeof document !== 'undefined') {{ var ev = new KeyboardEvent('keydown', {{ key: {key_json}, bubbles: true, cancelable: true }}); document.dispatchEvent(ev); }}"
                    ));
                    drain_renderer_js_effects(rt, current_base_url.as_ref(), &result_tx, &ipc_ep);
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

                    // 3. Drain pending JS side-effects (navigation, window.open, console)
                    drain_renderer_js_effects(rt, current_base_url.as_ref(), &result_tx, &ipc_ep);

                    // 4. Periodic idle GC
                    rt.maybe_idle_gc();

                    // 5. If DOM mutated, re-compute layout and emit FrameReady
                    if mutated || rt.is_dom_dirty() {
                        emit_frame(
                            rt,
                            &mut incremental_state,
                            &current_stylesheets,
                            viewport_size,
                            current_generation,
                            &mut frame_seq,
                            &result_tx,
                        );
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
