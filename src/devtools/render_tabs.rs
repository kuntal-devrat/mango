use mango_core::Color;
use mango_html::dom::{Document, NodeData, NodeId};
use mango_layout::box_tree::LayoutBox;
use mango_render::display_list::DisplayCommand;
use mango_render::font::{FontFamily, FontStyle, FontWeight, TextDecoration};

use super::types::{ConsoleLevel, DevTools};

impl DevTools {
    pub(crate) fn render_console_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        width: f32,
        content_y: f32,
        _content_h: f32,
    ) {
        let mut row_y = content_y + 16.0;

        if self.console_entries.is_empty() {
            cmds.push(DisplayCommand::DrawText {
                text: "No console output recorded".to_string(),
                x: 16.0,
                y: row_y,
                color: Color::rgb(120, 120, 120),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::SansSerif,
                style: FontStyle::Italic,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            return;
        }

        for entry in self.console_entries.iter().rev().take(12) {
            let (icon, color) = match entry.level {
                ConsoleLevel::Log => ("[log]", Color::rgb(204, 204, 204)),
                ConsoleLevel::Info => ("[info]", Color::rgb(79, 193, 255)),
                ConsoleLevel::Warn => ("[warn]", Color::rgb(220, 200, 100)),
                ConsoleLevel::Error => ("[error]", Color::rgb(241, 76, 76)),
            };

            cmds.push(DisplayCommand::DrawText {
                text: format!("{icon} {}", entry.message),
                x: 16.0,
                y: row_y,
                color,
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });

            // subtle divider
            cmds.push(DisplayCommand::DrawLine {
                x1: 0.0,
                y1: row_y + 4.0,
                x2: width,
                y2: row_y + 4.0,
                color: Color::rgb(45, 45, 45),
                thickness: 0.5,
            });

            row_y += 18.0;
        }
    }

    pub(crate) fn render_network_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        width: f32,
        content_y: f32,
        _content_h: f32,
    ) {
        let mut row_y = content_y + 16.0;

        // Header row
        cmds.push(DisplayCommand::DrawText {
            text: "Method   Status   MIME Type        Size     URL".to_string(),
            x: 16.0,
            y: row_y,
            color: Color::rgb(140, 140, 140),
            font_size: 11.0,
            weight: FontWeight::Bold,
            family: FontFamily::Monospace,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 18.0;

        if self.network_entries.is_empty() {
            cmds.push(DisplayCommand::DrawText {
                text: "No network requests recorded".to_string(),
                x: 16.0,
                y: row_y,
                color: Color::rgb(120, 120, 120),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::SansSerif,
                style: FontStyle::Italic,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            return;
        }

        for entry in self.network_entries.iter().rev().take(10) {
            let status_color = if entry.status >= 400 {
                Color::rgb(241, 76, 76)
            } else {
                Color::rgb(78, 201, 176)
            };

            let line = format!(
                "{:<8} {:<8} {:<16} {:<8} {}",
                entry.method,
                entry.status,
                entry.mime_type,
                format!("{} B", entry.size_bytes),
                entry.url
            );

            cmds.push(DisplayCommand::DrawText {
                text: line,
                x: 16.0,
                y: row_y,
                color: status_color,
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });

            cmds.push(DisplayCommand::DrawLine {
                x1: 0.0,
                y1: row_y + 4.0,
                x2: width,
                y2: row_y + 4.0,
                color: Color::rgb(45, 45, 45),
                thickness: 0.5,
            });

            row_y += 18.0;
        }
    }

    pub(crate) fn render_sources_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        _width: f32,
        content_y: f32,
        _content_h: f32,
        doc: &Document,
    ) {
        let mut row_y = content_y + 16.0;

        cmds.push(DisplayCommand::DrawText {
            text: "Document Source Summary:".to_string(),
            x: 16.0,
            y: row_y,
            color: Color::rgb(200, 200, 200),
            font_size: 12.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 20.0;

        let title = doc
            .find_element_by_tag(doc.root(), "title")
            .map(|t| doc.text_content(t))
            .unwrap_or_else(|| "Untitled".to_string());

        let scripts_count = count_tag(doc, doc.root(), "script");
        let stylesheets_count =
            count_tag(doc, doc.root(), "link") + count_tag(doc, doc.root(), "style");

        let stats = [
            format!("Title: {}", title.trim()),
            format!("Scripts: {} tag(s)", scripts_count),
            format!("Stylesheets: {} tag(s)", stylesheets_count),
        ];

        for stat in stats {
            cmds.push(DisplayCommand::DrawText {
                text: stat,
                x: 16.0,
                y: row_y,
                color: Color::rgb(180, 180, 180),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            row_y += 18.0;
        }
    }

    pub(crate) fn render_computed_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        _width: f32,
        content_y: f32,
        _content_h: f32,
        _doc: &Document,
        root_box: Option<&LayoutBox>,
    ) {
        let mut row_y = content_y + 16.0;

        cmds.push(DisplayCommand::DrawText {
            text: "Computed CSS Properties & Cascade Traceability:".to_string(),
            x: 16.0,
            y: row_y,
            color: Color::rgb(200, 200, 200),
            font_size: 12.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 22.0;

        if let Some(selected_id) = self.selected_node_id
            && let Some(root) = root_box
            && let Some(target_box) = root.find_box_for_node(selected_id)
        {
            let bb = target_box.dimensions.border_box();
            let pad = target_box.dimensions.padding;
            let mar = target_box.dimensions.margin;

            let computed_props = [
                (
                    "display",
                    format!("{:?}", target_box.box_type),
                    "Author Stylesheet",
                ),
                ("position", "static".to_string(), "User Agent"),
                ("box-sizing", "content-box".to_string(), "User Agent"),
                ("width", format!("{:.1}px", bb.width()), "Calculated Layout"),
                (
                    "height",
                    format!("{:.1}px", bb.height()),
                    "Calculated Layout",
                ),
                ("margin-top", format!("{:.0}px", mar.top), "Inline Style"),
                (
                    "margin-right",
                    format!("{:.0}px", mar.right),
                    "Inline Style",
                ),
                (
                    "margin-bottom",
                    format!("{:.0}px", mar.bottom),
                    "Inline Style",
                ),
                ("margin-left", format!("{:.0}px", mar.left), "Inline Style"),
                ("padding-top", format!("{:.0}px", pad.top), "Inline Style"),
                (
                    "padding-right",
                    format!("{:.0}px", pad.right),
                    "Inline Style",
                ),
                (
                    "padding-bottom",
                    format!("{:.0}px", pad.bottom),
                    "Inline Style",
                ),
                ("padding-left", format!("{:.0}px", pad.left), "Inline Style"),
            ];

            for (prop, val, origin) in computed_props {
                let line = format!("{:<16} {:<18} [{}]", prop, val, origin);
                cmds.push(DisplayCommand::DrawText {
                    text: line,
                    x: 16.0,
                    y: row_y,
                    color: Color::rgb(180, 220, 255),
                    font_size: 11.0,
                    weight: FontWeight::Regular,
                    family: FontFamily::Monospace,
                    style: FontStyle::Normal,
                    decoration: TextDecoration::None,
                    letter_spacing: 0.0,
                });
                row_y += 18.0;
            }
        } else {
            cmds.push(DisplayCommand::DrawText {
                text: "Select an element in Elements tab to inspect computed cascade".to_string(),
                x: 16.0,
                y: row_y,
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

    pub(crate) fn render_performance_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        _width: f32,
        content_y: f32,
        _content_h: f32,
    ) {
        let mut row_y = content_y + 16.0;

        cmds.push(DisplayCommand::DrawText {
            text: "Performance Profiler & User Timing:".to_string(),
            x: 16.0,
            y: row_y,
            color: Color::rgb(200, 200, 200),
            font_size: 12.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 22.0;

        if self.performance_metrics.is_empty() {
            cmds.push(DisplayCommand::DrawText {
                text: "No performance marks or measures recorded (use performance.mark / performance.measure)".to_string(),
                x: 16.0,
                y: row_y,
                color: Color::rgb(120, 120, 120),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::SansSerif,
                style: FontStyle::Italic,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            return;
        }

        for metric in self.performance_metrics.iter().rev().take(10) {
            let line = format!(
                "[{}] {:<24} start: {:>8.2}ms  duration: {:>8.2}ms",
                metric.category, metric.name, metric.start_time_ms, metric.duration_ms
            );
            cmds.push(DisplayCommand::DrawText {
                text: line,
                x: 16.0,
                y: row_y,
                color: Color::rgb(255, 180, 80),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            row_y += 18.0;
        }
    }

    pub(crate) fn render_debugger_tab(
        &self,
        cmds: &mut Vec<DisplayCommand>,
        _width: f32,
        content_y: f32,
        _content_h: f32,
    ) {
        let mut row_y = content_y + 16.0;

        let status_str = if self.is_paused {
            "Paused on Breakpoint"
        } else {
            "Debugger Running (Ready)"
        };

        cmds.push(DisplayCommand::DrawText {
            text: format!("JavaScript Debugger - {}", status_str),
            x: 16.0,
            y: row_y,
            color: if self.is_paused {
                Color::rgb(255, 100, 100)
            } else {
                Color::rgb(100, 220, 100)
            },
            font_size: 12.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 22.0;

        // Breakpoints
        cmds.push(DisplayCommand::DrawText {
            text: format!("Breakpoints ({}):", self.breakpoints.len()),
            x: 16.0,
            y: row_y,
            color: Color::rgb(200, 200, 200),
            font_size: 11.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 18.0;

        for bp in self.breakpoints.iter().take(4) {
            let status = if bp.enabled { "enabled" } else { "disabled" };
            let line = format!(
                "  #{}: {}:{} [{}]",
                bp.id, bp.script_url, bp.line_number, status
            );
            cmds.push(DisplayCommand::DrawText {
                text: line,
                x: 16.0,
                y: row_y,
                color: Color::rgb(160, 160, 160),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            row_y += 18.0;
        }

        // Call Stack
        cmds.push(DisplayCommand::DrawText {
            text: format!("Call Stack ({} frames):", self.call_stack.len()),
            x: 16.0,
            y: row_y,
            color: Color::rgb(200, 200, 200),
            font_size: 11.0,
            weight: FontWeight::Bold,
            family: FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        });
        row_y += 18.0;

        for (idx, frame) in self.call_stack.iter().enumerate().take(4) {
            let line = format!(
                "  #{}: {}() at {}:{}",
                idx, frame.function_name, frame.script_url, frame.line_number
            );
            cmds.push(DisplayCommand::DrawText {
                text: line,
                x: 16.0,
                y: row_y,
                color: Color::rgb(180, 180, 255),
                font_size: 11.0,
                weight: FontWeight::Regular,
                family: FontFamily::Monospace,
                style: FontStyle::Normal,
                decoration: TextDecoration::None,
                letter_spacing: 0.0,
            });
            row_y += 18.0;
        }
    }
}

pub(crate) fn count_tag(doc: &Document, root: NodeId, tag: &str) -> usize {
    let mut count = 0;
    for child in doc.children(root) {
        if let NodeData::Element(elem) = &child.data
            && elem.tag_name.eq_ignore_ascii_case(tag)
        {
            count += 1;
        }
        count += count_tag(doc, child.id, tag);
    }
    count
}
