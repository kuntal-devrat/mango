//! Navigation actor polling, progress ticking, and prepared document application.

use std::collections::HashMap;

use mango_net::FetchedDocument;
use mango_render::display_list::DisplayList;

use super::super::BrowserChrome;
use super::super::types::{PreparedDocument, SYNC_NAVIGATION_TIMEOUT};
use super::pipeline::prepare_document_pipeline;
use super::resources::extract_title;
use crate::navigation::error_page_html;

impl BrowserChrome {
    /// Polls background navigation. Returns true if a page navigation completed and UI needs redrawing.
    ///
    /// Drains the entire result queue and applies only the newest result whose
    /// epoch matches the active epoch; stale superseded results are discarded.
    pub fn tick_navigation(&mut self) -> bool {
        let mut batch: Vec<crate::navigation_actor::NavResult> = Vec::new();
        loop {
            match self.nav_actor.try_recv() {
                Ok(res) => {
                    batch.push(res);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_loading = false;
                    break;
                }
            }
        }
        if batch.is_empty() {
            return false;
        }
        // Keep newest epoch only: select the drained result with the highest
        // epoch; anything older is a superseded navigation and is dropped.
        let active = self.nav_actor.active_epoch();
        let mut best_idx: Option<usize> = None;
        let mut best_epoch: u64 = 0;
        let mut best_is_final = false;
        for (i, res) in batch.iter().enumerate() {
            let (epoch, is_final) = match res {
                crate::navigation_actor::NavResult::DocumentReady { epoch, .. }
                | crate::navigation_actor::NavResult::Error { epoch, .. } => (*epoch, true),
                crate::navigation_actor::NavResult::ProgressiveReady { epoch, .. } => {
                    (*epoch, false)
                }
            };
            if epoch > best_epoch || (epoch == best_epoch && is_final && !best_is_final) {
                best_epoch = epoch;
                best_is_final = is_final;
                best_idx = Some(i);
            }
        }
        let Some(idx) = best_idx else {
            return false;
        };
        if best_epoch != active {
            // Newest drained result is still stale; a newer navigation is in
            // flight. Keep the spinner and wait for it.
            return false;
        }
        match batch.into_iter().nth(idx).expect("best index valid") {
            crate::navigation_actor::NavResult::DocumentReady {
                tab_id,
                prep,
                push_history,
                ..
            } => {
                self.is_loading = false;
                if tab_id == self.active_tab_idx {
                    self.apply_prepared_document(prep, push_history);
                    self.persist_cookies();
                    true
                } else if let Some(tab) = self.tabs.get_mut(tab_id) {
                    let final_url = prep.url.clone();
                    let page_title =
                        extract_title(&prep.doc).unwrap_or_else(|| final_url.to_string());
                    tab.url = final_url.to_string();
                    tab.title = page_title.clone();
                    if push_history {
                        tab.push_history(final_url.to_string(), page_title);
                    }
                    tab.contents = Some(crate::tab::TabWebContents {
                        current_html: prep.html,
                        cached_document: Some(prep.doc),
                        base_url: Some(final_url),
                        cached_stylesheets: prep.cached_stylesheets,
                        root_box: None,
                        js_runtime: None,
                        style_snapshot: HashMap::new(),
                        focused_control: None,
                        page_display_list: DisplayList::new(),
                        display_list_cache: mango_layout::display_list::DisplayListCache::new(),
                        image_rx: None,
                        has_active_animations: false,
                        canvas_bg: mango_core::Color::WHITE,
                        engine: None,
                    });
                    self.persist_cookies();
                    false
                } else {
                    false
                }
            }
            crate::navigation_actor::NavResult::ProgressiveReady { tab_id, prep, .. } => {
                if tab_id == self.active_tab_idx {
                    self.current_html = prep.html;
                    self.cached_stylesheets = prep.cached_stylesheets;
                    self.cached_document = Some(prep.doc);
                    let final_url = prep.url.clone();
                    self.base_url = Some(final_url.clone());
                    if !prep.image_sources.is_empty() {
                        self.queue_image_backlog(prep.image_sources, final_url);
                    }
                    self.relayout();
                    self.page_dirty = true;
                    true
                } else {
                    false
                }
            }
            crate::navigation_actor::NavResult::Error {
                tab_id,
                url,
                message,
                push_history,
                ..
            } => {
                self.is_loading = false;
                let target = url.to_string();
                let err_html = error_page_html(&target, &message);
                if tab_id == self.active_tab_idx {
                    self.base_url = None;
                    self.cached_stylesheets.clear();
                    self.status_text = format!("Error: {message}");
                    self.load_html_internal(err_html, target, push_history);
                    self.address_focused = false;
                    self.persist_cookies();
                    true
                } else if let Some(tab) = self.tabs.get_mut(tab_id) {
                    tab.url = target.clone();
                    tab.title = format!("Error: {message}");
                    if push_history {
                        tab.push_history(target, tab.title.clone());
                    }
                    tab.contents = Some(crate::tab::TabWebContents {
                        current_html: err_html,
                        cached_document: None,
                        base_url: None,
                        cached_stylesheets: Vec::new(),
                        root_box: None,
                        js_runtime: None,
                        style_snapshot: HashMap::new(),
                        focused_control: None,
                        page_display_list: DisplayList::new(),
                        display_list_cache: mango_layout::display_list::DisplayListCache::new(),
                        image_rx: None,
                        has_active_animations: false,
                        canvas_bg: mango_core::Color::WHITE,
                        engine: None,
                    });
                    self.persist_cookies();
                    false
                } else {
                    false
                }
            }
        }
    }

    /// Synchronously waits for any pending navigation to complete.
    ///
    /// Used by headless rendering and tests. Bounded by
    /// [`SYNC_NAVIGATION_TIMEOUT`]: on timeout an error page is shown instead
    /// of hanging forever. Stale superseded results are discarded while
    /// waiting for the active epoch.
    pub fn wait_for_navigation(&mut self) {
        if !self.is_loading {
            return;
        }
        let deadline = std::time::Instant::now() + SYNC_NAVIGATION_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                self.is_loading = false;
                let target = self.address_text.clone();
                let err_html = error_page_html(&target, "Navigation timed out after 30s");
                self.base_url = None;
                self.cached_stylesheets.clear();
                self.status_text = "Error: Navigation timed out after 30s".to_string();
                self.load_html_internal(err_html, target, true);
                self.address_focused = false;
                self.persist_cookies();
                return;
            }
            // First drain anything already delivered without blocking.
            if self.tick_navigation() && !self.is_loading {
                return;
            }
            match self
                .nav_actor
                .recv_timeout(remaining.min(std::time::Duration::from_millis(200)))
            {
                Ok(crate::navigation_actor::NavResult::ProgressiveReady {
                    tab_id,
                    prep,
                    epoch,
                }) => {
                    if epoch == self.nav_actor.active_epoch() && tab_id == self.active_tab_idx {
                        self.current_html = prep.html;
                        self.cached_stylesheets = prep.cached_stylesheets;
                        self.cached_document = Some(prep.doc);
                        let final_url = prep.url.clone();
                        self.base_url = Some(final_url.clone());
                        if !prep.image_sources.is_empty() {
                            self.queue_image_backlog(prep.image_sources, final_url);
                        }
                        self.relayout();
                        self.page_dirty = true;
                    }
                    continue;
                }
                Ok(crate::navigation_actor::NavResult::DocumentReady {
                    tab_id,
                    prep,
                    push_history,
                    epoch,
                }) => {
                    if epoch != self.nav_actor.active_epoch() {
                        continue;
                    }
                    self.is_loading = false;
                    if tab_id == self.active_tab_idx {
                        self.apply_prepared_document(prep, push_history);
                    }
                    self.persist_cookies();
                    return;
                }
                Ok(crate::navigation_actor::NavResult::Error {
                    tab_id,
                    url,
                    message,
                    push_history,
                    epoch,
                }) => {
                    if epoch != self.nav_actor.active_epoch() {
                        continue;
                    }
                    self.is_loading = false;
                    let err_html = error_page_html(&url.as_str(), &message);
                    if tab_id == self.active_tab_idx {
                        self.base_url = None;
                        self.cached_stylesheets.clear();
                        self.status_text = format!("Error: {}", message);
                        self.load_html_internal(err_html, url.to_string(), push_history);
                        self.address_focused = false;
                    }
                    self.persist_cookies();
                    return;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Poll again until the deadline; tick_navigation above
                    // already applied anything that arrived.
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.is_loading = false;
                    return;
                }
            }
        }
    }

    /// Applies an asynchronously prepared document to the browser UI without freezing the event loop.
    pub fn apply_prepared_document(&mut self, prep: PreparedDocument, push_history: bool) {
        let final_url = prep.url.clone();
        self.base_url = Some(final_url.clone());
        self.cached_stylesheets = prep.cached_stylesheets;
        self.layer_tree.clear_page_tiles();
        self.display_list_cache.lock().unwrap().clear();
        self.page_display_list = DisplayList::new();
        self.root_box = None;
        mango_render::prune_image_caches();
        mango_render::clear_global_svg_symbols();

        // Web fonts are already fetched and registered on the background worker
        // thread in prepare_document_pipeline. Calling load_web_fonts here on the
        // main UI thread would block the UI with redundant network / lock work.

        // Queue images with epoch guarding, dedup, and memory-bounded cap.
        self.queue_image_backlog(prep.image_sources, final_url.clone());

        self.status_text = format!(
            "Loaded {} ({} bytes, HTTP {})",
            final_url,
            prep.html.len(),
            prep.status
        );
        self.devtools.record_network(
            final_url.to_string(),
            "GET",
            prep.status,
            prep.content_type.clone(),
            prep.html.len(),
        );
        self.devtools.log(
            crate::devtools::ConsoleLevel::Info,
            format!("Navigated to {}", final_url),
        );
        self.load_document_internal_with_scripts(
            prep.doc,
            prep.html,
            final_url.to_string(),
            push_history,
            true,
            Some(prep.preloaded_scripts),
        );
        self.address_focused = false;
    }

    /// Loads a fetched document into the browser, recursively fetching external stylesheets,
    /// web fonts, and images, and updating the page metadata and history.
    pub fn process_and_load_document(&mut self, doc_result: FetchedDocument, push_history: bool) {
        let prep = prepare_document_pipeline(&self.loader, doc_result);
        self.apply_prepared_document(prep, push_history);
    }

    /// Returns true if a navigation or page load is currently in progress.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }
}
