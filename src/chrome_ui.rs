//! Chrome UI definitions, layouts, and rendering routines for Mango Browser (OPT-001).
//!
//! Owns Chrome tab bar metrics, toolbar geometry, omnibox styling,
//! color palette constants, and UI element hover targets.

use mango_core::{Color, Rect};
use mango_render::display_list::DisplayCommand;
use mango_render::font::{FontFamily, FontWeight};

/// Height of the Chrome-style tab bar in pixels.
pub const TAB_BAR_HEIGHT: f32 = 36.0;
/// Height of the browser toolbar (navigation + address bar) in pixels.
pub const TOOLBAR_HEIGHT: f32 = 40.0;
/// Total header height (tab bar + toolbar).
pub const HEADER_HEIGHT: f32 = TAB_BAR_HEIGHT + TOOLBAR_HEIGHT;
/// Height of the status bar at the bottom.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Number of page images fetched synchronously before the first paint.
pub const EAGER_IMAGE_PREFETCH: usize = 12;

/// Images fetched per frame from the background prefetch queue.
pub const IMAGE_PREFETCH_PER_FRAME: usize = 4;

// ── Chrome Dark Mode Palette ──
pub const TAB_BAR_BG: Color = Color::rgb(32, 33, 36); // #202124 Chrome tab strip
pub const TAB_ACTIVE_BG: Color = Color::rgb(53, 54, 58); // #35363a Active tab (matches toolbar)
pub const TAB_INACTIVE_BG: Color = Color::rgb(41, 42, 45); // #292a2d Inactive tab
pub const TAB_HOVER_BG: Color = Color::rgb(48, 49, 53); // Inactive tab hover
pub const TAB_TEXT_ACTIVE: Color = Color::rgb(241, 243, 244);
pub const TAB_TEXT_INACTIVE: Color = Color::rgb(154, 160, 166);
pub const TAB_CLOSE_HOVER: Color = Color::rgb(90, 92, 98);

pub const TOOLBAR_BG: Color = Color::rgb(53, 54, 58); // #35363a Chrome toolbar
pub const TOOLBAR_TEXT: Color = Color::rgb(232, 234, 237);
pub const TOOLBAR_TEXT_DISABLED: Color = Color::rgb(112, 115, 120);
pub const TOOLBAR_BTN_HOVER: Color = Color::rgb(70, 72, 77);

pub const ADDRESSBAR_BG: Color = Color::rgb(32, 33, 36); // #202124 Omnibox background
pub const ADDRESSBAR_BORDER: Color = Color::MANGO_ORANGE;
pub const ADDRESSBAR_TEXT: Color = Color::rgb(232, 234, 237);

pub const STATUS_BAR_BG: Color = Color::rgb(32, 33, 36);
pub const STATUS_BAR_TEXT: Color = Color::rgb(154, 160, 166);
pub const CONTENT_BG: Color = Color::WHITE;

/// Interactive UI elements that can be hovered or clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverTarget {
    Back,
    Forward,
    Reload,
    Home,
    NewTab,
    Tab(usize),
    TabClose(usize),
    AddressBar,
    Menu,
    ScrollbarThumb,
    ScrollbarTrack,
}

/// Computes the width of individual tabs on the tab strip.
pub fn calculate_tab_width(window_width: f32, tab_count: usize) -> f32 {
    let count = tab_count.max(1);
    let max_strip_w = (window_width - 42.0).max(100.0);
    (max_strip_w / count as f32).clamp(80.0, 210.0)
}

/// Returns the boundary rectangle of the omnibox address bar.
pub fn omnibox_rect(window_width: f32) -> Rect {
    let addr_x = 132.0;
    let addr_y = TAB_BAR_HEIGHT + 5.0;
    let addr_w = (window_width - 176.0).max(120.0);
    let addr_h = 30.0;
    Rect::new(addr_x, addr_y, addr_w, addr_h)
}

/// Renders the Chrome status bar at the bottom of the window.
pub fn render_status_bar(window_width: f32, window_height: f32, status_text: &str) -> Vec<DisplayCommand> {
    let mut cmds = Vec::with_capacity(3);
    let status_y = window_height - STATUS_BAR_HEIGHT;

    // Status bar background
    cmds.push(DisplayCommand::FillRect {
        rect: Rect::new(0.0, status_y, window_width, STATUS_BAR_HEIGHT),
        color: STATUS_BAR_BG,
    });

    // Top border line
    cmds.push(DisplayCommand::DrawLine {
        x1: 0.0,
        y1: status_y,
        x2: window_width,
        y2: status_y,
        color: Color::rgb(48, 49, 53),
        thickness: 1.0,
    });

    // Status text
    cmds.push(DisplayCommand::draw_text(
        status_text.to_string(),
        10.0,
        status_y + 4.0,
        STATUS_BAR_TEXT,
        11.0,
        FontWeight::Regular,
        FontFamily::SansSerif,
    ));

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_strip_metrics() {
        let tab_w = calculate_tab_width(800.0, 4);
        assert!(tab_w >= 80.0 && tab_w <= 210.0);

        let many_tabs_w = calculate_tab_width(800.0, 20);
        assert_eq!(many_tabs_w, 80.0);
    }

    #[test]
    fn test_omnibox_rect_bounds() {
        let rect = omnibox_rect(1024.0);
        assert_eq!(rect.x(), 132.0);
        assert_eq!(rect.y(), TAB_BAR_HEIGHT + 5.0);
        assert_eq!(rect.height(), 30.0);
        assert!(rect.width() > 500.0);
    }

    #[test]
    fn test_status_bar_rendering() {
        let cmds = render_status_bar(800.0, 600.0, "Ready");
        assert_eq!(cmds.len(), 3);
    }
}
