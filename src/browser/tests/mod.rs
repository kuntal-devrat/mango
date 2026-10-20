//! Subsystem unit tests.

use super::BrowserChrome;
use mango_html::dom::NodeId;

/// Helper: reads a node attribute from the live JS runtime.
pub(super) fn attr(browser: &BrowserChrome, node_id: NodeId, name: &str) -> Option<String> {
    browser
        .js_runtime
        .as_ref()
        .and_then(|rt| rt.get_node_attribute(node_id, name))
}

/// Helper: hit-tests the first form control at a document-space point.
pub(super) fn hit_control(browser: &BrowserChrome, x: f32, y: f32) -> mango_layout::FormControlHit {
    browser
        .root_box
        .as_ref()
        .and_then(|r| r.hit_test_form_control(mango_core::Point::new(x, y)))
        .expect("expected to hit a form control")
}

mod core;
mod interactions;
mod pickers_lifecycle;
