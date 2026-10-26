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
    /// Polls background navigation. Returns true if any applied result changed
    /// visible state (the page or the tab strip).
    ///
    /// At most one result is applied per turn so a burst of progressive or
    /// multi-tab results cannot monopolize the window thread. Epochs are
    /// per-tab, so a result for tab A remains valid while tab B navigates.
    pub fn tick_navigation(&mut self) -> bool {
        match self.nav_actor.try_recv() {
            Ok(res) => {
                self.persist_cookies();
                self.process_nav_result(res)
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                for tab in &mut self.tabs {
                    tab.loading = false;
                }
                self.is_loading = false;
                false
            }
        }
    }

    /// Applies one navigation result if it is still current for its tab.
    fn process_nav_result(&mut self, res: crate::navigation_actor::NavResult) -> bool {
        match res {
            crate::navigation_actor::NavResult::DocumentReady {
                tab_id,
                prep,
                push_history,
                epoch,
            } => {
                if !self.nav_actor.is_current(tab_id, epoch) {
                    return false;
                }
                self.set_tab_loading(tab_id, false);
                if tab_id == self.active_tab_idx {
                    self.apply_prepared_document(prep, push_history);
                    true
                } else {
                    self.store_background_prepared(tab_id, prep, push_history);
                    false
                }
            }
            crate::navigation_actor::NavResult::ProgressiveReady {
                tab_id,
                prep,
                epoch,
            } => {
                if !self.nav_actor.is_current(tab_id, epoch) {
                    return false;
                }
                // Progressive preparation can finish after the final response
                // result because it runs independently from the network reader.
                // Never let a late partial snapshot overwrite a completed page.
                if !self.tabs.get(tab_id).is_some_and(|tab| tab.loading) {
                    return false;
                }
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
                    self.store_background_prepared(tab_id, prep, false);
                    false
                }
            }
            crate::navigation_actor::NavResult::Error {
                tab_id,
                url,
                message,
                push_history,
                epoch,
            } => {
                if !self.nav_actor.is_current(tab_id, epoch) {
                    return false;
                }
                self.set_tab_loading(tab_id, false);
                let target = url.to_string();
                let err_html = error_page_html(&target, &message);
                if tab_id == self.active_tab_idx {
                    self.base_url = None;
                    self.cached_stylesheets.clear();
                    self.status_text = format!("Error: {message}");
                    self.load_html_internal(err_html, target, push_history);
                    self.address_focused = false;
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
                        resource_scheduler:
                            crate::browser::resource_scheduler::DocumentResourceScheduler::default(),
                        has_active_animations: false,
                        canvas_bg: mango_core::Color::WHITE,
                        engine: None,
                    });
                    false
                } else {
                    false
                }
            }
        }
    }

    /// Stores a prepared document for a background tab without touching the
    /// active page's DOM, layout, or JavaScript runtime.
    fn store_background_prepared(
        &mut self,
        tab_id: usize,
        prep: PreparedDocument,
        push_history: bool,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_id) else {
            return;
        };
        let final_url = prep.url.clone();
        let page_title = extract_title(&prep.doc).unwrap_or_else(|| final_url.to_string());
        tab.url = final_url.to_string();
        tab.title = page_title.clone();
        if push_history {
            tab.push_history(final_url.to_string(), page_title);
        }
        // Replacing a background tab's contents drops its document without
        // going through navigation cleanup; release any canvas backing stores
        // owned by the old document first.
        if let Some(old) = tab.contents.take() {
            if let Some(doc) = &old.cached_document {
                mango_render::remove_canvases_for_document(doc.id());
            }
            if let Some(rt) = &old.js_runtime {
                mango_render::remove_canvases_for_document(rt.document_ref().id());
            }
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
            resource_scheduler:
                crate::browser::resource_scheduler::DocumentResourceScheduler::default(),
            has_active_animations: false,
            canvas_bg: mango_core::Color::WHITE,
            engine: None,
        });
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
                self.set_tab_loading(self.active_tab_idx, false);
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
            // Drain anything already delivered without blocking.
            self.tick_navigation();
            if !self.is_loading {
                return;
            }
            match self
                .nav_actor
                .recv_timeout(remaining.min(std::time::Duration::from_millis(200)))
            {
                Ok(res) => {
                    self.process_nav_result(res);
                    if !self.is_loading {
                        self.persist_cookies();
                        return;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    self.set_tab_loading(self.active_tab_idx, false);
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
        // Start image work only after the document generation has been
        // established by load_document_internal_with_scripts.  That function
        // resets the per-document scheduler; queueing before it would make
        // every image disappear from the resource report and detach image
        // work from the active generation.
        self.queue_image_backlog(prep.image_sources, final_url.clone());
        self.address_focused = false;
    }

    /// Loads a fetched document into the browser, recursively fetching external stylesheets,
    /// web fonts, and images, and updating the page metadata and history.
    pub fn process_and_load_document(&mut self, doc_result: FetchedDocument, push_history: bool) {
        let prep = prepare_document_pipeline(&self.loader, doc_result);
        self.apply_prepared_document(prep, push_history);
    }

    /// Returns true if a navigation or page load is currently in progress in
    /// the active tab.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Returns true if any tab (including a background tab) is loading.
    pub fn has_pending_navigation(&self) -> bool {
        self.tabs.iter().any(|tab| tab.loading)
    }
}
