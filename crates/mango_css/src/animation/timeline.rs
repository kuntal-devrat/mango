//! Scroll and view timelines for scroll-driven animations (CSS Scroll-driven Animations Module Level 1).

use mango_html::dom::NodeId;

/// A scroll-driven animation timeline tracking scroll progress in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollTimeline {
    pub name: String,
    pub source: Option<NodeId>,
    pub axis: crate::values::ScrollTimelineAxis,
    pub current_scroll_px: f32,
    pub max_scroll_px: f32,
}

impl ScrollTimeline {
    pub fn new(name: impl Into<String>, axis: crate::values::ScrollTimelineAxis) -> Self {
        Self {
            name: name.into(),
            source: None,
            axis,
            current_scroll_px: 0.0,
            max_scroll_px: 0.0,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.max_scroll_px <= 0.0 {
            0.0
        } else {
            (self.current_scroll_px / self.max_scroll_px).clamp(0.0, 1.0)
        }
    }
}

/// A view-driven animation timeline tracking subject intersection with scrollport.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewTimeline {
    pub name: String,
    pub subject: Option<NodeId>,
    pub axis: crate::values::ScrollTimelineAxis,
    pub entry_ratio: f32,
}

impl ViewTimeline {
    pub fn new(name: impl Into<String>, axis: crate::values::ScrollTimelineAxis) -> Self {
        Self {
            name: name.into(),
            subject: None,
            axis,
            entry_ratio: 0.0,
        }
    }

    pub fn progress(&self) -> f32 {
        self.entry_ratio.clamp(0.0, 1.0)
    }
}
