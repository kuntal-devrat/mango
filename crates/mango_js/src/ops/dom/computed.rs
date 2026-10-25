//! Computed style extraction and formatting op.

use deno_core::{OpState, op2};
use mango_html::dom::{NodeData, NodeId, node_id_from_js};

use super::computed_format::*;
use crate::dom_bindings::{LayoutBounds, SharedDocument};

#[op2]
#[serde]
pub fn op_mango_get_computed_style(
    state: &mut OpState,
    node_id: u32,
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;

    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d.clone(),
        None => return HashMap::new(),
    };
    let d = doc.borrow();
    let nid = node_id_from_js(node_id);
    if d.get(nid).is_none() {
        return HashMap::new();
    }

    let mut sheets = Vec::new();
    fn collect_styles_recursive(
        d: &mango_html::dom::Document,
        curr: NodeId,
        out: &mut Vec<mango_css::parser::Stylesheet>,
    ) {
        for child in d.children(curr) {
            if let NodeData::Element(el) = &child.data {
                if el.tag_name.eq_ignore_ascii_case("noscript") {
                    continue;
                }
                if el.tag_name.eq_ignore_ascii_case("style") {
                    let css_text = d.text_content(child.id);
                    if !css_text.trim().is_empty() {
                        out.push(mango_css::parser::parse_stylesheet(&css_text));
                    }
                }
            }
            collect_styles_recursive(d, child.id, out);
        }
    }
    collect_styles_recursive(&d, d.root(), &mut sheets);

    let sheet_refs: Vec<&mango_css::parser::Stylesheet> = sheets.iter().collect();
    let mut author_order = 1000;
    let author_index = mango_css::cascade::RuleIndex::from_stylesheets(
        &sheet_refs,
        mango_css::cascade::Origin::Author,
        &mut author_order,
    );

    let node_elem = d.get(nid).and_then(|n| match &n.data {
        NodeData::Element(el) => Some(el),
        _ => None,
    });

    let shadow_root_id = node_elem
        .and_then(|el| el.shadow_root)
        .or_else(|| {
            d.get(nid)
                .and_then(|n| n.parent)
                .and_then(|pid| d.get(pid))
                .and_then(|pn| match &pn.data {
                    NodeData::Element(pe) => pe.shadow_root,
                    _ => None,
                })
        })
        .or_else(|| d.get_enclosing_shadow_root(nid));

    let mut shadow_sheets = Vec::new();
    if let Some(sr_id) = shadow_root_id {
        collect_styles_recursive(&d, sr_id, &mut shadow_sheets);
    }
    let shadow_refs: Vec<&mango_css::parser::Stylesheet> = shadow_sheets.iter().collect();
    let mut shadow_order = 50000;
    let shadow_index = if !shadow_refs.is_empty() {
        Some(mango_css::cascade::RuleIndex::from_stylesheets(
            &shadow_refs,
            mango_css::cascade::Origin::Author,
            &mut shadow_order,
        ))
    } else {
        None
    };

    let parent_style = d
        .get(nid)
        .and_then(|n| n.parent)
        .map(|pid| mango_css::compute_style_with_index(pid, &d, &author_index, None));

    let style = mango_css::compute_style_with_shadow_index(
        nid,
        &d,
        &author_index,
        shadow_index.as_ref(),
        parent_style.as_ref(),
    );

    // Determine containing block width for resolving percentage margins/paddings
    let mut cb_w: Option<f32> = None;
    let is_abspos = matches!(
        style.position,
        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
    );

    let mut curr_ancestor = d.get(nid).and_then(|n| n.parent);
    while let Some(anc_id) = curr_ancestor {
        let anc_style = mango_css::compute_style_with_index(anc_id, &d, &author_index, None);
        let is_cb = if is_abspos {
            anc_style.position != mango_css::values::Position::Static
                || !anc_style.transform.is_identity()
                || anc_style.will_change
                || anc_style
                    .will_change_properties
                    .iter()
                    .any(|p| p == "transform" || p == "perspective")
        } else {
            true
        };

        if is_cb {
            if let Some(bounds) = state.try_borrow::<LayoutBounds>() {
                if let Some(rect) = bounds.borrow().get(&anc_id.raw()) {
                    if rect[2] > 0.0 {
                        cb_w = Some(rect[2]);
                        break;
                    }
                }
            }
            if let mango_css::values::Length::Px(px) = anc_style.width {
                if px > 0.0 {
                    cb_w = Some(px);
                    break;
                }
            }
            if !is_abspos {
                break;
            }
        }
        curr_ancestor = d.get(anc_id).and_then(|n| n.parent);
    }

    if cb_w.is_none() {
        let mut curr = d.get(nid).and_then(|n| n.parent);
        while let Some(anc_id) = curr {
            let anc_style = mango_css::compute_style_with_index(anc_id, &d, &author_index, None);
            if let mango_css::values::Length::Px(px) = anc_style.width {
                if px > 0.0 {
                    cb_w = Some(px);
                    break;
                }
            }
            curr = d.get(anc_id).and_then(|n| n.parent);
        }
    }

    let mut map = HashMap::new();

    // Color & Background Color
    map.insert("color".to_string(), format_css_color(&style.color));
    map.insert(
        "background-color".to_string(),
        format_css_color(&style.background_color),
    );

    // Display
    let display_str = match style.display {
        mango_css::values::Display::None => "none",
        mango_css::values::Display::Block => "block",
        mango_css::values::Display::Inline => "inline",
        mango_css::values::Display::InlineBlock => "inline-block",
        mango_css::values::Display::Flex => "flex",
        mango_css::values::Display::InlineFlex => "inline-flex",
        mango_css::values::Display::Grid => "grid",
        mango_css::values::Display::InlineGrid => "inline-grid",
        mango_css::values::Display::Table => "table",
        mango_css::values::Display::TableRow => "table-row",
        mango_css::values::Display::TableCell => "table-cell",
        mango_css::values::Display::ListItem => "list-item",
        _ => "block",
    };
    map.insert("display".to_string(), display_str.to_string());

    // Visibility
    let vis_str = match style.visibility {
        mango_css::values::Visibility::Visible => "visible",
        mango_css::values::Visibility::Hidden => "hidden",
        mango_css::values::Visibility::Collapse => "collapse",
    };
    map.insert("visibility".to_string(), vis_str.to_string());

    // Opacity
    map.insert("opacity".to_string(), style.opacity.to_string());

    // Font size & family & weight & line height
    map.insert("font-size".to_string(), format!("{}px", style.font_size));
    map.insert("font-family".to_string(), style.font_family.clone());
    map.insert(
        "font-weight".to_string(),
        format!("{}", style.font_weight.to_number()),
    );
    if let Some(lh) = style.line_height {
        map.insert("line-height".to_string(), format!("{}px", lh));
    } else {
        map.insert("line-height".to_string(), "normal".to_string());
    }

    // Text decoration & typography
    let td_line = style.text_decoration.to_css_string();
    let td_style = style.text_decoration_style.as_str().to_string();
    let td_color = format_css_color(&style.resolved_text_decoration_color());
    let td_thickness = match &style.text_decoration_thickness {
        mango_css::values::TextDecorationThickness::Auto => "auto".to_string(),
        mango_css::values::TextDecorationThickness::FromFont => "from-font".to_string(),
        mango_css::values::TextDecorationThickness::Length(l) => {
            format_css_length(l, style.font_size, style.root_font_size, None)
        }
    };
    map.insert("text-decoration-line".to_string(), td_line.clone());
    map.insert("text-decoration-style".to_string(), td_style.clone());
    map.insert("text-decoration-color".to_string(), td_color.clone());
    map.insert(
        "text-decoration-thickness".to_string(),
        td_thickness.clone(),
    );

    let td_shorthand = if td_line == "none"
        && td_thickness == "auto"
        && td_style == "solid"
        && style.text_decoration_color.is_none()
    {
        "none".to_string()
    } else {
        let mut parts = Vec::new();
        if td_line != "none" {
            parts.push(td_line);
        }
        if td_thickness != "auto" {
            parts.push(td_thickness);
        }
        if td_style != "solid" {
            parts.push(td_style);
        }
        if style.text_decoration_color.is_some() {
            parts.push(td_color);
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(" ")
        }
    };
    map.insert("text-decoration".to_string(), td_shorthand);

    // Position & Insets
    let pos_str = match style.position {
        mango_css::values::Position::Static => "static",
        mango_css::values::Position::Relative => "relative",
        mango_css::values::Position::Absolute => "absolute",
        mango_css::values::Position::Fixed => "fixed",
        mango_css::values::Position::Sticky => "sticky",
    };
    map.insert("position".to_string(), pos_str.to_string());
    let top_str = format_css_length(&style.top, style.font_size, style.root_font_size, cb_w);
    let right_str = format_css_length(&style.right, style.font_size, style.root_font_size, cb_w);
    let bottom_str = format_css_length(&style.bottom, style.font_size, style.root_font_size, cb_w);
    let left_str = format_css_length(&style.left, style.font_size, style.root_font_size, cb_w);
    map.insert("top".to_string(), top_str.clone());
    map.insert("right".to_string(), right_str.clone());
    map.insert("bottom".to_string(), bottom_str.clone());
    map.insert("left".to_string(), left_str.clone());

    map.insert("inset-block-start".to_string(), top_str.clone());
    map.insert("inset-block-end".to_string(), bottom_str.clone());
    map.insert("inset-inline-start".to_string(), left_str.clone());
    map.insert("inset-inline-end".to_string(), right_str.clone());
    let inset_shorthand = serialize_4_box(&top_str, &right_str, &bottom_str, &left_str);
    map.insert("inset".to_string(), inset_shorthand);
    map.insert("overlay".to_string(), "none".to_string());

    // Float & Clear & Z-Index
    let float_str = match style.float {
        mango_css::values::Float::None => "none",
        mango_css::values::Float::Left => "left",
        mango_css::values::Float::Right => "right",
        mango_css::values::Float::InlineStart => "inline-start",
        mango_css::values::Float::InlineEnd => "inline-end",
    };
    map.insert("float".to_string(), float_str.to_string());

    let clear_str = match style.clear {
        mango_css::values::Clear::None => "none",
        mango_css::values::Clear::Left => "left",
        mango_css::values::Clear::Right => "right",
        mango_css::values::Clear::Both => "both",
        mango_css::values::Clear::InlineStart => "inline-start",
        mango_css::values::Clear::InlineEnd => "inline-end",
    };
    map.insert("clear".to_string(), clear_str.to_string());

    let z_str = match style.z_index {
        Some(z) => z.to_string(),
        None => "auto".to_string(),
    };
    map.insert("z-index".to_string(), z_str);

    // Dimensions.
    //
    // CSSOM § 2.2 requires `getComputedStyle()` to return the *used* value for
    // `width` / `height` (a resolved pixel length), not the specified value.
    // When the layout engine has produced bounds for this node we use them;
    // otherwise we fall back to the computed value.
    let used_size = state
        .try_borrow::<LayoutBounds>()
        .and_then(|b| b.borrow().get(&nid.raw()).copied());
    let (used_w, used_h) = match used_size {
        Some([_, _, w, h]) => (Some(w), Some(h)),
        _ => (None, None),
    };
    map.insert(
        "width".to_string(),
        match used_w {
            Some(w) => format!("{w:.0}px"),
            None => format_css_length(&style.width, style.font_size, style.root_font_size, cb_w),
        },
    );
    map.insert(
        "height".to_string(),
        match used_h {
            Some(h) => format!("{h:.0}px"),
            None => format_css_length(&style.height, style.font_size, style.root_font_size, None),
        },
    );
    map.insert(
        "min-width".to_string(),
        format_css_length(
            &style.min_width,
            style.font_size,
            style.root_font_size,
            cb_w,
        ),
    );
    let max_w = match style.max_width {
        mango_css::values::Length::Auto => "none".to_string(),
        ref l => format_css_length(l, style.font_size, style.root_font_size, cb_w),
    };
    map.insert("max-width".to_string(), max_w);
    map.insert(
        "min-height".to_string(),
        format_css_length(
            &style.min_height,
            style.font_size,
            style.root_font_size,
            None,
        ),
    );
    let max_h = match style.max_height {
        mango_css::values::Length::Auto => "none".to_string(),
        ref l => format_css_length(l, style.font_size, style.root_font_size, None),
    };
    map.insert("max-height".to_string(), max_h);

    // Box sizing
    let bs_str = match style.box_sizing {
        mango_css::values::BoxSizing::BorderBox => "border-box",
        mango_css::values::BoxSizing::ContentBox => "content-box",
    };
    map.insert("box-sizing".to_string(), bs_str.to_string());

    // Margins
    let mt = format_css_length(
        &style.margin_top,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let mr = format_css_length(
        &style.margin_right,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let mb = format_css_length(
        &style.margin_bottom,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let ml = format_css_length(
        &style.margin_left,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    map.insert("margin-top".to_string(), mt.clone());
    map.insert("margin-right".to_string(), mr.clone());
    map.insert("margin-bottom".to_string(), mb.clone());
    map.insert("margin-left".to_string(), ml.clone());
    let margin_shorthand = serialize_4_box(&mt, &mr, &mb, &ml);
    map.insert("margin".to_string(), margin_shorthand);
    map.insert("margin-trim".to_string(), style.margin_trim.clone());

    // Paddings
    let pt = format_padding_length(
        &style.padding_top,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let pr = format_padding_length(
        &style.padding_right,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let pb = format_padding_length(
        &style.padding_bottom,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    let pl = format_padding_length(
        &style.padding_left,
        style.font_size,
        style.root_font_size,
        cb_w,
    );
    map.insert("padding-top".to_string(), pt.clone());
    map.insert("padding-right".to_string(), pr.clone());
    map.insert("padding-bottom".to_string(), pb.clone());
    map.insert("padding-left".to_string(), pl.clone());
    let padding_shorthand = serialize_4_box(&pt, &pr, &pb, &pl);
    map.insert("padding".to_string(), padding_shorthand);

    // Borders
    map.insert(
        "border-top-width".to_string(),
        format!("{:.0}px", style.border_top_width),
    );
    map.insert(
        "border-right-width".to_string(),
        format!("{:.0}px", style.border_right_width),
    );
    map.insert(
        "border-bottom-width".to_string(),
        format!("{:.0}px", style.border_bottom_width),
    );
    map.insert(
        "border-left-width".to_string(),
        format!("{:.0}px", style.border_left_width),
    );
    map.insert(
        "border-top-color".to_string(),
        format_css_color(&style.border_top_color),
    );
    map.insert(
        "border-right-color".to_string(),
        format_css_color(&style.border_right_color),
    );
    map.insert(
        "border-bottom-color".to_string(),
        format_css_color(&style.border_bottom_color),
    );
    map.insert(
        "border-left-color".to_string(),
        format_css_color(&style.border_left_color),
    );
    map.insert(
        "border-top-style".to_string(),
        format_border_style(style.border_top_style).to_string(),
    );
    map.insert(
        "border-right-style".to_string(),
        format_border_style(style.border_right_style).to_string(),
    );
    map.insert(
        "border-bottom-style".to_string(),
        format_border_style(style.border_bottom_style).to_string(),
    );
    map.insert(
        "border-left-style".to_string(),
        format_border_style(style.border_left_style).to_string(),
    );
    if style.border_top_color == style.border_right_color
        && style.border_right_color == style.border_bottom_color
        && style.border_bottom_color == style.border_left_color
    {
        map.insert(
            "border-color".to_string(),
            format_css_color(&style.border_top_color),
        );
    }
    if style.border_top_width == style.border_right_width
        && style.border_right_width == style.border_bottom_width
        && style.border_bottom_width == style.border_left_width
    {
        map.insert(
            "border-width".to_string(),
            format!("{:.0}px", style.border_top_width),
        );
    }
    if style.border_top_style == style.border_right_style
        && style.border_right_style == style.border_bottom_style
        && style.border_bottom_style == style.border_left_style
    {
        map.insert(
            "border-style".to_string(),
            format_border_style(style.border_top_style).to_string(),
        );
    }

    // Flexbox
    let fd_str = match style.flex_direction {
        mango_css::values::FlexDirection::Row => "row",
        mango_css::values::FlexDirection::RowReverse => "row-reverse",
        mango_css::values::FlexDirection::Column => "column",
        mango_css::values::FlexDirection::ColumnReverse => "column-reverse",
    };
    map.insert("flex-direction".to_string(), fd_str.to_string());

    let fw_str = match style.flex_wrap {
        mango_css::values::FlexWrap::NoWrap => "nowrap",
        mango_css::values::FlexWrap::Wrap => "wrap",
        mango_css::values::FlexWrap::WrapReverse => "wrap-reverse",
    };
    map.insert("flex-wrap".to_string(), fw_str.to_string());

    let fg_str = if style.flex_grow.fract() == 0.0 {
        format!("{:.0}", style.flex_grow)
    } else {
        format!("{}", style.flex_grow)
    };
    map.insert("flex-grow".to_string(), fg_str.clone());

    let fs_str = if style.flex_shrink.fract() == 0.0 {
        format!("{:.0}", style.flex_shrink)
    } else {
        format!("{}", style.flex_shrink)
    };
    map.insert("flex-shrink".to_string(), fs_str.clone());

    let flex_basis_str = match &style.flex_basis {
        mango_css::values::Length::Px(px) => {
            let clamped = px.max(0.0);
            let rounded = (clamped * 1000.0).round() / 1000.0;
            if (rounded - rounded.round()).abs() < 1e-4 {
                format!("{:.0}px", rounded.round())
            } else {
                format!("{rounded}px")
            }
        }
        mango_css::values::Length::Percent(pct) => {
            if pct.fract() == 0.0 {
                format!("{:.0}%", pct)
            } else {
                format!("{pct}%")
            }
        }
        mango_css::values::Length::Calc(calc) => {
            if calc.has_percent {
                format_css_length(
                    &style.flex_basis,
                    style.font_size,
                    style.root_font_size,
                    None,
                )
            } else {
                let px = (calc.px
                    + calc.em * style.font_size
                    + calc.ex * 0.5 * style.font_size
                    + calc.rem * style.root_font_size)
                    .max(0.0);
                let rounded = (px * 1000.0).round() / 1000.0;
                if (rounded - rounded.round()).abs() < 1e-4 {
                    format!("{:.0}px", rounded.round())
                } else {
                    format!("{rounded}px")
                }
            }
        }
        mango_css::values::Length::Math(_) => {
            let px = style
                .flex_basis
                .to_px(style.font_size, style.root_font_size, cb_w.unwrap_or(0.0))
                .max(0.0);
            let rounded = (px * 1000.0).round() / 1000.0;
            if (rounded - rounded.round()).abs() < 1e-4 {
                format!("{:.0}px", rounded.round())
            } else {
                format!("{rounded}px")
            }
        }
        _ => format_css_length(
            &style.flex_basis,
            style.font_size,
            style.root_font_size,
            None,
        ),
    };
    map.insert("flex-basis".to_string(), flex_basis_str.clone());

    // Flex and Flex-flow shorthands
    let flex_shorthand = format!("{fg_str} {fs_str} {flex_basis_str}");
    map.insert("flex".to_string(), flex_shorthand);
    let flex_flow_shorthand = format!("{fd_str} {fw_str}");
    map.insert("flex-flow".to_string(), flex_flow_shorthand);

    map.insert("flex-line-count".to_string(), "1".to_string());
    map.insert("order".to_string(), style.order.to_string());

    let jc_str = match style.justify_content {
        mango_css::values::JustifyContent::Normal => "normal",
        mango_css::values::JustifyContent::FlexStart => "flex-start",
        mango_css::values::JustifyContent::FlexEnd => "flex-end",
        mango_css::values::JustifyContent::Center => "center",
        mango_css::values::JustifyContent::SpaceBetween => "space-between",
        mango_css::values::JustifyContent::SpaceAround => "space-around",
        mango_css::values::JustifyContent::SpaceEvenly => "space-evenly",
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

    // Overflow
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

    // Custom properties
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

    map
}
