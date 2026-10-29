//! Browser history traversal (back/forward), URL navigation, and HTML document loading.

use std::collections::HashMap;

use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_js::JsRuntime;
use mango_net::Url;
use mango_render::display_list::DisplayList;

use super::super::BrowserChrome;
use super::super::types::ScriptToRun;
use super::super::welcome::{preprocess_wikipedia_appearance_html, resolve_navigation_input};
use super::pipeline::{collect_scripts, load_imported_stylesheets_parallel};
use super::resources::{extract_title, load_web_fonts_async};
use crate::chrome_ui::*;
use crate::navigation::*;

impl BrowserChrome {
    /// Navigates the browser to the specified URL or input string.
    pub fn navigate(&mut self, url: &str) {
        self.address_text = url.to_string();
        self.cursor_pos = self.address_text.len();
        self.navigate_to_address(true);
    }

    /// Public method to load HTML into the active tab.
    #[allow(dead_code)]
    pub fn load_html(&mut self, html: String, url: String) {
        self.load_html_internal(html, url, true);
    }

    /// Internal method to load HTML, execute scripts, initialize JS runtime, and re-layout.
    pub(crate) fn load_html_internal(&mut self, html: String, url: String, push_history: bool) {
        self.load_html_with_metadata(html, url, push_history, None, None);
    }

    /// Internal method to load HTML with known encoding and content-type metadata.
    pub(crate) fn load_html_with_metadata(
        &mut self,
        mut html: String,
        url: String,
        push_history: bool,
        encoding: Option<&str>,
        content_type: Option<&str>,
    ) {
        preprocess_wikipedia_appearance_html(&mut html, &url);
        let mut doc = parse_html(&html);
        if let Some(enc) = encoding {
            doc.character_set = enc.to_string();
        }
        if let Some(ct) = content_type {
            doc.content_type = ct.to_string();
        }
        self.load_document_internal(doc, html, url, push_history, false);
    }

    /// Loads an already parsed Document into the browser without re-parsing HTML.
    pub(crate) fn load_document_internal(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
    ) {
        self.load_document_internal_with_scripts(
            doc,
            html,
            url,
            push_history,
            skip_embedded_sheets_and_fonts,
            None,
        );
    }

    /// Fast non-blocking path for asynchronous isolated renderer navigation.
    /// Updates browser chrome state, tab history, and resource scheduling
    /// without blocking the UI thread on duplicate V8 runtime creation,
    /// full CSS selector matching, or layout tree construction.
    pub(crate) fn apply_async_navigation_state(
        &mut self,
        doc: &Document,
        html: &str,
        url: &str,
        push_history: bool,
    ) {
        self.resource_scheduler.cancel_generation(
            crate::browser::resource_scheduler::RejectReason::SupersededNavigation,
        );
        let generation = self
            .image_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        self.resource_scheduler.begin_document(generation);
        self.style_index_cache = None;
        self.style_index_version = u64::MAX;
        self.style_index_viewport = (0, 0);
        self.current_html = html.to_string();
        self.address_text = url.to_string();
        self.cursor_pos = self.address_text.len();
        self.context_menu = None;
        self.select_dropdown = None;
        self.picker = None;
        self.validation_bubble = None;
        self.range_dragging = None;
        self.range_drag_start_value = None;
        self.focused_control = None;
        self.focused_initial_value = None;
        self.control_cursor_pos = 0;

        if let Some(title) = extract_title(doc) {
            self.page_title = title;
        } else {
            self.page_title = url.to_string();
        }
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if push_history {
                if let Some(ref root) = self.root_box
                    && root.descendant_count() <= 10_000
                {
                    let (w, h) = (
                        self.width as f32,
                        (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                    );
                    let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                        root,
                        mango_core::Size::new(w, h),
                    );
                    tab.store_bfcache_snapshot(Box::new(snapshot));
                }
                tab.push_history(url.to_string(), self.page_title.clone());
                self.scroll_y = 0.0;
                self.scroll_y_f64 = 0.0;
            } else {
                tab.url = url.to_string();
                tab.set_title(self.page_title.clone());
            }
        }

        if let Some(old_doc) = self.cached_document.take() {
            mango_render::remove_canvases_for_document(old_doc.id());
        }
        if let Some(rt) = &self.js_runtime {
            mango_render::remove_canvases_for_document(rt.document_ref().id());
        }
        self.root_box = None;
        self.page_display_list = DisplayList::new();
        self.display_list_cache.lock().unwrap().clear();
        self.layer_tree.clear_page_tiles();
        mango_render::prune_image_caches();
        if let Some(mut old_rt) = self.js_runtime.take() {
            old_rt.clear_timers();
        }

        self.cached_document = Some(doc.clone());
        self.base_url = Url::parse(url).ok();
        self.anim_state = mango_css::AnimationState::new();
        self.style_snapshot.clear();
        self.hovered_dom_nodes.clear();
        self.active_dom_nodes.clear();
        self.pending_scripts.clear();

        if let Ok(parsed) = Url::parse(url)
            && let Some(ref frag) = parsed.fragment
        {
            let frag_str = frag.clone();
            if let Some(ref mut d) = self.cached_document {
                d.set_target_id(Some(frag_str.clone()));
            }
            self.scroll_to_anchor_fragment(&frag_str);
        }
    }

    pub(crate) fn load_document_internal_with_scripts(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
        preloaded_scripts: Option<Vec<ScriptToRun>>,
    ) {
        self.resource_scheduler.cancel_generation(
            crate::browser::resource_scheduler::RejectReason::SupersededNavigation,
        );
        let generation = self
            .image_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        self.resource_scheduler.begin_document(generation);
        self.style_index_cache = None;
        self.style_index_version = u64::MAX;
        self.style_index_viewport = (0, 0);
        self.current_html = html;
        self.address_text = url.clone();
        self.cursor_pos = self.address_text.len();
        self.context_menu = None;
        self.select_dropdown = None;
        self.picker = None;
        self.validation_bubble = None;
        self.range_dragging = None;
        self.range_drag_start_value = None;
        self.focused_control = None;
        self.focused_initial_value = None;
        self.control_cursor_pos = 0;

        if let Some(title) = extract_title(&doc) {
            self.page_title = title;
        } else {
            self.page_title = url.clone();
        }
        if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
            if push_history {
                if let Some(ref root) = self.root_box
                    // Avoid retaining large GitHub/news pages in BFCache. The
                    // active document remains fully usable; only the frozen
                    // back/forward copy is omitted.
                    && root.descendant_count() <= 10_000
                {
                    let (w, h) = (
                        self.width as f32,
                        (self.height.saturating_sub(HEADER_HEIGHT as u32)) as f32,
                    );
                    let snapshot = mango_layout::LayoutSnapshot::from_layout_box(
                        root,
                        mango_core::Size::new(w, h),
                    );
                    tab.store_bfcache_snapshot(Box::new(snapshot));
                }
                tab.push_history(url.clone(), self.page_title.clone());
                self.scroll_y = 0.0;
                self.scroll_y_f64 = 0.0;
            } else {
                tab.url = url.clone();
                tab.set_title(self.page_title.clone());
            }
        }

        if let Some(old_doc) = self.cached_document.take() {
            mango_render::remove_canvases_for_document(old_doc.id());
        }
        if let Some(rt) = &self.js_runtime {
            mango_render::remove_canvases_for_document(rt.document_ref().id());
        }
        self.root_box = None;
        self.page_display_list = DisplayList::new();
        self.display_list_cache.lock().unwrap().clear();
        self.layer_tree.clear_page_tiles();
        mango_render::prune_image_caches();
        if let Some(mut old_rt) = self.js_runtime.take() {
            // Cancel callbacks before dropping the runtime. Otherwise queued
            // timer/task source strings can keep a large page alive until the
            // runtime's deferred cleanup runs.
            old_rt.clear_timers();
            old_rt.gc();
        }

        // ── Initialize JavaScript Runtime ──
        let w = self.width as f32;
        let content_h = (self.height as f32 - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let session_storage = self
            .tabs
            .get(self.active_tab_idx)
            .map(|t| t.session_storage.clone())
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())));
        let mut js_rt = JsRuntime::new_with_full_context(
            doc,
            w,
            content_h,
            &url,
            self.local_storage.clone(),
            session_storage,
            self.cookie_jar.clone(),
            Some(self.loader.cache_arc()),
        );

        // Register import maps if any are defined in <script type="importmap">
        let mut import_maps = Vec::new();
        {
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
            let doc_snap = js_rt.document_ref();
            collect_import_maps(&doc_snap, doc_snap.root(), &mut import_maps);
        }
        for map_json in &import_maps {
            js_rt.register_import_map(map_json);
        }

        // Load web fonts and imported stylesheets from embedded and cached stylesheets.
        // Stylesheet @imports are bounded (6 concurrent + timeout) and font
        // registration is fully asynchronous: the UI thread never blocks here.
        let embedded_sheets = mango_layout::extract_style_elements(&js_rt.document_ref());
        self.base_url = Url::parse(&url).ok();
        if !skip_embedded_sheets_and_fonts {
            self.cached_stylesheets.clear();
            if self.cached_stylesheets.is_empty()
                && let Some(base) = self.base_url.as_ref()
            {
                let mut visited = std::collections::HashSet::new();
                let mut pending_imports = Vec::new();
                for sheet in &embedded_sheets {
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import) = rule {
                            pending_imports.push((base.clone(), import.url.clone()));
                        }
                    }
                }
                load_imported_stylesheets_parallel(
                    &self.loader,
                    pending_imports,
                    &mut visited,
                    &mut self.cached_stylesheets,
                );
            }
            load_web_fonts_async(
                self.loader.clone(),
                self.base_url.clone(),
                self.cached_stylesheets.clone(),
            );
            load_web_fonts_async(
                self.loader.clone(),
                self.base_url.clone(),
                embedded_sheets.clone(),
            );
        }

        // Inject extension stylesheets
        let (ext_css, ext_js) = self
            .extension_manager
            .get_content_scripts_for_url(&url, crate::extensions::RunAt::DocumentEnd);
        for css in ext_css {
            let sheet = mango_css::parser::parse_stylesheet(&css);
            self.cached_stylesheets.push(sheet);
        }

        // Reset the animation runtime and load the page's @keyframes definitions.
        // Author sheets win over embedded ones, matching the cascade order used
        // for style resolution.
        self.anim_state = mango_css::AnimationState::new();
        self.style_snapshot.clear();
        self.hovered_dom_nodes.clear();
        self.active_dom_nodes.clear();
        self.anim_state
            .animations
            .set_keyframes(mango_css::collect_keyframes(
                embedded_sheets.iter().chain(self.cached_stylesheets.iter()),
            ));

        // CSS background images are loaded lazily from visible layout boxes;
        // do not decode every theme texture before the first paint.
        self.relayout();
        self.reprioritize_visible_images();

        // Extract and execute scripts in document order
        let scripts = if let Some(pre) = preloaded_scripts {
            pre
        } else {
            let mut s = Vec::new();
            let doc_ref = self.document_view();
            collect_scripts(&doc_ref, doc_ref.root(), &mut s);
            s
        };

        for script in &scripts {
            let kind = if script.is_module() {
                crate::browser::resource_scheduler::ResourceKind::Module
            } else {
                crate::browser::resource_scheduler::ResourceKind::Script
            };
            let priority = if script.is_defer() {
                crate::browser::resource_scheduler::ResourcePriority::Defer
            } else if script.is_async() {
                crate::browser::resource_scheduler::ResourcePriority::Async
            } else {
                crate::browser::resource_scheduler::ResourcePriority::Blocking
            };
            let url = match script {
                ScriptToRun::Inline { .. } => "inline://document".to_string(),
                ScriptToRun::External { src, .. } | ScriptToRun::Preloaded { src, .. } => {
                    src.clone()
                }
            };
            let id = self.resource_scheduler.discover(kind, priority, url);
            self.resource_scheduler.transition(
                id,
                crate::browser::resource_scheduler::ResourceState::Queued,
            );
        }

        // Separate scripts into sync, async, and defer per HTML spec (§4.12.1)
        let mut sync_scripts = Vec::new();
        let mut async_scripts = Vec::new();
        let mut defer_scripts = Vec::new();

        for s in scripts {
            if s.is_defer() {
                defer_scripts.push(s);
            } else if s.is_async() {
                async_scripts.push(s);
            } else {
                sync_scripts.push(s);
            }
        }

        let mut external_count = 0;
        // Non-blocking external scripts on the UI thread: serve from the
        // memory cache only. Uncached remote scripts were already preloaded on
        // the background pipeline (`prepare_document_pipeline`); blocking the
        // Non-blocking external scripts on the UI thread: serve from the
        // memory cache only. Uncached remote scripts were already preloaded on
        // the background pipeline (`prepare_document_pipeline`); blocking the
        // UI on a network fetch here would freeze the page.
        let script_timeout = |code_len: usize| -> std::time::Duration {
            let extra_secs = (code_len / 1_000_000) as u64 * 5;
            std::time::Duration::from_secs(30 + extra_secs).min(std::time::Duration::from_secs(60))
        };
        let mut run_script = |s: ScriptToRun, rt: &mut mango_js::JsRuntime, ext_cnt: &mut usize| {
            let is_mod = s.is_module();
            if !is_mod {
                if let Some(node_id) = s.node_id() {
                    let js_id = mango_html::node_id_to_js(node_id);
                    let _ = rt.execute_script(&format!(
                            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript({js_id});"
                        ));
                } else if let Some(src) = s.src() {
                    let script_name = self
                        .base_url
                        .as_ref()
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
            match s {
                ScriptToRun::Inline {
                    code, is_module, ..
                } => {
                    let to = script_timeout(code.len());
                    let res = if is_module {
                        rt.execute_module_with_timeout(&code, to)
                    } else {
                        rt.execute_script_with_timeout(&code, to)
                    };
                    if let Err(e) = res {
                        log::warn!("Inline script error: {e}");
                    }
                    self.resource_scheduler.finish_url(
                        "inline://document",
                        crate::browser::resource_scheduler::ResourceState::Completed,
                        None,
                        code.len(),
                    );
                }
                ScriptToRun::Preloaded {
                    code,
                    src,
                    is_module,
                    ..
                } => {
                    *ext_cnt += 1;
                    let to = script_timeout(code.len());
                    let script_name = self
                        .base_url
                        .as_ref()
                        .and_then(|b| b.resolve(&src).ok())
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| src.clone());
                    let res = if is_module {
                        rt.execute_module_with_name_and_timeout(&script_name, &code, to)
                    } else {
                        rt.execute_script_with_name_and_timeout(&script_name, &code, to)
                    };
                    if let Err(e) = res {
                        log::warn!("Preloaded script error ({src}): {e}");
                    }
                    self.resource_scheduler.finish_url(
                        &src,
                        crate::browser::resource_scheduler::ResourceState::Completed,
                        None,
                        code.len(),
                    );
                }
                ScriptToRun::External { src, is_module, .. } => {
                    let Some(ref base) = self.base_url else {
                        if !is_mod {
                            let _ = rt.execute_script(
                                    "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                                );
                        }
                        return;
                    };
                    let Ok(resolved) = base.resolve(&src) else {
                        if !is_mod {
                            let _ = rt.execute_script(
                                    "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                                );
                        }
                        return;
                    };
                    let key = resolved.as_str();
                    let script_code = if let Some(cached) = self.loader.cache().get(&key) {
                        Some(String::from_utf8_lossy(&cached.body).into_owned())
                    } else {
                        log::warn!("Resource failed: external script unavailable in cache: {src}");
                        self.devtools.log(
                            crate::devtools::ConsoleLevel::Error,
                            format!("Resource failed: external script unavailable in cache: {src}"),
                        );
                        self.resource_scheduler.finish_url(
                            &src,
                            crate::browser::resource_scheduler::ResourceState::Failed,
                            Some(crate::browser::resource_scheduler::RejectReason::NetworkFailure),
                            0,
                        );
                        None
                    };
                    if let Some(code) = script_code {
                        *ext_cnt += 1;
                        let to = script_timeout(code.len());
                        let res = if is_module {
                            rt.execute_module_with_name_and_timeout(&key, &code, to)
                        } else {
                            rt.execute_script_with_name_and_timeout(&key, &code, to)
                        };
                        if let Err(e) = res {
                            log::warn!("External script error ({src}): {e}");
                        }
                        self.resource_scheduler.finish_url(
                            &src,
                            crate::browser::resource_scheduler::ResourceState::Completed,
                            None,
                            code.len(),
                        );
                    }
                }
            };
            if !is_mod {
                let _ = rt.execute_script(
                        "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                    );
            }
        };

        self.pending_scripts.clear();

        if self.async_navigation {
            // Interactive UI mode: Queue ALL scripts for non-blocking incremental execution in tick_js.
            // This guarantees the UI thread renders the initial visual frame and flushes Wayland
            // events/pings IMMEDIATELY (<16ms) without locking up the OS event loop.
            for s in sync_scripts {
                self.enqueue_pending_script(s);
            }
            for s in defer_scripts {
                self.enqueue_pending_script(s);
            }
            for s in async_scripts {
                self.enqueue_pending_script(s);
            }
            for js_code in ext_js {
                self.enqueue_pending_script(ScriptToRun::Inline {
                    node_id: None,
                    code: js_code,
                    is_module: false,
                });
            }
        } else {
            // Synchronous / test mode: execute synchronous scripts inline
            for s in sync_scripts {
                run_script(s, &mut js_rt, &mut external_count);
            }
            for s in defer_scripts {
                self.enqueue_pending_script(s);
            }
            for s in async_scripts {
                self.enqueue_pending_script(s);
            }
            for js_code in ext_js {
                let to = script_timeout(js_code.len());
                let _ = js_rt.execute_script_with_timeout(&js_code, to);
            }
            // Dispatch DOMContentLoaded and window load if no scripts pending
            if self.pending_scripts.is_empty() {
                let _ = js_rt.dispatch_dom_content_loaded();
                let _ = js_rt.dispatch_load();
            }
        }

        self.js_runtime = Some(js_rt);

        // ── Autofocus on interactive form control if specified ──
        if !self.address_focused {
            fn find_autofocus(doc: &Document, nid: NodeId) -> Option<NodeId> {
                if let Some(node) = doc.get(nid) {
                    if let NodeData::Element(elem) = &node.data {
                        let tag = elem.tag_name.to_ascii_lowercase();
                        if (tag == "input"
                            || tag == "textarea"
                            || tag == "button"
                            || tag == "select")
                            && elem.get_attribute("autofocus").is_some()
                        {
                            return Some(nid);
                        }
                    }
                    for child in doc.children(nid) {
                        if let Some(found) = find_autofocus(doc, child.id) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            let autofocus_nid = {
                let doc_snap = self.document_view();
                find_autofocus(&doc_snap, doc_snap.root())
            };
            if let Some(autofocus_nid) = autofocus_nid {
                self.focused_control = Some(autofocus_nid);
                let current_val = if let Some(ref rt) = self.js_runtime {
                    rt.get_node_attribute(autofocus_nid, "value")
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                self.control_cursor_pos = current_val.chars().count();
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_node_attribute(autofocus_nid, "data-mango-focused", "true");
                    rt.set_node_attribute(
                        autofocus_nid,
                        "_mango_cursor_pos",
                        &self.control_cursor_pos.to_string(),
                    );
                }
            }
        }

        // Post-script relayout: only run if the JS runtime's DOM dirty bit was
        // set during script execution.  The pre-layout already ran above at line
        // ~1080; a second unconditional full layout wastes ~50ms on every page
        // load for pages with no script-driven DOM mutations.
        let needs_post_layout = self.js_runtime.as_ref().is_some_and(|rt| rt.is_dom_dirty());
        if needs_post_layout {
            self.relayout();
        }

        self.drain_js_effects();

        // If initial URL has an in-page anchor fragment, scroll to it
        if let Ok(parsed) = Url::parse(&url)
            && let Some(ref frag) = parsed.fragment
        {
            let frag_str = frag.clone();
            if let Some(ref mut doc) = self.cached_document {
                doc.set_target_id(Some(frag_str.clone()));
            }
            if let Some(ref mut rt) = self.js_runtime {
                rt.set_target_id(Some(frag_str.clone()));
            }
            self.scroll_to_anchor_fragment(&frag_str);
        }
    }

    /// Ticks the JavaScript event loop (executing setTimeout/setInterval).
    /// Navigates the current browser tab to the address in `self.address_text`.
    pub(crate) fn navigate_to_address(&mut self, push_history: bool) {
        self.is_all_selected = false;
        let mut input = self.address_text.trim().to_string();
        if let Some(unwrapped) =
            crate::browser::navigation::pipeline::unwrap_tracking_redirect(&input)
        {
            log::info!("Unwrapped tracking redirect: {} -> {}", input, unwrapped);
            input = unwrapped;
            self.address_text = input.clone();
        }
        log::info!("Navigate to: {}", input);

        if input.starts_with('#') {
            if let Some(base) = &self.base_url
                && let Ok(resolved) = base.resolve(&input)
            {
                input = resolved.to_string();
                self.address_text = input.clone();
            } else {
                let current_url = self
                    .tabs
                    .get(self.active_tab_idx)
                    .map(|t| t.url.clone())
                    .unwrap_or_else(|| "http://localhost/".to_string());
                if let Ok(base) = Url::parse(&current_url)
                    && let Ok(resolved) = base.resolve(&input)
                {
                    self.base_url = Some(base);
                    input = resolved.to_string();
                    self.address_text = input.clone();
                }
            }
        }

        if is_special_or_local_page(&input) {
            self.base_url = None;
            self.cached_stylesheets.clear();
            let html = resolve_navigation_input(&input);
            self.status_text = format!("Rendered: {}", input);
            self.load_html_internal(html, input, push_history);
            self.address_focused = false;
            return;
        }

        if is_search_query(&input) {
            let search_url = self.config.format_search_url(&input);
            log::info!(
                "Search query: routing to configured search engine: {}",
                search_url
            );
            input = search_url;
            self.address_text = input.clone();
        }

        let parsed_url = match Url::parse(&input) {
            Ok(url) => url,
            Err(_) if !input.contains("://") && !input.starts_with('/') => {
                let with_https = format!("https://{}", input);
                match Url::parse(&with_https) {
                    Ok(url) => {
                        input = with_https;
                        self.address_text = input.clone();
                        url
                    }
                    Err(e) => {
                        let err_msg = format!("Invalid URL: {}", e);
                        let err_html = error_page_html(&input, &err_msg);
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = err_msg;
                        self.load_html_internal(err_html, input, push_history);
                        self.address_focused = false;
                        return;
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Invalid URL: {}", e);
                let err_html = error_page_html(&input, &err_msg);
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = err_msg;
                self.load_html_internal(err_html, input, push_history);
                self.address_focused = false;
                return;
            }
        };

        // Same-document fragment navigation: avoid full network re-fetch when only the #hash changes
        if let Some(base) = &self.base_url
            && parsed_url.scheme == base.scheme
            && parsed_url.host == base.host
            && parsed_url.port == base.port
            && parsed_url.path == base.path
            && parsed_url.query == base.query
        {
            if push_history && let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                tab.push_history(base.to_string(), self.page_title.clone());
            }
            self.base_url = Some(parsed_url.clone());
            self.address_text = parsed_url.to_string();
            self.address_focused = false;
            self.set_tab_loading(self.active_tab_idx, false);
            if let Some(ref frag) = parsed_url.fragment {
                let frag_str = frag.clone();
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(Some(frag_str.clone()));
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(Some(frag_str.clone()));
                }
                self.relayout();
                self.scroll_to_anchor_fragment(&frag_str);
            } else {
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(None);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(None);
                }
                self.relayout();
                self.scroll_to_y(0.0, false);
            }
            return;
        }

        self.status_text = format!("Loading {}...", parsed_url);
        self.set_tab_loading(self.active_tab_idx, true);
        self.address_focused = false;

        // P0 UI-freeze fix: ALL network navigations dispatch via the background
        // NavigationActor pool, even when `async_navigation == false`. The UI
        // thread returns immediately with a loading spinner; completion is
        // delivered through `tick_navigation()` (desktop event loop) or the
        // bounded `wait_for_navigation()` bridge (tests/headless). No
        // synchronous `fetch_document` ever runs on the UI thread here.
        self.nav_actor
            .navigate(self.active_tab_idx, parsed_url, push_history);
        if !self.async_navigation {
            self.wait_for_navigation();
        }
    }
}
