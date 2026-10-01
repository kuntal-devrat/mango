//! Scroll and scrollbar management subsystem for Mango Browser (OPT-001).
//!
//! Provides layout extent calculation, scroll offset clamping, scrollbar
//! geometry calculation (track and thumb), drag interaction calculations,
//! and scrollbar display list command rendering.

use mango_core::{Color, Rect};
use mango_layout::box_tree::LayoutBox;
use mango_render::display_list::DisplayCommand;

/// Recursively computes the true maximum scrollable bottom boundary of a layout box tree,
/// excluding fixed-position overlays so sticky/fixed elements do not inflate scroll boundaries.
pub fn compute_scrollable_extent(box_node: &LayoutBox) -> f32 {
    let mut max_y = box_node.dimensions.margin_box().bottom();
    for child in &box_node.children {
        let is_fixed = child
            .style
            .as_ref()
            .is_some_and(|s| s.position == mango_css::values::Position::Fixed);
        if !is_fixed {
            max_y = max_y.max(compute_scrollable_extent(child));
        }
    }
    max_y
}

/// Returns the true scrollable document height scanning all layout boxes in the root.
pub fn compute_scrollable_height(root_box: Option<&LayoutBox>) -> f32 {
    if let Some(root) = root_box {
        compute_scrollable_extent(root)
    } else {
        0.0
    }
}

/// Computes maximum allowed scroll offset based on rendered content height.
pub fn compute_max_scroll(doc_h: f32, viewport_h: f32, header_h: f32, status_bar_h: f32) -> f32 {
    let content_h = (viewport_h - header_h - status_bar_h - 1.0).max(10.0);
    (doc_h - content_h).max(0.0)
}

/// Clamps scroll offset given a scroll wheel delta.
pub fn clamp_scroll(current_scroll_y: f32, delta_y: f32, max_scroll: f32) -> f32 {
    let scroll_step = delta_y * 45.0;
    (current_scroll_y - scroll_step).clamp(0.0, max_scroll)
}

/// Scroll behavior mode (CSSOM View Module §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

/// A smooth scrolling animation controller implementing cubic ease-out interpolation.
#[derive(Debug, Clone)]
pub struct SmoothScrollAnimation {
    pub start_y: f32,
    pub target_y: f32,
    pub current_y: f32,
    pub start_time: std::time::Instant,
    pub duration: std::time::Duration,
    pub max_scroll: f32,
    pub active: bool,
}

impl SmoothScrollAnimation {
    /// Creates a new smooth scroll animation towards `target_y`.
    pub fn new(
        current_y: f32,
        target_y: f32,
        max_scroll: f32,
        duration: std::time::Duration,
    ) -> Self {
        let clamped_target = target_y.clamp(0.0, max_scroll);
        Self {
            start_y: current_y,
            target_y: clamped_target,
            current_y,
            start_time: std::time::Instant::now(),
            duration: duration.max(std::time::Duration::from_millis(50)),
            max_scroll,
            active: (current_y - clamped_target).abs() > 0.5,
        }
    }

    /// Advances the smooth scroll animation to time `now`.
    /// Returns `(new_scroll_y, is_finished)`.
    pub fn step(&mut self, now: std::time::Instant) -> (f32, bool) {
        if !self.active {
            return (self.current_y, true);
        }

        let elapsed = now.saturating_duration_since(self.start_time);
        let progress = (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);

        // Cubic ease-out: 1.0 - (1.0 - t)^3
        let ease_out = 1.0 - (1.0 - progress).powi(3);
        let interpolated = self.start_y + (self.target_y - self.start_y) * ease_out;
        self.current_y = interpolated.clamp(0.0, self.max_scroll);

        if progress >= 1.0 {
            self.current_y = self.target_y;
            self.active = false;
            (self.current_y, true)
        } else {
            (self.current_y, false)
        }
    }

    /// Retargets an active animation to a new target while maintaining current velocity.
    pub fn update_target(&mut self, new_target: f32, duration: std::time::Duration) {
        let clamped = new_target.clamp(0.0, self.max_scroll);
        self.start_y = self.current_y;
        self.target_y = clamped;
        self.start_time = std::time::Instant::now();
        self.duration = duration;
        self.active = (self.current_y - clamped).abs() > 0.5;
    }
}

/// Geometry of a vertical scrollbar including track and draggable thumb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarGeometry {
    pub track_rect: Rect,
    pub thumb_rect: Rect,
}

impl ScrollbarGeometry {
    /// Computes the scrollbar geometry for the given viewport and document metrics.
    pub fn compute(
        window_w: f32,
        window_h: f32,
        header_h: f32,
        status_bar_h: f32,
        doc_h: f32,
        scroll_y: f32,
        max_scroll: f32,
    ) -> Option<Self> {
        if max_scroll <= 0.0 || doc_h <= 0.0 {
            return None;
        }

        let content_y_top = header_h + 1.0;
        let content_h = (window_h - header_h - status_bar_h - 1.0).max(10.0);
        let scrollbar_w = 12.0f32;
        let scrollbar_x = window_w - scrollbar_w;

        let track_rect = Rect::new(scrollbar_x, content_y_top, scrollbar_w, content_h);

        let thumb_h = ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
        let max_thumb_travel = (content_h - thumb_h).max(1.0);
        let thumb_y = content_y_top + (scroll_y / max_scroll) * max_thumb_travel;

        let thumb_padding = 2.0f32;
        let thumb_rect = Rect::new(
            scrollbar_x + thumb_padding,
            thumb_y,
            scrollbar_w - thumb_padding * 2.0,
            thumb_h,
        );

        Some(Self {
            track_rect,
            thumb_rect,
        })
    }
}

/// Computes the new scroll position during an active mouse drag of the scrollbar thumb.
pub fn compute_drag_scroll(
    drag_start_scroll_y: f32,
    drag_start_mouse_y: f32,
    current_mouse_y: f32,
    content_h: f32,
    doc_h: f32,
    max_scroll: f32,
) -> f32 {
    if max_scroll <= 0.0 || doc_h <= 0.0 {
        return 0.0;
    }
    let thumb_h = ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
    let max_thumb_travel = (content_h - thumb_h).max(1.0);
    let delta_mouse_y = current_mouse_y - drag_start_mouse_y;
    let delta_scroll = (delta_mouse_y / max_thumb_travel) * max_scroll;
    (drag_start_scroll_y + delta_scroll).clamp(0.0, max_scroll)
}

/// Emits display commands to render the vertical scrollbar.
pub fn render_scrollbar(
    geom: &ScrollbarGeometry,
    is_dragging: bool,
    is_hovered: bool,
) -> Vec<DisplayCommand> {
    let mut cmds = Vec::with_capacity(2);

    // Track background (subtle semi-transparent)
    cmds.push(DisplayCommand::FillRect {
        rect: geom.track_rect,
        color: Color::rgba(240, 240, 240, 140),
    });

    let thumb_color = if is_dragging {
        Color::rgb(100, 100, 100) // Active dark dragging
    } else if is_hovered {
        Color::rgb(140, 140, 140) // Medium hover
    } else {
        Color::rgb(190, 190, 190) // Normal sleek grey
    };

    cmds.push(DisplayCommand::FillRoundedRect {
        rect: geom.thumb_rect,
        color: thumb_color,
        radii: [4.0, 4.0, 4.0, 4.0],
    });

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_geometry_and_clamping() {
        let max_s = compute_max_scroll(1200.0, 600.0, 76.0, 24.0);
        assert!(max_s > 0.0);

        let scrolled = clamp_scroll(0.0, -2.0, max_s);
        assert!(scrolled > 0.0);

        let clamped_top = clamp_scroll(0.0, 2.0, max_s);
        assert_eq!(clamped_top, 0.0);

        let geom = ScrollbarGeometry::compute(800.0, 600.0, 76.0, 24.0, 1200.0, 100.0, max_s);
        assert!(geom.is_some());
        let g = geom.unwrap();
        assert!(g.thumb_rect.height() >= 32.0);
        assert!(g.thumb_rect.y() >= g.track_rect.y());

        let cmds = render_scrollbar(&g, false, true);
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_drag_scroll_calculation() {
        let max_s = 500.0;
        let new_scroll = compute_drag_scroll(100.0, 200.0, 250.0, 500.0, 1000.0, max_s);
        assert!(new_scroll > 100.0);
        assert!(new_scroll <= max_s);
    }

    #[test]
    fn test_smooth_scroll_animation() {
        let mut anim =
            SmoothScrollAnimation::new(0.0, 200.0, 500.0, std::time::Duration::from_millis(300));
        assert!(anim.active);
        assert_eq!(anim.start_y, 0.0);
        assert_eq!(anim.target_y, 200.0);

        // Step halfway
        let t_half = anim.start_time + std::time::Duration::from_millis(150);
        let (y_half, finished) = anim.step(t_half);
        assert!(!finished);
        assert!(y_half > 0.0 && y_half < 200.0);
        // Cubic ease-out at 0.5 is 1 - 0.5^3 = 0.875, so y > 150
        assert!(y_half > 150.0);

        // Step past completion
        let t_end = anim.start_time + std::time::Duration::from_millis(350);
        let (y_end, finished) = anim.step(t_end);
        assert!(finished);
        assert_eq!(y_end, 200.0);
        assert!(!anim.active);

        // Retargeting
        anim.update_target(400.0, std::time::Duration::from_millis(200));
        assert!(anim.active);
        assert_eq!(anim.start_y, 200.0);
        assert_eq!(anim.target_y, 400.0);
    }
}
