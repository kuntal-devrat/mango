//! Context menu subsystem for Mango Browser (OPT-001).
//!
//! Provides popup context menus for right-click interactions on pages, links,
//! and media elements with theme styling and item hit-testing.

use mango_core::{Color, Rect};
use mango_render::display_list::{BorderWidths, DisplayCommand};
use mango_render::font::{FontFamily, FontWeight};

/// Actions triggered from the right-click context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuAction {
    Back,
    Forward,
    Reload,
    CopyLink(String),
    ViewSource,
    Inspect,
    ToggleMediaPlay(mango_html::dom::NodeId, bool),
    ToggleMediaMute(mango_html::dom::NodeId, bool),
}

/// An item in the right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub action: ContextMenuAction,
    pub enabled: bool,
}

/// The active popup right-click context menu.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub items: Vec<ContextMenuItem>,
    pub hovered_idx: Option<usize>,
}

impl ContextMenu {
    /// Creates a new context menu positioned at (x, y) with the provided items.
    pub fn new(x: f32, y: f32, items: Vec<ContextMenuItem>, window_w: f32, window_h: f32) -> Self {
        let menu_w = 175.0f32;
        let menu_h = items.len() as f32 * 26.0 + 8.0;

        let clamped_x = x.min((window_w - menu_w - 4.0).max(0.0));
        let clamped_y = y.min((window_h - menu_h - 4.0).max(0.0));

        Self {
            x: clamped_x,
            y: clamped_y,
            width: menu_w,
            height: menu_h,
            items,
            hovered_idx: None,
        }
    }

    /// Tests whether the given coordinates are within the context menu boundary.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Updates the hovered item index based on mouse coordinates.
    pub fn update_hover(&mut self, px: f32, py: f32) -> bool {
        let old_hover = self.hovered_idx;
        if !self.contains(px, py) {
            self.hovered_idx = None;
        } else {
            let rel_y = py - (self.y + 4.0);
            if rel_y >= 0.0 {
                let idx = (rel_y / 26.0) as usize;
                if idx < self.items.len() {
                    self.hovered_idx = Some(idx);
                } else {
                    self.hovered_idx = None;
                }
            } else {
                self.hovered_idx = None;
            }
        }
        self.hovered_idx != old_hover
    }

    /// Hit-tests a click within the context menu and returns the selected action if enabled.
    pub fn handle_click(&self, px: f32, py: f32) -> Option<ContextMenuAction> {
        if !self.contains(px, py) {
            return None;
        }
        let rel_y = py - (self.y + 4.0);
        if rel_y >= 0.0 {
            let idx = (rel_y / 26.0) as usize;
            if let Some(item) = self.items.get(idx) {
                if item.enabled {
                    return Some(item.action.clone());
                }
            }
        }
        None
    }

    /// Emits display commands to render the context menu popup.
    pub fn render(&self) -> Vec<DisplayCommand> {
        let mut cmds = Vec::with_capacity(self.items.len() * 3 + 2);
        let menu_rect = Rect::new(self.x, self.y, self.width, self.height);

        // Menu card background
        cmds.push(DisplayCommand::FillRoundedRect {
            rect: menu_rect,
            color: Color::rgb(41, 42, 45),
            radii: [6.0, 6.0, 6.0, 6.0],
        });

        // Menu border
        cmds.push(DisplayCommand::DrawBorder {
            rect: menu_rect,
            color: Color::rgb(70, 72, 77),
            widths: BorderWidths {
                top: 1.0,
                right: 1.0,
                bottom: 1.0,
                left: 1.0,
            },
            radii: [6.0; 4],
        });

        for (i, item) in self.items.iter().enumerate() {
            let item_y = self.y + 4.0 + (i as f32 * 26.0);
            let item_rect = Rect::new(self.x + 4.0, item_y, self.width - 8.0, 24.0);

            if self.hovered_idx == Some(i) && item.enabled {
                cmds.push(DisplayCommand::FillRoundedRect {
                    rect: item_rect,
                    color: Color::rgb(70, 72, 77),
                    radii: [4.0, 4.0, 4.0, 4.0],
                });
            }

            let text_color = if item.enabled {
                Color::rgb(232, 234, 237)
            } else {
                Color::rgb(112, 115, 120)
            };

            cmds.push(DisplayCommand::draw_text(
                item.label.clone(),
                self.x + 12.0,
                item_y + 4.0,
                text_color,
                13.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            ));
        }

        cmds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_creation_and_bounds() {
        let items = vec![
            ContextMenuItem {
                label: "Back".to_string(),
                action: ContextMenuAction::Back,
                enabled: true,
            },
            ContextMenuItem {
                label: "Forward".to_string(),
                action: ContextMenuAction::Forward,
                enabled: false,
            },
        ];
        let menu = ContextMenu::new(100.0, 200.0, items, 800.0, 600.0);
        assert_eq!(menu.x, 100.0);
        assert_eq!(menu.y, 200.0);
        assert!(menu.contains(110.0, 210.0));
        assert!(!menu.contains(50.0, 50.0));

        let action = menu.handle_click(110.0, 210.0);
        assert_eq!(action, Some(ContextMenuAction::Back));

        // Disabled item returns None
        let disabled_action = menu.handle_click(110.0, 235.0);
        assert_eq!(disabled_action, None);
    }
}
