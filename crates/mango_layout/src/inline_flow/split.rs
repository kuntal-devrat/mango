use std::sync::Arc;

use mango_css::computed::ComputedStyle;

use super::atom::{InlineAtom, InlineAtomKind};
use super::metrics::measure_text_width_with_style_and_spacing;
use super::shaping::{font_family_for, font_weight_for};

pub(crate) fn split_overflow_text(
    text: &str,
    style: &Arc<ComputedStyle>,
    available_width: f32,
    height: f32,
    link_target: Option<&str>,
    link_frame: Option<&str>,
    container_width: f32,
    owner_inline_id: Option<mango_html::dom::NodeId>,
) -> Vec<InlineAtom> {
    let font_size = style.font_size;
    let weight = font_weight_for(style);
    let family = font_family_for(style);
    let letter_spacing_px = style.letter_spacing.to_px(font_size, 16.0, container_width);

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut cur_w = 0.0;

    for ch in text.chars() {
        let s = ch.to_string();
        let w = measure_text_width_with_style_and_spacing(
            &s,
            font_size,
            weight,
            family,
            letter_spacing_px,
        );
        if !current.is_empty() && (cur_w + w > available_width) {
            chunks.push(InlineAtom {
                kind: InlineAtomKind::Text {
                    text: std::mem::take(&mut current),
                    is_space: false,
                },
                style: style.clone(),
                width: cur_w,
                height,
                link_target: link_target.map(|s| s.to_string()),
                link_frame: link_frame.map(|s| s.to_string()),
                owner_inline_id,
            });
            cur_w = 0.0;
        }
        current.push(ch);
        cur_w += w;
    }

    if !current.is_empty() {
        chunks.push(InlineAtom {
            kind: InlineAtomKind::Text {
                text: current,
                is_space: false,
            },
            style: style.clone(),
            width: cur_w,
            height,
            link_target: link_target.map(|s| s.to_string()),
            link_frame: link_frame.map(|s| s.to_string()),
            owner_inline_id,
        });
    }

    chunks
}

pub(crate) fn try_auto_hyphenate(
    text: &str,
    style: &ComputedStyle,
    max_w: f32,
    container_width: f32,
) -> Option<(String, String)> {
    let font_size = style.font_size;
    let weight = font_weight_for(style);
    let family = font_family_for(style);
    let letter_spacing = style.letter_spacing.to_px(font_size, 16.0, container_width);

    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 6 {
        return None;
    }

    let mut best_split = None;
    for i in (2..=chars.len() - 2).rev() {
        let prefix: String = chars[..i].iter().collect();
        let prefix_with_hyphen = format!("{}-", prefix);
        let w = measure_text_width_with_style_and_spacing(
            &prefix_with_hyphen,
            font_size,
            weight,
            family,
            letter_spacing,
        );
        if w <= max_w {
            let suffix: String = chars[i..].iter().collect();
            best_split = Some((prefix, suffix));
            break;
        }
    }
    best_split
}

#[allow(dead_code)]
pub(crate) fn to_roman(mut n: usize) -> String {
    let roman_pairs = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut result = String::new();
    for (val, sym) in roman_pairs {
        while n >= val {
            result.push_str(sym);
            n -= val;
        }
    }
    if result.is_empty() {
        "I".to_string()
    } else {
        result
    }
}
