use mango_core::{Color, Rect};
use mango_render::display_list::{DisplayCommand, DisplayList};

use crate::box_tree::LayoutBox;

use super::types::apply_opacity;

pub(crate) fn render_progress_element(
    box_node: &LayoutBox,
    _style: &mango_css::computed::ComputedStyle,
    opacity: f32,
    list: &mut DisplayList,
    content: Rect,
) {
    if content.width() < 4.0 || content.height() < 4.0 {
        return;
    }
    let track_color = apply_opacity(Color::rgb(230, 230, 230), opacity);
    let border_color = apply_opacity(Color::rgb(190, 195, 200), opacity);
    let radii = [3.0; 4];

    // Track
    list.push(DisplayCommand::FillRoundedRect {
        rect: content,
        color: track_color,
        radii,
    });
    list.push(DisplayCommand::DrawBorder {
        rect: content,
        color: border_color,
        widths: mango_render::display_list::BorderWidths {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        },
        radii,
    });

    let max_val = box_node
        .get_attribute("max")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|&m| m > 0.0)
        .unwrap_or(1.0);

    let accent = _style.accent_color.unwrap_or(Color::rgb(0, 120, 215));
    let fill_color = apply_opacity(accent, opacity); // Progress bar fill

    if let Some(val_str) = box_node.get_attribute("value") {
        // Determinate progress
        if let Ok(val) = val_str.parse::<f32>() {
            let ratio = (val / max_val).clamp(0.0, 1.0);
            let fill_w = (content.width() * ratio).max(0.0);
            if fill_w > 0.0 {
                let fill_rect = Rect::new(content.x(), content.y(), fill_w, content.height());
                list.push(DisplayCommand::FillRoundedRect {
                    rect: fill_rect,
                    color: fill_color,
                    radii,
                });
            }
        }
    } else {
        // Indeterminate progress: active indicator centered
        let indet_w = (content.width() * 0.35).clamp(16.0, content.width());
        let indet_x = content.x() + (content.width() - indet_w) * 0.5;
        let indet_rect = Rect::new(indet_x, content.y(), indet_w, content.height());
        list.push(DisplayCommand::FillRoundedRect {
            rect: indet_rect,
            color: fill_color,
            radii,
        });
    }
}

pub(crate) fn render_meter_element(
    box_node: &LayoutBox,
    _style: &mango_css::computed::ComputedStyle,
    opacity: f32,
    list: &mut DisplayList,
    content: Rect,
) {
    if content.width() < 4.0 || content.height() < 4.0 {
        return;
    }
    let track_color = apply_opacity(Color::rgb(235, 238, 242), opacity);
    let border_color = apply_opacity(Color::rgb(180, 185, 190), opacity);
    let radii = [3.0; 4];

    // Gauge track
    list.push(DisplayCommand::FillRoundedRect {
        rect: content,
        color: track_color,
        radii,
    });
    list.push(DisplayCommand::DrawBorder {
        rect: content,
        color: border_color,
        widths: mango_render::display_list::BorderWidths {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        },
        radii,
    });

    let min_val = box_node
        .get_attribute("min")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let max_val = box_node
        .get_attribute("max")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0)
        .max(min_val);
    let val = box_node
        .get_attribute("value")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val)
        .clamp(min_val, max_val);

    let low_val = box_node
        .get_attribute("low")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val)
        .clamp(min_val, max_val);
    let high_val = box_node
        .get_attribute("high")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(max_val)
        .clamp(low_val, max_val);
    let opt_val = box_node
        .get_attribute("optimum")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(min_val + (max_val - min_val) / 2.0)
        .clamp(min_val, max_val);

    // HTML5 §4.10.14: Color zones (green, yellow, red)
    let fill_color = if opt_val >= high_val {
        // High is optimal
        if val >= high_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else if val >= low_val {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        } else {
            Color::rgb(220, 53, 69) // Red (poor)
        }
    } else if opt_val <= low_val {
        // Low is optimal
        if val <= low_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else if val <= high_val {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        } else {
            Color::rgb(220, 53, 69) // Red (poor)
        }
    } else {
        // Middle is optimal
        if val >= low_val && val <= high_val {
            Color::rgb(40, 167, 69) // Green (optimal)
        } else {
            Color::rgb(255, 193, 7) // Yellow/Orange (suboptimal)
        }
    };

    let span = (max_val - min_val).max(0.0001);
    let ratio = ((val - min_val) / span).clamp(0.0, 1.0);
    let fill_w = (content.width() * ratio).max(0.0);
    if fill_w > 0.0 {
        let fill_rect = Rect::new(content.x(), content.y(), fill_w, content.height());
        list.push(DisplayCommand::FillRoundedRect {
            rect: fill_rect,
            color: apply_opacity(fill_color, opacity),
            radii,
        });
    }
}
