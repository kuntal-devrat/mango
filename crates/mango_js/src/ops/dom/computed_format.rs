//! Helper formatting functions for computed style serialization.

use mango_core::Color;

pub fn format_padding_length(
    l: &mango_css::values::Length,
    font_size: f32,
    root_font_size: f32,
    containing_block_width: Option<f32>,
) -> String {
    let cb_w = containing_block_width.unwrap_or(0.0);
    let px = l.to_px(font_size, root_font_size, cb_w).max(0.0);
    let rounded = (px * 1000.0).round() / 1000.0;
    if (rounded - rounded.round()).abs() < 1e-4 {
        format!("{:.0}px", rounded.round())
    } else {
        format!("{rounded}px")
    }
}

pub fn serialize_4_box(t: &str, r: &str, b: &str, l: &str) -> String {
    if t == r && r == b && b == l {
        t.to_string()
    } else if t == b && r == l {
        format!("{t} {r}")
    } else if r == l {
        format!("{t} {r} {b}")
    } else {
        format!("{t} {r} {b} {l}")
    }
}

pub fn format_css_length(
    l: &mango_css::values::Length,
    font_size: f32,
    root_font_size: f32,
    containing_block_width: Option<f32>,
) -> String {
    match l {
        mango_css::values::Length::Px(px) => {
            let rounded = (*px * 1000.0).round() / 1000.0;
            if (rounded - rounded.round()).abs() < 1e-4 {
                format!("{:.0}px", rounded.round())
            } else {
                format!("{rounded}px")
            }
        }
        mango_css::values::Length::Percent(pct) => {
            if let Some(cb_width) = containing_block_width {
                let px = (pct / 100.0) * cb_width;
                let rounded = (px * 1000.0).round() / 1000.0;
                if (rounded - rounded.round()).abs() < 1e-4 {
                    format!("{:.0}px", rounded.round())
                } else {
                    format!("{rounded}px")
                }
            } else if pct.fract() == 0.0 {
                format!("{:.0}%", pct)
            } else {
                format!("{pct}%")
            }
        }
        mango_css::values::Length::Auto => "auto".to_string(),
        mango_css::values::Length::Content => "content".to_string(),
        mango_css::values::Length::MinContent => "min-content".to_string(),
        mango_css::values::Length::MaxContent => "max-content".to_string(),
        mango_css::values::Length::FitContent => "fit-content".to_string(),
        mango_css::values::Length::Calc(calc) => {
            if containing_block_width.is_none() && calc.has_percent {
                let resolved_px = calc.px
                    + calc.em * font_size
                    + calc.ex * 0.5 * font_size
                    + calc.rem * root_font_size;
                let pct_rounded = (calc.percent * 1000.0).round() / 1000.0;
                let pct_str = if (pct_rounded - pct_rounded.round()).abs() < 1e-4 {
                    format!("{:.0}%", pct_rounded.round())
                } else {
                    format!("{pct_rounded}%")
                };
                if (resolved_px * 1000.0).round().abs() < 1e-4 {
                    pct_str
                } else {
                    let px_rounded = (resolved_px.abs() * 1000.0).round() / 1000.0;
                    let px_str = if (px_rounded - px_rounded.round()).abs() < 1e-4 {
                        format!("{:.0}px", px_rounded.round())
                    } else {
                        format!("{px_rounded}px")
                    };
                    if resolved_px > 0.0 {
                        format!("calc({pct_str} + {px_str})")
                    } else {
                        format!("calc({pct_str} - {px_str})")
                    }
                }
            } else {
                let cb_w = containing_block_width.unwrap_or(0.0);
                let px = l.to_px(font_size, root_font_size, cb_w);
                let rounded = (px * 1000.0).round() / 1000.0;
                if (rounded - rounded.round()).abs() < 1e-4 {
                    format!("{:.0}px", rounded.round())
                } else {
                    format!("{rounded}px")
                }
            }
        }
        mango_css::values::Length::Math(_)
        | mango_css::values::Length::Em(_)
        | mango_css::values::Length::Ex(_)
        | mango_css::values::Length::Rem(_)
        | mango_css::values::Length::Vw(_)
        | mango_css::values::Length::Vh(_)
        | mango_css::values::Length::Vmin(_)
        | mango_css::values::Length::Vmax(_) => {
            let cb_w = containing_block_width.unwrap_or(0.0);
            let px = l.to_px(font_size, root_font_size, cb_w);
            let rounded = (px * 1000.0).round() / 1000.0;
            if (rounded - rounded.round()).abs() < 1e-4 {
                format!("{:.0}px", rounded.round())
            } else {
                format!("{rounded}px")
            }
        }
        _ => "0px".to_string(),
    }
}

pub fn format_css_color(c: &Color) -> String {
    if c.a == 255 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else if c.a == 0 {
        "rgba(0, 0, 0, 0)".to_string()
    } else {
        format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, c.a as f32 / 255.0)
    }
}

pub fn format_border_style(s: mango_css::values::BorderStyle) -> &'static str {
    match s {
        mango_css::values::BorderStyle::None => "none",
        mango_css::values::BorderStyle::Solid => "solid",
        mango_css::values::BorderStyle::Dashed => "dashed",
        mango_css::values::BorderStyle::Dotted => "dotted",
        mango_css::values::BorderStyle::Double => "double",
        mango_css::values::BorderStyle::Hidden => "hidden",
        mango_css::values::BorderStyle::Groove => "groove",
        mango_css::values::BorderStyle::Ridge => "ridge",
        mango_css::values::BorderStyle::Inset => "inset",
        mango_css::values::BorderStyle::Outset => "outset",
    }
}
