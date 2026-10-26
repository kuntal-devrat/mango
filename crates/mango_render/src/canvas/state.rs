//! Canvas state and font parsing for the 2D rendering context.

use mango_core::Color;
use tiny_skia::{LineCap, LineJoin, Transform};

use crate::font::{FontFamily, FontStyle, FontWeight, font_manager};
use crate::painter::effects::RadialGradient;

/// Paint style for canvas fill and stroke operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasPaintStyle {
    Color(Color),
    RadialGradient(RadialGradient),
}

impl Default for CanvasPaintStyle {
    fn default() -> Self {
        Self::Color(Color::BLACK)
    }
}

/// Drawing state saved and restored on the canvas state stack.
#[derive(Debug, Clone)]
pub struct CanvasState {
    pub transform: Transform,
    pub fill_color: Color,
    pub stroke_color: Color,
    pub fill_style: CanvasPaintStyle,
    pub stroke_style: CanvasPaintStyle,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub global_alpha: f32,
    pub font_size: f32,
    pub font_family: FontFamily,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_str: String,
    pub text_align: String,    // "left", "right", "center", "start", "end"
    pub text_baseline: String, // "top", "hanging", "middle", "alphabetic", "ideographic", "bottom"
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            transform: Transform::identity(),
            fill_color: Color::BLACK,
            stroke_color: Color::BLACK,
            fill_style: CanvasPaintStyle::Color(Color::BLACK),
            stroke_style: CanvasPaintStyle::Color(Color::BLACK),
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            global_alpha: 1.0,
            font_size: 10.0,
            font_family: FontFamily::SansSerif,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            font_str: "10px sans-serif".to_string(),
            text_align: "start".to_string(),
            text_baseline: "alphabetic".to_string(),
        }
    }
}

pub(crate) fn parse_canvas_font(font_str: &str, state: &mut CanvasState) {
    let lower = font_str.to_ascii_lowercase();
    let mut words = lower.split_whitespace().peekable();

    while let Some(word) = words.peek() {
        match *word {
            "italic" | "oblique" => {
                state.font_style = FontStyle::Italic;
                words.next();
            }
            "bold" | "bolder" | "700" | "800" | "900" => {
                state.font_weight = FontWeight::Bold;
                words.next();
            }
            "normal" => {
                state.font_weight = FontWeight::Regular;
                state.font_style = FontStyle::Normal;
                words.next();
            }
            w if w.ends_with("px") => {
                if let Ok(sz) = w.trim_end_matches("px").parse::<f32>() {
                    state.font_size = sz.max(1.0);
                }
                words.next();
                break;
            }
            w if w.parse::<f32>().is_ok() => {
                if let Ok(sz) = w.parse::<f32>() {
                    state.font_size = sz.max(1.0);
                }
                words.next();
                break;
            }
            _ => {
                words.next();
            }
        }
    }

    let remainder: Vec<&str> = words.collect();
    if !remainder.is_empty() {
        let fam_str = remainder.join(" ");
        state.font_family = font_manager().resolve_family_for(&fam_str);
    }
}
