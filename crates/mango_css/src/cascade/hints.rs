//! HTML presentational hints mapping for legacy attributes (bgcolor, align, border, cellpadding, etc.).

use mango_core::Color;
use mango_html::dom::{Document, ElementData, NodeData, NodeId};

use crate::properties::Declaration;
use crate::specificity::Specificity;
use crate::values::{Length, Value};

use super::types::{MatchedDeclaration, Origin};

pub(crate) fn parse_presentational_color(raw: &str) -> Option<Color> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    Value::parse_color(s).or_else(|| Value::parse_color(&format!("#{s}")))
}

pub(crate) fn parse_presentational_dimension(raw: &str) -> Option<Value> {
    let s = raw.trim();
    if let Some(rest) = s.strip_suffix('%')
        && let Ok(pct) = rest.trim().parse::<f32>()
    {
        return Some(Value::Length(Length::Percent(pct)));
    }
    let num_str = s.strip_suffix("px").unwrap_or(s).trim();
    if let Ok(px) = num_str.parse::<f32>() {
        return Some(Value::Length(Length::Px(px)));
    }
    None
}

fn push_hint(
    matched_decls: &mut Vec<MatchedDeclaration>,
    source_order: &mut usize,
    name: impl Into<String>,
    val: Value,
) {
    *source_order += 1;
    matched_decls.push(MatchedDeclaration {
        declaration: Declaration::new(name, val, false),
        specificity: Specificity(0, 0, 0),
        origin: Origin::PresentationalHint,
        source_order: *source_order,
        layer_order: None,
    });
}

pub(crate) fn collect_presentational_hints(
    node_id: NodeId,
    doc: &Document,
    elem: &ElementData,
    source_order: &mut usize,
    matched_decls: &mut Vec<MatchedDeclaration>,
) {
    let tag = elem.tag_name.to_ascii_lowercase();

    // bgcolor -> background-color
    if let Some(bg) = elem.get_attribute("bgcolor")
        && let Some(c) = parse_presentational_color(bg)
    {
        push_hint(
            matched_decls,
            source_order,
            "background-color",
            Value::Color(c),
        );
    }

    // background -> background-image
    if let Some(bg_img) = elem.get_attribute("background") {
        let trimmed = bg_img.trim();
        if !trimmed.is_empty() {
            push_hint(
                matched_decls,
                source_order,
                "background-image",
                Value::Url(trimmed.to_string()),
            );
        }
    }

    // width
    if let Some(w) = elem.get_attribute("width")
        && let Some(dim) = parse_presentational_dimension(w)
    {
        push_hint(matched_decls, source_order, "width", dim);
    }

    // height
    if let Some(h) = elem.get_attribute("height")
        && let Some(dim) = parse_presentational_dimension(h)
    {
        push_hint(matched_decls, source_order, "height", dim);
    }

    // align
    if let Some(align) = elem.get_attribute("align") {
        let a = align.trim().to_ascii_lowercase();
        if tag == "table" || tag == "hr" {
            match a.as_str() {
                "center" => {
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-left",
                        Value::Keyword("auto".to_string()),
                    );
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-right",
                        Value::Keyword("auto".to_string()),
                    );
                }
                "right" => {
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-left",
                        Value::Keyword("auto".to_string()),
                    );
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-right",
                        Value::Length(Length::Px(0.0)),
                    );
                }
                "left" => {
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-left",
                        Value::Length(Length::Px(0.0)),
                    );
                    push_hint(
                        matched_decls,
                        source_order,
                        "margin-right",
                        Value::Keyword("auto".to_string()),
                    );
                }
                _ => {}
            }
        } else {
            let ta = match a.as_str() {
                "center" => Some(crate::values::TextAlign::Center),
                "right" => Some(crate::values::TextAlign::Right),
                "left" => Some(crate::values::TextAlign::Left),
                "justify" => Some(crate::values::TextAlign::Justify),
                _ => None,
            };
            if let Some(ta_val) = ta {
                push_hint(
                    matched_decls,
                    source_order,
                    "text-align",
                    Value::TextAlign(ta_val),
                );
            }
        }
    }

    // valign
    if let Some(valign) = elem.get_attribute("valign") {
        let va = match valign.trim().to_ascii_lowercase().as_str() {
            "top" => Some(crate::values::VerticalAlign::Top),
            "middle" => Some(crate::values::VerticalAlign::Middle),
            "bottom" => Some(crate::values::VerticalAlign::Bottom),
            "baseline" => Some(crate::values::VerticalAlign::Baseline),
            _ => None,
        };
        if let Some(va_val) = va {
            push_hint(
                matched_decls,
                source_order,
                "vertical-align",
                Value::VerticalAlign(va_val),
            );
        }
    }

    // border (on table or img)
    if let Some(b) = elem.get_attribute("border")
        && let Ok(b_val) = b.trim().parse::<f32>()
    {
        if b_val <= 0.0 {
            for side in &[
                "border-top-style",
                "border-right-style",
                "border-bottom-style",
                "border-left-style",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::BorderStyle(crate::values::BorderStyle::None),
                );
            }
            for side in &[
                "border-top-width",
                "border-right-width",
                "border-bottom-width",
                "border-left-width",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::Length(Length::Px(0.0)),
                );
            }
        } else {
            for side in &[
                "border-top-style",
                "border-right-style",
                "border-bottom-style",
                "border-left-style",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::BorderStyle(crate::values::BorderStyle::Solid),
                );
            }
            for side in &[
                "border-top-width",
                "border-right-width",
                "border-bottom-width",
                "border-left-width",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::Length(Length::Px(b_val)),
                );
            }
        }
    }

    // frameborder (on iframe: "0" or "no" removes border)
    if tag == "iframe"
        && let Some(fb) = elem.get_attribute("frameborder")
    {
        let fb_clean = fb.trim().to_ascii_lowercase();
        if fb_clean == "0" || fb_clean == "no" {
            for side in &[
                "border-top-style",
                "border-right-style",
                "border-bottom-style",
                "border-left-style",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::BorderStyle(crate::values::BorderStyle::None),
                );
            }
            for side in &[
                "border-top-width",
                "border-right-width",
                "border-bottom-width",
                "border-left-width",
            ] {
                push_hint(
                    matched_decls,
                    source_order,
                    *side,
                    Value::Length(Length::Px(0.0)),
                );
            }
        }
    }

    // For table, cellspacing attribute -> border-spacing
    if tag == "table"
        && let Some(cs_str) = elem.get_attribute("cellspacing")
        && let Ok(cs) = cs_str.trim().parse::<f32>()
    {
        push_hint(
            matched_decls,
            source_order,
            "border-spacing",
            Value::Length(Length::Px(cs)),
        );
    }

    // For td or th, inherit cellpadding from ancestor table, and valign/align from parent tr
    if tag == "td" || tag == "th" {
        // Inherit valign and align from parent tr if not explicitly declared on cell
        if let Some(parent_id) = doc.get(node_id).and_then(|n| n.parent)
            && let Some(p_node) = doc.get(parent_id)
            && let NodeData::Element(p_elem) = &p_node.data
            && p_elem.tag_name.eq_ignore_ascii_case("tr")
        {
            if elem.get_attribute("valign").is_none()
                && let Some(valign) = p_elem.get_attribute("valign")
            {
                let va = match valign.trim().to_ascii_lowercase().as_str() {
                    "top" => Some(crate::values::VerticalAlign::Top),
                    "middle" => Some(crate::values::VerticalAlign::Middle),
                    "bottom" => Some(crate::values::VerticalAlign::Bottom),
                    "baseline" => Some(crate::values::VerticalAlign::Baseline),
                    _ => None,
                };
                if let Some(va_val) = va {
                    push_hint(
                        matched_decls,
                        source_order,
                        "vertical-align",
                        Value::VerticalAlign(va_val),
                    );
                }
            }
            if elem.get_attribute("align").is_none()
                && let Some(align) = p_elem.get_attribute("align")
            {
                let ta = match align.trim().to_ascii_lowercase().as_str() {
                    "center" => Some(crate::values::TextAlign::Center),
                    "right" => Some(crate::values::TextAlign::Right),
                    "left" => Some(crate::values::TextAlign::Left),
                    "justify" => Some(crate::values::TextAlign::Justify),
                    _ => None,
                };
                if let Some(ta_val) = ta {
                    push_hint(
                        matched_decls,
                        source_order,
                        "text-align",
                        Value::TextAlign(ta_val),
                    );
                }
            }
        }

        // Inherit cellpadding from ancestor table
        let mut cur = node_id;
        while let Some(parent_id) = doc.get(cur).and_then(|n| n.parent) {
            if let Some(p_node) = doc.get(parent_id)
                && let NodeData::Element(p_elem) = &p_node.data
                && p_elem.tag_name.eq_ignore_ascii_case("table")
            {
                if let Some(cp_str) = p_elem.get_attribute("cellpadding")
                    && let Ok(cp) = cp_str.trim().parse::<f32>()
                {
                    for side in &[
                        "padding-top",
                        "padding-right",
                        "padding-bottom",
                        "padding-left",
                    ] {
                        push_hint(
                            matched_decls,
                            source_order,
                            *side,
                            Value::Length(Length::Px(cp)),
                        );
                    }
                }
                break;
            }
            cur = parent_id;
        }
    }
}
