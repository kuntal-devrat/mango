use std::sync::Arc;

use mango_core::Rect;
use mango_css::computed::ComputedStyle;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;

use super::atom::{InlineAtom, InlineAtomKind};
use super::metrics::measure_text_width_with_style;
use super::shaping::{font_family_for, font_weight_for};

pub(crate) fn collect_ruby_atoms(
    ruby_box: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    current_link: Option<&str>,
    current_frame: Option<&str>,
    _container_width: f32,
    _container_height: f32,
    owner_inline_id: Option<mango_html::dom::NodeId>,
) {
    let ruby_style = ruby_box
        .style
        .clone()
        .unwrap_or_else(|| Arc::new(ComputedStyle::default()));
    let font_size = ruby_style.font_size;

    let mut bases: Vec<(String, Arc<ComputedStyle>)> = Vec::new();
    let mut rts: Vec<(String, Arc<ComputedStyle>)> = Vec::new();

    for child in &ruby_box.children {
        // Skip rp or display: none
        if child.tag_name.as_deref() == Some("rp")
            || child
                .style
                .as_ref()
                .map(|s| s.display == mango_css::values::Display::None)
                .unwrap_or(false)
        {
            continue;
        }

        let tag = child.tag_name.as_deref().unwrap_or("");
        if tag == "rt" {
            let mut t = String::new();
            extract_text_content(child, &mut t);
            let s = if let Some(child_style) = &child.style {
                if child_style.font_size >= font_size {
                    let mut s = (**child_style).clone();
                    s.font_size = (font_size * 0.5).max(8.0);
                    Arc::new(s)
                } else {
                    child_style.clone()
                }
            } else {
                let mut s = (*ruby_style).clone();
                s.font_size = (font_size * 0.5).max(8.0);
                Arc::new(s)
            };
            rts.push((t, s));
        } else if tag == "rb" {
            let mut t = String::new();
            extract_text_content(child, &mut t);
            let s = child.style.clone().unwrap_or_else(|| ruby_style.clone());
            bases.push((t, s));
        } else {
            // TextNode or other inline container
            let mut t = String::new();
            extract_text_content(child, &mut t);
            if !t.is_empty() {
                let s = child.style.clone().unwrap_or_else(|| ruby_style.clone());
                bases.push((t, s));
            }
        }
    }

    if bases.is_empty() && rts.is_empty() {
        return;
    }

    // Pair bases and annotations
    let pairs: Vec<(String, Arc<ComputedStyle>, String, Arc<ComputedStyle>)> =
        if bases.len() == rts.len() {
            bases
                .into_iter()
                .zip(rts)
                .map(|((b_txt, b_s), (rt_txt, rt_s))| (b_txt, b_s, rt_txt, rt_s))
                .collect()
        } else if rts.len() == 1 {
            let combined_base: String = bases.iter().map(|(t, _)| t.as_str()).collect();
            let b_style = bases
                .first()
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| ruby_style.clone());
            let (rt_txt, rt_s) = rts.remove(0);
            vec![(combined_base, b_style, rt_txt, rt_s)]
        } else {
            let max_len = bases.len().max(rts.len());
            let mut result = Vec::new();
            for i in 0..max_len {
                let (b_txt, b_s) = bases
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| (String::new(), ruby_style.clone()));
                let (rt_txt, rt_s) = rts
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| (String::new(), ruby_style.clone()));
                result.push((b_txt, b_s, rt_txt, rt_s));
            }
            result
        };

    for (b_txt, b_style, rt_txt, rt_style) in pairs {
        let b_w = measure_text_width_with_style(
            &b_txt,
            b_style.font_size,
            font_weight_for(&b_style),
            font_family_for(&b_style),
        );
        let (b_ascent, b_descent, _) = mango_render::font::font_manager().font_metrics(
            font_family_for(&b_style),
            font_weight_for(&b_style),
            b_style.font_size,
        );
        let b_h = (b_ascent + b_descent).max(b_style.font_size);

        let rt_w = measure_text_width_with_style(
            &rt_txt,
            rt_style.font_size,
            font_weight_for(&rt_style),
            font_family_for(&rt_style),
        );
        let (rt_ascent, rt_descent, _) = mango_render::font::font_manager().font_metrics(
            font_family_for(&rt_style),
            font_weight_for(&rt_style),
            rt_style.font_size,
        );
        let rt_h = (rt_ascent + rt_descent).max(rt_style.font_size);

        let pair_w = b_w.max(rt_w);
        let pair_h = b_h + rt_h;

        let b_x = (pair_w - b_w) / 2.0;
        let rt_x = (pair_w - rt_w) / 2.0;

        let mut ruby_pair_box = LayoutBox::new(BoxType::InlineBlock, Some(b_style.clone()));
        ruby_pair_box.tag_name = Some("ruby".into());
        ruby_pair_box.dimensions.content = Rect::new(0.0, 0.0, pair_w, pair_h);

        // Child 0: RT text box
        let mut rt_box = LayoutBox::new(BoxType::TextNode(rt_txt), Some(rt_style));
        rt_box.tag_name = Some("rt".into());
        rt_box.dimensions.content = Rect::new(rt_x, 0.0, rt_w, rt_h);
        ruby_pair_box.children.push(rt_box);

        // Child 1: Base text box
        let mut base_box = LayoutBox::new(BoxType::TextNode(b_txt), Some(b_style.clone()));
        base_box.tag_name = Some("rb".into());
        base_box.dimensions.content = Rect::new(b_x, rt_h, b_w, b_h);
        ruby_pair_box.children.push(base_box);

        out.push(InlineAtom {
            kind: InlineAtomKind::AtomicBox(Box::new(ruby_pair_box)),
            style: b_style,
            width: pair_w,
            height: pair_h,
            link_target: current_link.map(|s| s.to_string()),
            link_frame: current_frame.map(|s| s.to_string()),
            owner_inline_id,
        });
    }
}

pub(crate) fn extract_text_content(node: &LayoutBox, out: &mut String) {
    match &node.box_type {
        BoxType::TextNode(t) => out.push_str(t),
        _ => {
            for child in &node.children {
                extract_text_content(child, out);
            }
        }
    }
}
