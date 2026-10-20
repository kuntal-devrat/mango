//! Browser history traversal (back/forward), URL navigation, and HTML document loading.

use std::collections::HashMap;

use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;
use mango_js::JsRuntime;
use mango_net::Url;
use mango_render::display_list::DisplayList;
use mango_render::{cache_image, decode_data_uri, get_cached_image};

use super::super::BrowserChrome;
use super::super::forms::find_element_by_id;
use super::super::types::ScriptToRun;
use super::super::welcome::{preprocess_wikipedia_appearance_html, resolve_navigation_input};
use super::pipeline::{collect_scripts, load_imported_stylesheets_parallel};
use super::resources::{collect_stylesheet_images, extract_title, load_web_fonts_async};
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

    pub(crate) fn load_document_internal_with_scripts(
        &mut self,
        doc: Document,
        html: String,
        url: String,
        push_history: bool,
        skip_embedded_sheets_and_fonts: bool,
        preloaded_scripts: Option<Vec<ScriptToRun>>,
    ) {
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
                if let Some(ref root) = self.root_box {
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

        self.cached_document = None;
        self.root_box = None;
        self.page_display_list = DisplayList::new();
        self.display_list_cache.lock().unwrap().clear();
        self.layer_tree.clear_page_tiles();
        mango_render::prune_image_caches();
        if let Some(mut old_rt) = self.js_runtime.take() {
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
        let mut js_rt = JsRuntime::new_with_url_storage_session_and_cookies(
            doc,
            w,
            content_h,
            &url,
            self.local_storage.clone(),
            session_storage,
            self.cookie_jar.clone(),
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
                && let Some(ref base) = self.base_url.clone()
            {
                let mut visited = std::collections::HashSet::new();
                let mut pending_imports = Vec::new();
                for sheet in &embedded_sheets {
                    for rule in &sheet.rules {
                        if let mango_css::parser::Rule::Import(import_path) = rule {
                            pending_imports.push((base.clone(), import_path.clone()));
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

        // Pre-layout so the page renders immediately even if scripts take time.
        // CSS background images are queued from the style scan BEFORE the first
        // layout (no box tree needed) and fetched on a background worker, so
        // there is exactly ONE layout here and no middle blocking relayout.
        if let Some(ref base) = self.base_url.clone() {
            let mut bg_images = Vec::new();
            let dummy_doc = js_rt.document_ref();
            collect_stylesheet_images(&dummy_doc, &self.cached_stylesheets, &mut bg_images);
            collect_stylesheet_images(&dummy_doc, &embedded_sheets, &mut bg_images);
            drop(dummy_doc);
            let mut seen_bg = std::collections::HashSet::new();
            bg_images.retain(|u| seen_bg.insert(u.clone()));
            // Inline fast path only (data: URIs + memory cache). Remote URLs go
            // to the background backlog; never block the UI thread here.
            let mut remote_bg = Vec::new();
            for bg in bg_images {
                if bg.trim_start().starts_with("data:") {
                    if let Some(decoded) = decode_data_uri(&bg) {
                        cache_image(&bg, decoded);
                    }
                } else if get_cached_image(&bg).is_none() {
                    remote_bg.push(bg);
                }
            }
            if !remote_bg.is_empty() && self.image_rx.is_none() {
                self.queue_image_backlog(remote_bg, base.clone());
            }
        }
        self.relayout();

        // Extract and execute scripts in document order
        let scripts = if let Some(pre) = preloaded_scripts {
            pre
        } else {
            let mut s = Vec::new();
            let doc_ref = self.document_view();
            collect_scripts(&doc_ref, doc_ref.root(), &mut s);
            s
        };

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
            // Provide at least 1.5s for normal scripts, plus ~100ms per 10KB for large bundles, capped at 4.0s
            let extra_ms = (code_len / 10_000) as u64 * 100;
            std::time::Duration::from_millis(1500 + extra_ms)
                .min(std::time::Duration::from_millis(4000))
        };
        let run_script = |s: ScriptToRun, rt: &mut mango_js::JsRuntime, ext_cnt: &mut usize| match s
        {
            ScriptToRun::Inline { code, is_module } => {
                let to = script_timeout(code.len());
                let res = if is_module {
                    rt.execute_module_with_timeout(&code, to)
                } else {
                    rt.execute_script_with_timeout(&code, to)
                };
                if let Err(e) = res {
                    log::warn!("Inline script error: {e}");
                }
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
            }
            ScriptToRun::External {
                src,
                is_module,
                nonce,
                ..
            } => {
                const MAX_EXTERNAL_SCRIPTS: usize = 128;
                if *ext_cnt >= MAX_EXTERNAL_SCRIPTS {
                    return;
                }
                let Some(ref base) = self.base_url else {
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    return;
                };
                let key = resolved.as_str();
                let script_code = if let Some(cached) = self.loader.cache().get(&key) {
                    Some(String::from_utf8_lossy(&cached.body).into_owned())
                } else {
                    match self
                        .loader
                        .fetch_script_with_nonce(base, &src, nonce.as_deref())
                    {
                        Ok(code) => Some(code),
                        Err(e) => {
                            log::warn!("Failed to fetch external script {src}: {e}");
                            None
                        }
                    }
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
                }
            }
        };

        self.pending_scripts.clear();

        // 1. Run synchronous inline and external scripts with 50ms budget on UI thread
        let sync_start = std::time::Instant::now();
        let mut sync_iter = sync_scripts.into_iter();
        while let Some(s) = sync_iter.next() {
            run_script(s, &mut js_rt, &mut external_count);
            if sync_start.elapsed() > std::time::Duration::from_millis(50) {
                for remaining in sync_iter {
                    self.pending_scripts.push_back(remaining);
                }
                break;
            }
        }

        // 2. Queue defer and async scripts for non-blocking incremental execution in tick_js
        for s in defer_scripts {
            self.pending_scripts.push_back(s);
        }
        for s in async_scripts {
            self.pending_scripts.push_back(s);
        }

        // 3. Run extension content scripts
        for js_code in ext_js {
            let to = script_timeout(js_code.len());
            let _ = js_rt.execute_script_with_timeout(&js_code, to);
        }

        // 4. Dispatch DOMContentLoaded and window load events if no scripts are pending
        if self.pending_scripts.is_empty() {
            let _ = js_rt.dispatch_dom_content_loaded();
            let _ = js_rt.dispatch_load();
        }

        if let Some(alert_msg) = js_rt.take_status_text() {
            self.status_text = alert_msg;
        }

        let redirect_url = js_rt.take_pending_navigation();

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

        // If a script requested navigation (e.g. window.location.href = ...), follow it
        if let Some(redirect) = redirect_url
            && redirect != url
        {
            self.navigate(&redirect);
        }

        // If a script requested opening a window/tab (window.open)
        if let Some(open_req) = self
            .js_runtime
            .as_mut()
            .and_then(|rt| rt.take_pending_open_window())
        {
            let nav_url = if let Some(base) = &self.base_url {
                base.resolve(&open_req.url)
                    .map(|u| u.to_string())
                    .unwrap_or(open_req.url)
            } else {
                open_req.url
            };
            self.new_tab_and_navigate(&nav_url);
        }
    }

    /// Executes a single pending deferred/async script with a watchdog timeout.
    pub(crate) fn execute_pending_script(&mut self, s: ScriptToRun) {
        let Some(ref mut rt) = self.js_runtime else {
            return;
        };
        let script_timeout = |code_len: usize| -> std::time::Duration {
            let extra_ms = (code_len / 10_000) as u64 * 100;
            std::time::Duration::from_millis(2500 + extra_ms)
                .min(std::time::Duration::from_secs(120))
        };
        match s {
            ScriptToRun::Inline { code, is_module } => {
                let to = script_timeout(code.len());
                let res = if is_module {
                    rt.execute_module_with_timeout(&code, to)
                } else {
                    rt.execute_script_with_timeout(&code, to)
                };
                if let Err(e) = res {
                    log::warn!("Pending inline script error: {e}");
                }
            }
            ScriptToRun::Preloaded {
                code,
                src,
                is_module,
                ..
            } => {
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
                    log::warn!("Pending preloaded script error ({src}): {e}");
                }
            }
            ScriptToRun::External {
                src,
                is_module,
                nonce,
                ..
            } => {
                let Some(ref base) = self.base_url else {
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    return;
                };
                let key = resolved.as_str();
                let script_code = if let Some(cached) = self.loader.cache().get(&key) {
                    Some(String::from_utf8_lossy(&cached.body).into_owned())
                } else {
                    match self
                        .loader
                        .fetch_script_with_nonce(base, &src, nonce.as_deref())
                    {
                        Ok(code) => Some(code),
                        Err(e) => {
                            log::warn!("Failed to fetch external script {src}: {e}");
                            None
                        }
                    }
                };
                if let Some(code) = script_code {
                    let to = script_timeout(code.len());
                    let res = if is_module {
                        rt.execute_module_with_name_and_timeout(&key, &code, to)
                    } else {
                        rt.execute_script_with_name_and_timeout(&key, &code, to)
                    };
                    if let Err(e) = res {
                        log::warn!("Pending external script error ({src}): {e}");
                    }
                }
            }
        }
    }

    /// Ticks the JavaScript event loop (executing setTimeout/setInterval).
    /// Navigates the current browser tab to the address in `self.address_text`.
    pub(crate) fn navigate_to_address(&mut self, push_history: bool) {
        self.is_all_selected = false;
        let mut input = self.address_text.trim().to_string();
        log::info!("Navigate to: {}", input);

        if input.starts_with('#')
            && let Some(base) = &self.base_url
            && let Ok(resolved) = base.resolve(&input)
        {
            input = resolved.to_string();
            self.address_text = input.clone();
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
            self.is_loading = false;
            if let Some(ref frag) = parsed_url.fragment {
                let frag_str = frag.clone();
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(Some(frag_str.clone()));
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(Some(frag_str.clone()));
                }
                let doc = self.active_document();
                if let Some(node_id) = find_element_by_id(&doc, &frag_str)
                    && let Some(root) = &self.root_box
                    && let Some(lbox) = root.find_box_for_node(node_id)
                {
                    let target_y = lbox.dimensions.content.y();
                    let max_s = self.max_scroll();
                    self.scroll_y = target_y.clamp(0.0, max_s);
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                        tab.scroll_y = self.scroll_y;
                    }
                }
            } else {
                if let Some(ref mut doc) = self.cached_document {
                    doc.set_target_id(None);
                }
                if let Some(ref mut rt) = self.js_runtime {
                    rt.set_target_id(None);
                }
            }
            self.relayout();
            return;
        }

        self.status_text = format!("Loading {}...", parsed_url);
        self.is_loading = true;
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
