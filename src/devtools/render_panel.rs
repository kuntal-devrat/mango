use mango_core::{Color, Rect};
use mango_html::dom::{Document, NodeData, NodeId};
use mango_layout::box_tree::LayoutBox;
use mango_render::display_list::DisplayCommand;
use mango_render::font::{FontFamily, FontStyle, FontWeight, TextDecoration};

use super::types::{DevTools, DevToolsTab};

impl DevTools {
    /// Renders the complete DevTools panel into display commands.
    pub fn render_panel(
        &self,
        width: f32,
        window_height: f32,
        doc: &Document,
        root_box: Option<&LayoutBox>,
    ) -> Vec<DisplayCommand> {
        let mut cmds = Vec::new();
        if !self.is_open {
            return cmds;
        }

        let panel_y = (window_height - self.height).max(0.0);
        let panel_h = self.height;

        // 1. Panel background (dark VS Code theme)
        cmds.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, panel_y, width, panel_h),
            color: Color::rgb(30, 30, 30),
        });

        // 2. Panel top divider bar
        cmds.push(DisplayCommand::DrawLine {
            x1: 0.0,
            y1: panel_y,
            x2: width,
            y2: panel_y,
            color: Color::rgb(60, 60, 60),
            thickness: 1.5,
        });

        // 3. Tab bar (26px height)
        let tab_bar_h = 26.0;
        cmds.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, panel_y, width, tab_bar_h),
            color: Color::rgb(40, 40, 40),
        });

        // Tabs
        let tabs = [
            (DevToolsTab::Elements, "Elements"),
            (DevToolsTab::Console, "Console"),
            (DevToolsTab::Network, "Network"),
            (DevToolsTab::Sources, "Sources"),
            (DevToolsTab::Computed, "Computed"),
            (DevToolsTab::Performance, "Perf"),
            (DevToolsTab::Debugger, "Debugger"),
        ];

        let mut tab_x = 10.0;
        for (tab_type, title) in tabs {
            let is_active = self.active_tab == tab_type;
            let tab_w = 68.0;

            if is_active {
                cmds.push(DisplayCommand::FillRect {
                    rect: Rect::new(tab_x, panel_y, tab_w, tab_bar_h),
                    color: Color::rgb(30, 30, 30),
                });
                cmds.push(DisplayCommand::DrawLine {
                    x1: tab_x,
                    y1: panel_y,
                    x2: tab_x + tab_w,
                    y2: panel_y,
                    color: Color::rgb(255, 161, 54), // Mango orange active indicator
                    thickness: 2.0,
                });
            }

            let text_color = if is_active {
                Color::rgb(240, 240, 240)
            } else {
                Color::rgb(150, 150, 150)
            };

            let label = match tab_type {
                DevToolsTab::Console if !self.console_entries.is_empty() => {
                    format!("{} ({})", title, self.console_entries.len())
                }
                DevToolsTab::Network if !self.network_entries.is_empty() => {
                    format!("{} ({})", title, self.network_entries.len())
                }
                DevToolsTab::Performance if !self.performance_metrics.is_empty() => {
                    format!("{} ({})", title, self.performance_metrics.len())
                }
                DevToolsTab::Debugger if !self.breakpoints.is_empty() => {
                    format!("{} ({})", title, self.breakpoints.len())
                }
                _ => title.to_string(),
            };

            cmds.push(DisplayCommand::DrawText {
                text: label,
                x: tab_x + 6.0,
                y: panel_y + 17.0,
                color: text_color,
                font_size: 10.5,
                weight: if is_active {
                    FontWeight::Bold
                } else {
                    FontWeight::Regular
                },
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });

            tab_x += tab_w + 4.0;
        }

        // Close button [X]
        cmds.push(DisplayCommand::DrawText {
            text: "×".to_string(),
            x: width - 20.0,
            y: panel_y + 18.0,
            color: Color::rgb(160, 160, 160),
            font_size: 16.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });

        // 4. Panel Content Area
        let content_y = panel_y + tab_bar_h;
        let content_h = panel_h - tab_bar_h;

        match self.active_tab {
            DevToolsTab::Elements => {
                self.render_elements_tab(&mut cmds, width, content_y, content_h, doc, root_box);
            }
            DevToolsTab::Console => {
                self.render_console_tab(&mut cmds, width, content_y, content_h);
            }
            DevToolsTab::Network => {
                self.render_network_tab(&mut cmds, width, content_y, content_h);
            }
            DevToolsTab::Sources => {
                self.render_sources_tab(&mut cmds, width, content_y, content_h, doc);
            }
            DevToolsTab::Computed => {
                self.render_computed_tab(&mut cmds, width, content_y, content_h, doc, root_box);
            }
            DevToolsTab::Performance => {
                self.render_performance_tab(&mut cmds, width, content_y, content_h);
            }
            DevToolsTab::Debugger => {
                self.render_debugger_tab(&mut cmds, width, content_y, content_h);
            }
        }

        cmds
    }

    pub(crate) fn render_elements_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        width: f32,
        content_y: f32,
        content_h: f32,
        doc: &Document,
        root_box: Option<&LayoutBox>,
    ) {
        let left_w = (width * 0.6).max(200.0);
        let right_x = left_w;
        let _right_w = width - left_w;

        // Vertical pane split line
        cmds.push(DisplayCommand::DrawLine {
            x1: right_x,
            y1: content_y,
            x2: right_x,
            y2: content_y + content_h,
            color: Color::rgb(50, 50, 50),
            thickness: 1.0,
        });

        // Left Pane: DOM Elements Tree
        let mut row_y = content_y + 16.0;
        let mut lines = Vec::new();
        format_dom_tree(doc, doc.root(), 0, &mut lines);

        for (node_id, depth, text) in lines.iter().take(12) {
            let is_selected = self.selected_node_id == Some(*node_id);
            if is_selected {
                cmds.push(DisplayCommand::FillRect {
                    rect: Rect::new(0.0, row_y - 12.0, left_w, 16.0),
                    color: Color::rgb(9, 71, 113),
                });
            }

            let indent_x = 12.0 + (*depth as f32 * 14.0);
            cmds.push(DisplayCommand::DrawText {
                text: text.clone(),
                x: indent_x,
                y: row_y,
                color: if is_selected {
                    Color::WHITE
                } else {
                    Color::rgb(86, 156, 214)
                },
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });

            row_y += 18.0;
        }

        // Right Pane: Computed Styles & Box Metrics
        let style_x = right_x + 12.0;
        let mut style_y = content_y + 16.0;

        cmds.push(DisplayCommand::DrawText {
            text: "Styles & Box Metrics".to_string(),
            x: style_x,
            y: style_y,
            color: Color::rgb(200, 200, 200),
            font_size: 12.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        style_y += 22.0;

        if let Some(selected_id) = self.selected_node_id
            && let Some(root) = root_box
            && let Some(target_box) = root.find_box_for_node(selected_id)
        {
            let bb = target_box.dimensions.border_box();
            let pad = target_box.dimensions.padding;
            let mar = target_box.dimensions.margin;

            let metrics = [
                format!("Dimensions: {:.1} × {:.1} px", bb.width(), bb.height()),
                format!("Position: ({:.1}, {:.1})", bb.x(), bb.y()),
                format!(
                    "Margin: {:.0} {:.0} {:.0} {:.0}",
                    mar.top, mar.right, mar.bottom, mar.left
                ),
                format!(
                    "Padding: {:.0} {:.0} {:.0} {:.0}",
                    pad.top, pad.right, pad.bottom, pad.left
                ),
                format!("Display: {:?}", target_box.box_type),
            ];

            for line in metrics {
                cmds.push(DisplayCommand::DrawText {
                    text: line,
                    x: style_x,
                    y: style_y,
                    color: Color::rgb(180, 180, 180),
                    font_size: 11.0,
                    weight: FontWeight::Regular,
                    family: FontFamily::Monospace,
                    style: FontStyle::Normal,
                    decoration: TextDecoration::None,
                    letter_spacing: 0.0,
                });
                style_y += 18.0;
            }
        } else {
            cmds.push(DisplayCommand::DrawText {
                text: "Select an element to view computed metrics".to_string(),
                x: style_x,
                y: style_y,
                color: Color::rgb(120, 120, 120),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::SansSerif,
                style: FontStyle::Italic,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
        }
    }

    /// Renders a semi-transparent inspect highlight overlay on top of the inspected element.
    pub fn render_highlight(
        &self,
        root_box: Option<&LayoutBox>,
        scroll_y: f32,
        content_y: f32,
    ) -> Vec<DisplayCommand> {
        let mut cmds = Vec::new();
        let target_node_id = self.hovered_node_id.or(self.selected_node_id);
        let Some(target_id) = target_node_id else {
            return cmds;
        };
        let Some(root) = root_box else {
            return cmds;
        };
        let Some(target_box) = root.find_box_for_node(target_id) else {
            return cmds;
        };

        let bb = target_box.dimensions.border_box();
        let translated_y = bb.y() + content_y - scroll_y;

        // Semi-transparent blue fill
        cmds.push(DisplayCommand::FillRect {
            rect: Rect::new(bb.x(), translated_y, bb.width(), bb.height()),
            color: Color::rgba(66, 133, 244, 80),
        });

        // Blue border outline
        cmds.push(DisplayCommand::DrawBorder {
            rect: Rect::new(bb.x(), translated_y, bb.width(), bb.height()),
            color: Color::rgb(66, 133, 244),
            widths: mango_render::BorderWidths::all(1.0),
            radii: [0.0; 4],
        });

        // Small badge showing dimensions
        let badge_text = format!("{:.0} × {:.0}", bb.width(), bb.height());
        let badge_y = (translated_y - 18.0).max(content_y);
        cmds.push(DisplayCommand::FillRect {
            rect: Rect::new(bb.x(), badge_y, 70.0, 16.0),
            color: Color::rgb(33, 33, 33),
        });
        cmds.push(DisplayCommand::DrawText {
            text: badge_text,
            x: bb.x() + 4.0,
            y: badge_y + 12.0,
            color: Color::WHITE,
            font_size: 10.0,
            weight: FontWeight::Bold,
            family: FontFamily::Monospace,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });

        cmds
    }
}

pub(crate) fn format_dom_tree(
    doc: &Document,
    node_id: NodeId,
    depth: usize,
    out: &mut Vec<(NodeId, usize, String)>,
) {
    let Some(node) = doc.get(node_id) else {
        return;
    };
    match &node.data {
        NodeData::Element(elem) => {
            let mut label = format!("<{}", elem.tag_name);
            if let Some(id) = elem.id() {
                label.push_str(&format!(" id=\"{id}\""));
            }
            if let Some(class) = elem.get_attribute("class") {
                label.push_str(&format!(" class=\"{class}\""));
            }
            label.push('>');

            out.push((node_id, depth, label));

            for child in doc.children(node_id) {
                format_dom_tree(doc, child.id, depth + 1, out);
            }
        }
        NodeData::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let preview = if trimmed.len() > 24 {
                    format!("\"{}...\"", &trimmed[..24])
                } else {
                    format!("\"{trimmed}\"")
                };
                out.push((node_id, depth, preview));
            }
        }
        NodeData::Document => {
            for child in doc.children(node_id) {
                format_dom_tree(doc, child.id, depth, out);
            }
        }
        _ => {}
    }
}
