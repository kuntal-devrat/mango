//! Style sharing cache for deduplicating `ComputedStyle` instances across identical elements and text nodes.
//!
//! On large feeds or data-dense pages with thousands of repeated elements (e.g. news cards,
//! comments, table rows, list items), allocating a full ~2 KB `ComputedStyle` per element
//! exhausts hundreds of megabytes of memory.
//!
//! `StyleSharingCache` enables elements with identical tags, classes, attributes, pseudo states,
//! and parent styles to share a single `Arc<ComputedStyle>`, dropping style-tree memory by 80–90%.

use std::collections::HashMap;
use std::sync::Arc;

use mango_css::cascade::RuleIndex;
use mango_css::computed::ComputedStyle;
use mango_html::dom::{Document, ElementData, NodeId};

/// Key uniquely identifying an element's cascaded style equivalence class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementStyleKey {
    /// Pointer address of parent element's computed style (for inheritance).
    pub parent_style_ptr: usize,
    /// Element tag name (canonical lowercase).
    pub tag_name: String,
    /// Element id attribute, if any.
    pub id: Option<String>,
    /// Element class attribute, if any.
    pub class: Option<String>,
    /// Element inline style attribute, if any.
    pub inline_style: Option<String>,
    /// Remaining attributes that might affect attribute selectors.
    pub other_attributes: Vec<(String, String)>,
    /// Positional classification (first-child, last-child, nth-child, etc.).
    pub positional_key: u32,
    /// Hover state.
    pub is_hovered: bool,
    /// Active / mouse-down state.
    pub is_active: bool,
}

/// Cache for sharing `Arc<ComputedStyle>` across structurally identical elements and text nodes.
pub struct StyleSharingCache {
    /// Element style cache.
    element_cache: HashMap<ElementStyleKey, Arc<ComputedStyle>>,
    /// Text node style cache keyed by parent style pointer address.
    text_cache: HashMap<usize, Arc<ComputedStyle>>,
    /// Whether the stylesheet index contains any positional pseudo-class rules.
    has_positional_rules: bool,
    /// Whether the stylesheet index contains any nth-child / nth-of-type rules.
    has_nth_rules: bool,
    /// Whether the stylesheet index contains any :empty pseudo-class rules.
    has_empty_rules: bool,
}

impl StyleSharingCache {
    /// Creates a new style sharing cache configured for the given author stylesheet index.
    pub fn new(rule_index: &RuleIndex) -> Self {
        let has_nth_rules = rule_index.has_pseudo_rules("nth-child")
            || rule_index.has_pseudo_rules("nth-last-child")
            || rule_index.has_pseudo_rules("nth-of-type")
            || rule_index.has_pseudo_rules("nth-last-of-type");

        let has_empty_rules = rule_index.has_pseudo_rules("empty");

        let has_positional_rules = has_nth_rules
            || has_empty_rules
            || rule_index.has_pseudo_rules("first-child")
            || rule_index.has_pseudo_rules("last-child")
            || rule_index.has_pseudo_rules("only-child")
            || rule_index.has_pseudo_rules("first-of-type")
            || rule_index.has_pseudo_rules("last-of-type")
            || rule_index.has_pseudo_rules("only-of-type")
            || rule_index.has_pseudo_rules("root")
            || rule_index.has_pseudo_rules("target");

        Self {
            element_cache: HashMap::new(),
            text_cache: HashMap::new(),
            has_positional_rules,
            has_nth_rules,
            has_empty_rules,
        }
    }

    /// Computes or retrieves a shared `Arc<ComputedStyle>` for a text node inheriting from `parent_style`.
    pub fn get_or_compute_text_style(
        &mut self,
        parent_style: Option<&Arc<ComputedStyle>>,
        compute_fn: impl FnOnce() -> ComputedStyle,
    ) -> Arc<ComputedStyle> {
        let ptr = parent_style.map(|s| Arc::as_ptr(s) as usize).unwrap_or(0);

        if let Some(cached) = self.text_cache.get(&ptr) {
            return Arc::clone(cached);
        }

        let computed = Arc::new(compute_fn());
        self.text_cache.insert(ptr, Arc::clone(&computed));
        computed
    }

    /// Checks if a cached style exists for the given element key.
    pub fn get_element_style(&self, key: &ElementStyleKey) -> Option<Arc<ComputedStyle>> {
        self.element_cache.get(key).cloned()
    }

    /// Stores a computed style for the given element key.
    pub fn insert_element_style(&mut self, key: ElementStyleKey, style: Arc<ComputedStyle>) {
        self.element_cache.insert(key, style);
    }

    /// Builds a cache key for an element node, if eligible for style sharing.
    pub fn make_element_key(
        &self,
        doc: &Document,
        node_id: NodeId,
        el: &ElementData,
        parent_style: Option<&Arc<ComputedStyle>>,
    ) -> Option<ElementStyleKey> {
        // Never share style for shadow hosts or template elements
        if el.shadow_root.is_some() || el.template_contents.is_some() {
            return None;
        }

        let parent_ptr = parent_style.map(|s| Arc::as_ptr(s) as usize).unwrap_or(0);

        let mut id = None;
        let mut class = None;
        let mut inline_style = None;
        let mut other_attrs = Vec::new();

        for (name, val) in &el.attributes {
            if name.eq_ignore_ascii_case("id") {
                id = Some(val.clone());
            } else if name.eq_ignore_ascii_case("class") {
                class = Some(val.clone());
            } else if name.eq_ignore_ascii_case("style") {
                inline_style = Some(val.clone());
            } else {
                other_attrs.push((name.clone(), val.clone()));
            }
        }

        let positional_key = if self.has_positional_rules {
            self.compute_positional_key(doc, node_id, &el.tag_name)
        } else {
            0
        };

        Some(ElementStyleKey {
            parent_style_ptr: parent_ptr,
            tag_name: el.tag_name.to_ascii_lowercase(),
            id,
            class,
            inline_style,
            other_attributes: other_attrs,
            positional_key,
            is_hovered: el.is_hovered,
            is_active: el.is_active,
        })
    }

    fn compute_positional_key(&self, doc: &Document, node_id: NodeId, tag: &str) -> u32 {
        let Some(parent_id) = doc.get(node_id).and_then(|n| n.parent) else {
            return 1; // Root element flag
        };

        let mut key: u32 = 0;
        let mut is_first = true;
        let mut is_first_of_type = true;
        let mut child_idx = 0;
        let mut found_self = false;
        let mut has_subsequent = false;
        let mut has_subsequent_of_type = false;

        for child in doc.children(parent_id) {
            if let mango_html::dom::NodeData::Element(c_el) = &child.data {
                if !found_self {
                    if child.id == node_id {
                        found_self = true;
                    } else {
                        is_first = false;
                        if c_el.tag_name.eq_ignore_ascii_case(tag) {
                            is_first_of_type = false;
                        }
                    }
                    child_idx += 1;
                } else {
                    has_subsequent = true;
                    if c_el.tag_name.eq_ignore_ascii_case(tag) {
                        has_subsequent_of_type = true;
                    }
                }
            }
        }

        if is_first {
            key |= 1 << 0;
        }
        if !has_subsequent {
            key |= 1 << 1;
        }
        if is_first_of_type {
            key |= 1 << 2;
        }
        if !has_subsequent_of_type {
            key |= 1 << 3;
        }

        if self.has_empty_rules {
            let is_empty = doc.get(node_id).and_then(|n| n.first_child).is_none();
            if is_empty {
                key |= 1 << 4;
            }
        }

        if self.has_nth_rules {
            // Encode 1-based child index into higher 16 bits
            key |= (child_idx.min(65535) as u32) << 16;
        }

        key
    }
}

/// Derives the normalized inline computed style for a text node from its parent element style.
pub fn derive_text_computed_style(parent_style: Option<&ComputedStyle>) -> ComputedStyle {
    let mut style = parent_style.cloned().unwrap_or_default();
    style.display = mango_css::values::Display::Inline;
    style.position = mango_css::values::Position::Static;
    style.float = mango_css::values::Float::None;
    style.clear = mango_css::values::Clear::None;

    let is_parent_inline = parent_style
        .map(|s| {
            matches!(
                s.display,
                mango_css::values::Display::Inline
                    | mango_css::values::Display::Ruby
                    | mango_css::values::Display::RubyBase
                    | mango_css::values::Display::RubyText
            )
        })
        .unwrap_or(false);
    if !is_parent_inline {
        style.vertical_align = mango_css::values::VerticalAlign::Baseline;
    }

    style.background_image = None;
    style.background_color = mango_core::Color::TRANSPARENT;
    style.box_shadow = None;
    style.border_top_width = 0.0;
    style.border_right_width = 0.0;
    style.border_bottom_width = 0.0;
    style.border_left_width = 0.0;
    style.margin_top = mango_css::values::Length::Px(0.0);
    style.margin_right = mango_css::values::Length::Px(0.0);
    style.margin_bottom = mango_css::values::Length::Px(0.0);
    style.margin_left = mango_css::values::Length::Px(0.0);
    style.padding_top = mango_css::values::Length::Px(0.0);
    style.padding_right = mango_css::values::Length::Px(0.0);
    style.padding_bottom = mango_css::values::Length::Px(0.0);
    style.padding_left = mango_css::values::Length::Px(0.0);
    style.width = mango_css::values::Length::Auto;
    style.height = mango_css::values::Length::Auto;
    style.min_width = mango_css::values::Length::Auto;
    style.min_height = mango_css::values::Length::Auto;
    style.max_width = mango_css::values::Length::Auto;
    style.max_height = mango_css::values::Length::Auto;

    style
}

/// Checks whether an HTML tag consumes attributes during box-tree construction or hit-testing.
///
/// Plain formatting elements like `<div>`, `<span>`, `<p>`, headings, etc. do not read
/// `attributes` on `StyledNode` or `LayoutBox` after cascade matching has resolved. Skipping
/// attribute duplication for them saves tens of thousands of heap allocations on large pages.
pub fn is_attribute_consuming_tag(tag: &str) -> bool {
    matches!(
        tag,
        "img"
            | "embed"
            | "object"
            | "svg"
            | "iframe"
            | "video"
            | "audio"
            | "canvas"
            | "select"
            | "input"
            | "button"
            | "textarea"
            | "form"
            | "a"
            | "area"
            | "ol"
            | "ul"
            | "li"
            | "details"
            | "summary"
            | "picture"
            | "source"
            | "track"
            | "meter"
            | "progress"
            | "table"
            | "td"
            | "th"
    )
}
