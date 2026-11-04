use std::collections::HashMap;
use std::sync::mpsc::Sender;

use mango_core::{Color, Size};
use mango_css::Stylesheet;
use mango_css::computed::ComputedStyle;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_ipc::{IpcEndpoint, IpcMessage};
use mango_js::JsRuntime;
use mango_layout::box_tree::LayoutBox;
use mango_layout::style_tree::{PageStyleIndex, StyleInvalidator, StyledNode};
use mango_net::Url;
use mango_render::display_list::DisplayList;

use super::types::{RenderFramePayload, RendererResult};
use crate::browser::render::layout::canvas_background_color;

/// Collects `<script type="importmap">` definitions from document.
pub fn collect_import_maps(doc: &Document, node_id: NodeId, out: &mut Vec<String>) {
    if let Some(node) = doc.get(node_id) {
        if let NodeData::Element(elem) = &node.data
            && elem.tag_name.eq_ignore_ascii_case("script")
            && elem
                .get_attribute("type")
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("importmap"))
        {
            let content = doc.text_content(node_id);
            if !content.trim().is_empty() {
                out.push(content);
            }
        }
        for child in doc.children(node_id) {
            collect_import_maps(doc, child.id, out);
        }
    }
}

/// Drains pending client-side navigations, window opens, and console logs from the JS runtime.
pub fn drain_renderer_js_effects(
    rt: &mut JsRuntime,
    current_base_url: Option<&Url>,
    result_tx: &Sender<RendererResult>,
    ipc_ep: &IpcEndpoint,
) {
    let resolve_url = |url: String| {
        current_base_url
            .and_then(|b| b.resolve(&url).ok())
            .map(|u| u.to_string())
            .unwrap_or(url)
    };
    if let Some(url) = rt.take_pending_navigation() {
        let nav_url = resolve_url(url);
        log::info!("Isolated renderer emitted navigation request: {}", nav_url);
        let _ = result_tx.send(RendererResult::NavigationRequested {
            url: nav_url.clone(),
        });
        let _ = ipc_ep.send(&IpcMessage::NavigationRequest { url: nav_url });
    }
    if let Some(open_req) = rt.take_pending_open_window() {
        let nav_url = resolve_url(open_req.url);
        let _ = result_tx.send(RendererResult::OpenWindow {
            url: nav_url.clone(),
            features: open_req.features.clone(),
        });
        let _ = ipc_ep.send(&IpcMessage::OpenWindow {
            url: nav_url,
            features: open_req.features,
        });
    }
    for msg in rt.take_console_messages() {
        use crate::devtools::ConsoleLevel as D;
        use mango_js::console::ConsoleLevel as J;
        let lvl = match msg.level {
            J::Log => D::Log,
            J::Info => D::Info,
            J::Warn => D::Warn,
            J::Error => D::Error,
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
}

/// Tracks incrementally updated styles, invalidations, and cached layout state for the renderer actor.
pub struct IncrementalRendererState {
    style_index: Option<PageStyleIndex>,
    styled_root: Option<StyledNode>,
    invalidator: StyleInvalidator,
    style_hash: u64,
    last_viewport: Size,
    doc_node_count: usize,
}

impl Default for IncrementalRendererState {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalRendererState {
    pub fn new() -> Self {
        Self {
            style_index: None,
            styled_root: None,
            invalidator: StyleInvalidator::new(),
            style_hash: 0,
            last_viewport: Size::new(0.0, 0.0),
            doc_node_count: 0,
        }
    }

    pub fn compute_layout(
        &mut self,
        doc: &Document,
        dirty_nodes: std::collections::HashSet<mango_html::dom::NodeId>,
        stylesheets: &[Stylesheet],
        viewport: Size,
    ) -> (LayoutBox, DisplayList, f32, Color) {
        let mut media = mango_css::current_media_environment();
        media.viewport_width = viewport.width;
        media.viewport_height = viewport.height;

        let embedded = mango_layout::extract_style_elements(doc);
        let mut hasher = std::hash::DefaultHasher::new();
        for s in &embedded {
            std::hash::Hash::hash(&s.rules.len(), &mut hasher);
        }
        let cur_hash = std::hash::Hasher::finish(&hasher);

        let sheets_changed = self.style_index.is_none()
            || cur_hash != self.style_hash
            || self.last_viewport != viewport;
        if sheets_changed {
            self.style_hash = cur_hash;
            self.last_viewport = viewport;
            let mut all_sheets = stylesheets.to_vec();
            all_sheets.extend(embedded);
            self.style_index = Some(PageStyleIndex::from_stylesheets(all_sheets, media.clone()));
            self.styled_root = None;
        }

        let style_index = self.style_index.as_ref().unwrap();
        let cur_node_count = doc.live_node_count();

        let styled_root = if let Some(ref mut root) = self.styled_root {
            if cur_node_count != self.doc_node_count {
                self.doc_node_count = cur_node_count;
                let r = mango_layout::build_style_tree_with_cached_index(
                    doc,
                    style_index.index(),
                    &media,
                )
                .unwrap_or_else(|| StyledNode::new(Some(doc.root()), ComputedStyle::default()));
                self.styled_root = Some(r.clone());
                r
            } else if !dirty_nodes.is_empty() {
                self.invalidator.mark_all_dirty(dirty_nodes);
                let author_refs: Vec<&Stylesheet> = stylesheets.iter().collect();
                self.invalidator
                    .invalidate_and_update(root, doc, &author_refs);
                root.clone()
            } else {
                root.clone()
            }
        } else {
            self.doc_node_count = cur_node_count;
            let root =
                mango_layout::build_style_tree_with_cached_index(doc, style_index.index(), &media)
                    .unwrap_or_else(|| StyledNode::new(Some(doc.root()), ComputedStyle::default()));
            self.styled_root = Some(root.clone());
            root
        };

        let (root_box, display_list) =
            mango_layout::layout_styled_document(doc, styled_root, &media, 0.0, 0.0);
        let canvas_bg = canvas_background_color(&root_box);
        let scrollable_height = crate::scroll::compute_scrollable_height(Some(&root_box));
        (root_box, display_list, scrollable_height, canvas_bg)
    }
}

/// Computes layout incrementally and emits a `RenderFramePayload` to the browser process.
pub fn emit_frame(
    rt: &mut JsRuntime,
    incremental_state: &mut IncrementalRendererState,
    stylesheets: &[Stylesheet],
    viewport_size: Size,
    current_generation: u64,
    frame_seq: &mut u64,
    result_tx: &Sender<RendererResult>,
) -> Option<String> {
    let dirty_nodes = rt.take_dirty_nodes();
    let has_dom_mutations = !dirty_nodes.is_empty() || *frame_seq == 0;
    let eval_title = rt.eval("document.title").ok().filter(|s| !s.is_empty());
    let doc_ref = rt.document_ref();
    let title =
        eval_title.or_else(|| crate::browser::navigation::resources::extract_title(&doc_ref));
    let (mut root, display_list, scrollable_height, canvas_bg) =
        incremental_state.compute_layout(&doc_ref, dirty_nodes, stylesheets, viewport_size);
    root.prune_raw_children();

    let doc_mirror = if has_dom_mutations {
        Some(doc_ref.clone())
    } else {
        None
    };
    drop(doc_ref);

    *frame_seq = frame_seq.wrapping_add(1);
    let _ = result_tx.send(RendererResult::FrameReady(Box::new(RenderFramePayload {
        frame_id: *frame_seq,
        generation: current_generation,
        display_list,
        scrollable_height,
        title: title.clone(),
        canvas_bg,
        root_box: Some(root),
        style_snapshot: HashMap::new(),
        focused_control: None,
        doc_mirror,
    })));
    title
}
