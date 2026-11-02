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

pub fn format_flex_alignment_overflow_custom(
    style: &mango_css::ComputedStyle,
    cb_w: Option<f32>,
    map: &mut std::collections::HashMap<String, String>,
) {
    let jc_str = match style.justify_content {
        mango_css::values::JustifyContent::Normal => "normal",
        mango_css::values::JustifyContent::FlexStart => "flex-start",
        mango_css::values::JustifyContent::FlexEnd => "flex-end",
        mango_css::values::JustifyContent::Center => "center",
        mango_css::values::JustifyContent::SpaceBetween => "space-between",
        mango_css::values::JustifyContent::SpaceAround => "space-around",
        mango_css::values::JustifyContent::SpaceEvenly => "space-evenly",
        mango_css::values::JustifyContent::Start => "start",
        mango_css::values::JustifyContent::End => "end",
        mango_css::values::JustifyContent::Left => "left",
        mango_css::values::JustifyContent::Right => "right",
    };
    map.insert("justify-content".to_string(), jc_str.to_string());

    let ji_str = match style.justify_items {
        mango_css::values::JustifyItems::Stretch => "stretch",
        mango_css::values::JustifyItems::FlexStart => "flex-start",
        mango_css::values::JustifyItems::FlexEnd => "flex-end",
        mango_css::values::JustifyItems::Center => "center",
        mango_css::values::JustifyItems::Baseline => "baseline",
    };
    map.insert("justify-items".to_string(), ji_str.to_string());

    let js_str = match style.justify_self {
        mango_css::values::JustifySelf::Auto => "auto",
        mango_css::values::JustifySelf::Stretch => "stretch",
        mango_css::values::JustifySelf::FlexStart => "flex-start",
        mango_css::values::JustifySelf::FlexEnd => "flex-end",
        mango_css::values::JustifySelf::Center => "center",
        mango_css::values::JustifySelf::Baseline => "baseline",
    };
    map.insert("justify-self".to_string(), js_str.to_string());

    let ai_str = match style.align_items {
        mango_css::values::AlignItems::Normal => "normal",
        mango_css::values::AlignItems::Stretch => "stretch",
        mango_css::values::AlignItems::FlexStart => "flex-start",
        mango_css::values::AlignItems::FlexEnd => "flex-end",
        mango_css::values::AlignItems::Center => "center",
        mango_css::values::AlignItems::Baseline => "baseline",
        mango_css::values::AlignItems::SafeCenter => "safe center",
        mango_css::values::AlignItems::SafeFlexStart => "safe flex-start",
        mango_css::values::AlignItems::SafeFlexEnd => "safe flex-end",
        mango_css::values::AlignItems::SafeStart => "safe start",
        mango_css::values::AlignItems::SafeEnd => "safe end",
        mango_css::values::AlignItems::LastBaseline => "last baseline",
    };
    map.insert("align-items".to_string(), ai_str.to_string());

    let as_str = match style.align_self {
        mango_css::values::AlignSelf::Auto => "auto",
        mango_css::values::AlignSelf::Stretch => "stretch",
        mango_css::values::AlignSelf::FlexStart => "flex-start",
        mango_css::values::AlignSelf::FlexEnd => "flex-end",
        mango_css::values::AlignSelf::Center => "center",
        mango_css::values::AlignSelf::Baseline => "baseline",
        mango_css::values::AlignSelf::SafeCenter => "safe center",
        mango_css::values::AlignSelf::SafeFlexStart => "safe flex-start",
        mango_css::values::AlignSelf::SafeFlexEnd => "safe flex-end",
        mango_css::values::AlignSelf::SafeStart => "safe start",
        mango_css::values::AlignSelf::SafeEnd => "safe end",
        mango_css::values::AlignSelf::LastBaseline => "last baseline",
    };
    map.insert("align-self".to_string(), as_str.to_string());

    let ac_str = match style.align_content {
        mango_css::values::AlignContent::Normal => "normal",
        mango_css::values::AlignContent::Stretch => "stretch",
        mango_css::values::AlignContent::FlexStart => "flex-start",
        mango_css::values::AlignContent::FlexEnd => "flex-end",
        mango_css::values::AlignContent::Center => "center",
        mango_css::values::AlignContent::SpaceBetween => "space-between",
        mango_css::values::AlignContent::SpaceAround => "space-around",
        mango_css::values::AlignContent::SpaceEvenly => "space-evenly",
    };
    map.insert("align-content".to_string(), ac_str.to_string());

    let rg = format_css_length(&style.row_gap, style.font_size, style.root_font_size, None);
    let cg = format_css_length(
        &style.column_gap,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    map.insert("row-gap".to_string(), rg.clone());
    map.insert("column-gap".to_string(), cg.clone());
    let gap_shorthand = if rg == cg { rg } else { format!("{rg} {cg}") };
    map.insert("gap".to_string(), gap_shorthand);

    let ox_str = match style.overflow_x {
        mango_css::values::Overflow::Visible => "visible",
        mango_css::values::Overflow::Hidden => "hidden",
        mango_css::values::Overflow::Scroll => "scroll",
        mango_css::values::Overflow::Auto => "auto",
    };
    let oy_str = match style.overflow_y {
        mango_css::values::Overflow::Visible => "visible",
        mango_css::values::Overflow::Hidden => "hidden",
        mango_css::values::Overflow::Scroll => "scroll",
        mango_css::values::Overflow::Auto => "auto",
    };
    map.insert("overflow-x".to_string(), ox_str.to_string());
    map.insert("overflow-y".to_string(), oy_str.to_string());
    map.insert(
        "overflow".to_string(),
        if ox_str == oy_str {
            ox_str.to_string()
        } else {
            format!("{ox_str} {oy_str}")
        },
    );

    for (k, v) in style.custom_properties.iter() {
        let val_str = match v {
            mango_css::Value::String(s)
            | mango_css::Value::Keyword(s)
            | mango_css::Value::Url(s) => s.clone(),
            mango_css::Value::Number(n) => format!("{}", n),
            mango_css::Value::Percentage(p) => format!("{}%", p),
            mango_css::Value::Color(c) => {
                if c.a == 255 {
                    format!("rgb({}, {}, {})", c.r, c.g, c.b)
                } else {
                    format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, c.a as f32 / 255.0)
                }
            }
            mango_css::Value::Length(l) => match l {
                mango_css::values::Length::Px(px) => format!("{}px", px),
                mango_css::values::Length::Percent(pct) => format!("{}%", pct),
                mango_css::values::Length::Auto => "auto".to_string(),
                _ => format!("{:?}", l),
            },
            _ => format!("{:?}", v),
        };
        map.insert(k.clone(), val_str);
    }
}
