use deno_core::{OpState, op2};
use mango_html::dom::{NodeData, NodeId};

use crate::dom_bindings::{
    LayoutBounds, SharedDocument, collect_elements_by_kind, collect_matching_elements,
    find_element_by_id_recursive, find_first_matching_element, query_selector_simple,
};

// ── DOM Query Ops ────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_get_element_by_id(state: &mut OpState, #[string] id: String) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    find_element_by_id_recursive(&d, d.root(), &id)
        .map(|id| id.raw())
        .unwrap_or(0)
}

#[op2(fast)]
pub fn op_mango_query_selector(
    state: &mut OpState,
    root_id: u32,
    #[string] selector: String,
) -> u32 {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return 0;
    };
    let d = doc.borrow();
    let root = if root_id == 0 {
        d.root()
    } else {
        NodeId::from_raw(root_id)
    };
    if let Some(selectors) = mango_css::parse_selectors(&selector)
        && let Some(found) = find_first_matching_element(&d, root, &selectors)
    {
        return found.raw();
    }
    query_selector_simple(&d, root, &selector)
        .map(|id| id.raw())
        .unwrap_or(0)
}

#[op2]
#[serde]
pub fn op_mango_query_selector_all(
    state: &mut OpState,
    root_id: u32,
    #[string] selector: String,
) -> Vec<u32> {
    let Some(doc) = state.try_borrow::<SharedDocument>() else {
        return Vec::new();
    };
    let d = doc.borrow();
    let root = if root_id == 0 {
        d.root()
    } else {
        NodeId::from_raw(root_id)
    };
    let mut out = Vec::new();
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        collect_matching_elements(&d, root, &selectors, &mut out);
    }
    if out.is_empty() {
        collect_elements_by_kind(&d, root, &selector, &mut out);
    }
    out.into_iter().map(|id| id.raw()).collect()
}

#[op2]
#[serde]
pub fn op_mango_get_document_elements(state: &mut OpState, #[string] tag: String) -> Vec<u32> {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let d = doc.borrow();
    let mut out = Vec::new();
    collect_elements_by_kind(&d, d.root(), &tag, &mut out);
    out.into_iter().map(|id| id.raw()).collect()
}

#[op2(fast)]
pub fn op_mango_element_matches(
    state: &mut OpState,
    node_id: u32,
    #[string] selector: String,
) -> bool {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return false,
    };
    let d = doc.borrow();
    let nid = NodeId::from_raw(node_id);
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        return selectors.matches(nid, &d);
    }
    false
}

#[op2(fast)]
pub fn op_mango_element_closest(
    state: &mut OpState,
    node_id: u32,
    #[string] selector: String,
) -> u32 {
    let doc = match state.try_borrow::<SharedDocument>() {
        Some(d) => d,
        None => return 0,
    };
    let d = doc.borrow();
    if let Some(selectors) = mango_css::parse_selectors(&selector) {
        let mut curr = Some(NodeId::from_raw(node_id));
        while let Some(id) = curr {
            if matches!(d.get(id).map(|n| &n.data), Some(NodeData::Element(_)))
                && selectors.matches(id, &d)
            {
                return id.raw();
            }
            curr = d.get(id).and_then(|n| n.parent);
        }
    }
    0
}

// ── Layout Measurement Ops ───────────────────────────────────────────────────

#[op2]
#[serde]
pub fn op_mango_get_element_rect(state: &mut OpState, node_id: u32) -> Vec<f32> {
    if let Some(bounds) = state.try_borrow::<LayoutBounds>()
        && let Some(&rect) = bounds.borrow().get(&node_id)
    {
        return rect.to_vec();
    }
    vec![0.0, 0.0, 0.0, 0.0]
}

#[op2(fast)]
pub fn op_mango_element_from_point(state: &mut OpState, x: f32, y: f32) -> u32 {
    if let Some(bounds) = state.try_borrow::<LayoutBounds>() {
        let b = bounds.borrow();
        let mut best: Option<(u32, f32)> = None;
        for (&node_id, &[bx, by, bw, bh]) in b.iter() {
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let area = bw * bh;
                if best.is_none_or(|(_, best_area)| area < best_area) {
                    best = Some((node_id, area));
                }
            }
        }
        if let Some((id, _)) = best {
            return id;
        }
    }
    0
}

#[op2]
#[serde]
pub fn op_mango_elements_from_point(state: &mut OpState, x: f32, y: f32) -> Vec<u32> {
    let mut matches: Vec<(u32, f32)> = Vec::new();
    if let Some(bounds) = state.try_borrow::<LayoutBounds>() {
        let b = bounds.borrow();
        for (&node_id, &[bx, by, bw, bh]) in b.iter() {
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                let area = bw * bh;
                matches.push((node_id, area));
            }
        }
    }
    matches.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    matches.into_iter().map(|(id, _)| id).collect()
}

#[op2(fast)]
pub fn op_mango_set_focus_state(_state: &mut OpState, _node_id: u32, _focused: bool) {}

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
    let nid = NodeId::from_raw(node_id);
    if d.get(nid).is_none() {
        return HashMap::new();
    }

    let mut sheets = Vec::new();
    let mut stack = vec![d.root()];
    while let Some(curr) = stack.pop() {
        if let Some(node) = d.get(curr) {
            if let NodeData::Element(el) = &node.data {
                if el.tag_name.eq_ignore_ascii_case("style") {
                    let css_text = d.text_content(node.id);
                    if !css_text.trim().is_empty() {
                        sheets.push(mango_css::parser::parse_stylesheet(&css_text));
                    }
                }
            }
            for child in d.children(curr) {
                stack.push(child.id);
            }
        }
    }

    let sheet_refs: Vec<&mango_css::parser::Stylesheet> = sheets.iter().collect();
    let style = mango_css::compute_style(nid, &d, &sheet_refs, None);

    let mut map = HashMap::new();

    // Color
    let color_str = if style.color.a == 255 {
        format!(
            "rgb({}, {}, {})",
            style.color.r, style.color.g, style.color.b
        )
    } else {
        format!(
            "rgba({}, {}, {}, {})",
            style.color.r,
            style.color.g,
            style.color.b,
            style.color.a as f32 / 255.0
        )
    };
    map.insert("color".to_string(), color_str);

    // Background color
    let bg_color_str = if style.background_color.a == 255 {
        format!(
            "rgb({}, {}, {})",
            style.background_color.r, style.background_color.g, style.background_color.b
        )
    } else if style.background_color.a == 0 {
        "rgba(0, 0, 0, 0)".to_string()
    } else {
        format!(
            "rgba({}, {}, {}, {})",
            style.background_color.r,
            style.background_color.g,
            style.background_color.b,
            style.background_color.a as f32 / 255.0
        )
    };
    map.insert("background-color".to_string(), bg_color_str);

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

    // Position
    let pos_str = match style.position {
        mango_css::values::Position::Static => "static",
        mango_css::values::Position::Relative => "relative",
        mango_css::values::Position::Absolute => "absolute",
        mango_css::values::Position::Fixed => "fixed",
        mango_css::values::Position::Sticky => "sticky",
    };
    map.insert("position".to_string(), pos_str.to_string());

    // Width & Height
    let w_str = match style.width {
        mango_css::values::Length::Px(px) => format!("{}px", px),
        mango_css::values::Length::Percent(pct) => format!("{}%", pct),
        mango_css::values::Length::Auto => "auto".to_string(),
        _ => "auto".to_string(),
    };
    map.insert("width".to_string(), w_str);

    let h_str = match style.height {
        mango_css::values::Length::Px(px) => format!("{}px", px),
        mango_css::values::Length::Percent(pct) => format!("{}%", pct),
        mango_css::values::Length::Auto => "auto".to_string(),
        _ => "auto".to_string(),
    };
    map.insert("height".to_string(), h_str);

    // Box sizing
    let bs_str = match style.box_sizing {
        mango_css::values::BoxSizing::BorderBox => "border-box",
        mango_css::values::BoxSizing::ContentBox => "content-box",
    };
    map.insert("box-sizing".to_string(), bs_str.to_string());

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
