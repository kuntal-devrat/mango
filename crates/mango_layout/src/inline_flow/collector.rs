use std::sync::Arc;

use mango_core::{EdgeSizes, Rect};
use mango_css::computed::ComputedStyle;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

use super::atom::{InlineAtom, InlineAtomKind};
use super::metrics::{measure_text_width_with_style, measure_text_width_with_style_and_spacing};
use super::shaping::{
    font_family_for, font_weight_for, is_cjk_char, is_cjk_closing_punct, is_cjk_opening_punct,
    is_combining_mark, used_line_height,
};

pub(crate) fn collect_inline_atoms(
    box_node: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    link_target: Option<&str>,
    link_frame: Option<&str>,
    container_width: f32,
    container_height: f32,
    active_bidi_override: Option<mango_css::values::UnicodeBidi>,
) {
    collect_inline_atoms_internal(
        box_node,
        out,
        link_target,
        link_frame,
        container_width,
        container_height,
        None,
        None,
        active_bidi_override,
    );
}

fn collect_inline_atoms_internal(
    box_node: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    link_target: Option<&str>,
    link_frame: Option<&str>,
    container_width: f32,
    container_height: f32,
    owner_inline_id: Option<mango_html::dom::NodeId>,
    positioned_inline_style: Option<&Arc<ComputedStyle>>,
    active_bidi_override: Option<mango_css::values::UnicodeBidi>,
) {
    let (next_owner_id, next_pos_style) = if box_node.box_type == BoxType::InlineNode
        && box_node
            .style
            .as_ref()
            .map(|s| s.position != mango_css::values::Position::Static)
            .unwrap_or(false)
    {
        (
            box_node.node_id.or(owner_inline_id),
            box_node.style.as_ref(),
        )
    } else {
        (owner_inline_id, positioned_inline_style)
    };

    let mut style = box_node
        .style
        .clone()
        .unwrap_or_else(|| Arc::new(ComputedStyle::default()));

    let current_bidi_override = match style.unicode_bidi {
        mango_css::values::UnicodeBidi::BidiOverride
        | mango_css::values::UnicodeBidi::IsolateOverride => Some(style.unicode_bidi),
        mango_css::values::UnicodeBidi::Isolate | mango_css::values::UnicodeBidi::Plaintext => None,
        mango_css::values::UnicodeBidi::Normal | mango_css::values::UnicodeBidi::Embed => {
            active_bidi_override
        }
    };

    if let Some(pos_style) = next_pos_style {
        if matches!(box_node.box_type, BoxType::TextNode(_)) {
            let mut merged = (*style).clone();
            merged.position = pos_style.position;
            merged.top = pos_style.top;
            merged.bottom = pos_style.bottom;
            merged.left = pos_style.left;
            merged.right = pos_style.right;
            merged.z_index = pos_style.z_index;
            if let Some(bidi_override) = current_bidi_override {
                merged.unicode_bidi = bidi_override;
            }
            style = Arc::new(merged);
        } else if let Some(bidi_override) = current_bidi_override {
            if style.unicode_bidi != bidi_override {
                let mut merged = (*style).clone();
                merged.unicode_bidi = bidi_override;
                style = Arc::new(merged);
            }
        }
    } else if let Some(bidi_override) = current_bidi_override {
        if style.unicode_bidi != bidi_override {
            let mut merged = (*style).clone();
            merged.unicode_bidi = bidi_override;
            style = Arc::new(merged);
        }
    }
    if style.float != mango_css::values::Float::None {
        let box_clone = box_node.clone();
        out.push(InlineAtom {
            kind: InlineAtomKind::Float(Box::new(box_clone)),
            style: style.clone(),
            width: 0.0,
            height: 0.0,
            link_target: link_target.map(|s| s.to_string()),
            link_frame: link_frame.map(|s| s.to_string()),
            owner_inline_id: next_owner_id,
        });
        return;
    }
    // `position` is not inherited. Text nodes can carry a cloned computed
    // style from an absolutely positioned ancestor, but the text still
    // participates in that ancestor's inline formatting context. Only an
    // actual element box is out-of-flow here.
    if !matches!(box_node.box_type, BoxType::TextNode(_))
        && matches!(
            style.position,
            mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
        )
    {
        let mut box_clone = box_node.clone();
        box_clone.containing_inline_id = next_owner_id;
        box_clone.positioned_inline_style = next_pos_style.cloned();
        out.push(InlineAtom {
            kind: InlineAtomKind::OutOfFlow(Box::new(box_clone)),
            style: style.clone(),
            width: 0.0,
            height: 0.0,
            link_target: None,
            link_frame: None,
            owner_inline_id: next_owner_id,
        });
        return;
    }
    let font_size = style.font_size;
    let weight = font_weight_for(&style);
    let family = font_family_for(&style);
    let line_height = used_line_height(&style, family, weight);

    if box_node.tag_name.as_deref() == Some("br") {
        out.push(InlineAtom {
            kind: InlineAtomKind::LineBreak { is_br: true },
            style: style.clone(),
            width: 0.0,
            height: line_height,
            link_target: None,
            link_frame: None,
            owner_inline_id: next_owner_id,
        });
        return;
    }

    let current_link = box_node.link_target.as_deref().or(link_target);
    let current_frame = box_node.link_frame.as_deref().or(link_frame);

    if box_node.tag_name.as_deref() == Some("wbr") {
        out.push(InlineAtom {
            kind: InlineAtomKind::WordBreakOpportunity,
            style: style.clone(),
            width: 0.0,
            height: 0.0,
            link_target: current_link.map(|s| s.to_string()),
            link_frame: current_frame.map(|s| s.to_string()),
            owner_inline_id: next_owner_id,
        });
        return;
    }

    match &box_node.box_type {
        BoxType::TextNode(raw_text) => {
            let transformed_text = style.text_transform.apply(raw_text);
            let shaped_text = if transformed_text
                .chars()
                .any(crate::shaping::is_complex_char)
            {
                crate::shaping::shape_complex_script(&transformed_text)
            } else {
                transformed_text
            };
            let text = &shaped_text;
            let ws = style.white_space;
            let preserve_spaces = matches!(
                ws,
                mango_css::values::WhiteSpace::Pre
                    | mango_css::values::WhiteSpace::PreWrap
                    | mango_css::values::WhiteSpace::BreakSpaces
            );
            let preserve_newlines = matches!(
                ws,
                mango_css::values::WhiteSpace::Pre
                    | mango_css::values::WhiteSpace::PreWrap
                    | mango_css::values::WhiteSpace::PreLine
                    | mango_css::values::WhiteSpace::BreakSpaces
            );
            let letter_spacing_px = style.letter_spacing.to_px(font_size, 16.0, container_width);
            let word_spacing_px = style.word_spacing.to_px(font_size, 16.0, container_width);

            let push_word = |word: String, out: &mut Vec<InlineAtom>| {
                if word.is_empty() {
                    return;
                }
                if style.word_break == mango_css::values::WordBreak::BreakAll {
                    for c in word.chars() {
                        let c_str = c.to_string();
                        let w = measure_text_width_with_style_and_spacing(
                            &c_str,
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        );
                        out.push(InlineAtom {
                            kind: InlineAtomKind::Text {
                                text: c_str,
                                is_space: false,
                            },
                            style: style.clone(),
                            width: w,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    }
                } else {
                    let w = measure_text_width_with_style_and_spacing(
                        &word,
                        font_size,
                        weight,
                        family,
                        letter_spacing_px,
                    );
                    out.push(InlineAtom {
                        kind: InlineAtomKind::Text {
                            text: word,
                            is_space: false,
                        },
                        style: style.clone(),
                        width: w,
                        height: line_height,
                        link_target: current_link.map(|s| s.to_string()),
                        link_frame: current_frame.map(|s| s.to_string()),
                        owner_inline_id: next_owner_id,
                    });
                }
            };

            let mut current_word = String::new();
            for ch in text.chars() {
                if ch == '\u{00AD}' {
                    if style.hyphens != mango_css::values::Hyphens::None {
                        push_word(std::mem::take(&mut current_word), out);
                        out.push(InlineAtom {
                            kind: InlineAtomKind::SoftHyphen,
                            style: style.clone(),
                            width: 0.0,
                            height: 0.0,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    }
                    continue;
                } else if ch == '\u{200B}' {
                    push_word(std::mem::take(&mut current_word), out);
                    out.push(InlineAtom {
                        kind: InlineAtomKind::WordBreakOpportunity,
                        style: style.clone(),
                        width: 0.0,
                        height: 0.0,
                        link_target: current_link.map(|s| s.to_string()),
                        link_frame: current_frame.map(|s| s.to_string()),
                        owner_inline_id: next_owner_id,
                    });
                } else if ch == '\n' {
                    push_word(std::mem::take(&mut current_word), out);
                    if preserve_newlines {
                        out.push(InlineAtom {
                            kind: InlineAtomKind::LineBreak { is_br: false },
                            style: style.clone(),
                            width: 0.0,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    } else if out
                        .iter()
                        .rev()
                        .find(|a| !matches!(a.kind, InlineAtomKind::OutOfFlow(_)))
                        .map(|a| !a.is_space())
                        .unwrap_or(true)
                    {
                        let space_w = (measure_text_width_with_style_and_spacing(
                            " ",
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        ) + word_spacing_px)
                            .max(0.0);
                        out.push(InlineAtom {
                            kind: InlineAtomKind::Text {
                                text: " ".to_string(),
                                is_space: true,
                            },
                            style: style.clone(),
                            width: space_w,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    }
                } else if ch.is_ascii_whitespace() {
                    push_word(std::mem::take(&mut current_word), out);
                    if ws == mango_css::values::WhiteSpace::BreakSpaces {
                        let space_char = if ch == '\t' { "    " } else { " " };
                        let space_w = (measure_text_width_with_style_and_spacing(
                            space_char,
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        ) + word_spacing_px)
                            .max(0.0);
                        out.push(InlineAtom {
                            kind: InlineAtomKind::Text {
                                text: space_char.to_string(),
                                is_space: true,
                            },
                            style: style.clone(),
                            width: space_w,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    } else if preserve_spaces
                        || out
                            .iter()
                            .rev()
                            .find(|a| !matches!(a.kind, InlineAtomKind::OutOfFlow(_)))
                            .map(|a| {
                                !a.is_space() && !matches!(a.kind, InlineAtomKind::LineBreak { .. })
                            })
                            .unwrap_or(false)
                    {
                        let space_char = if ch == '\t' { "    " } else { " " };
                        let space_w = (measure_text_width_with_style_and_spacing(
                            space_char,
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        ) + word_spacing_px)
                            .max(0.0);
                        out.push(InlineAtom {
                            kind: InlineAtomKind::Text {
                                text: space_char.to_string(),
                                is_space: true,
                            },
                            style: style.clone(),
                            width: space_w,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                            link_frame: current_frame.map(|s| s.to_string()),
                            owner_inline_id: next_owner_id,
                        });
                    }
                } else if is_cjk_closing_punct(ch)
                    && style.word_break != mango_css::values::WordBreak::BreakAll
                {
                    // CJK closing punctuation cannot begin a line. Attach to current word or previous atom if possible.
                    if !current_word.is_empty() {
                        current_word.push(ch);
                        push_word(std::mem::take(&mut current_word), out);
                    } else if let Some(last_atom) = out.last_mut() {
                        if let InlineAtomKind::Text { text, is_space } = &mut last_atom.kind {
                            if !*is_space {
                                text.push(ch);
                                last_atom.width = measure_text_width_with_style_and_spacing(
                                    text,
                                    font_size,
                                    weight,
                                    family,
                                    letter_spacing_px,
                                );
                            } else {
                                current_word.push(ch);
                            }
                        } else {
                            current_word.push(ch);
                        }
                    } else {
                        current_word.push(ch);
                    }
                } else if is_cjk_opening_punct(ch)
                    && style.word_break != mango_css::values::WordBreak::BreakAll
                {
                    // CJK opening punctuation cannot end a line. It must attach to the following character.
                    if !current_word.is_empty() && !current_word.chars().all(is_cjk_opening_punct) {
                        push_word(std::mem::take(&mut current_word), out);
                    }
                    current_word.push(ch);
                } else if is_cjk_char(ch)
                    && style.word_break != mango_css::values::WordBreak::KeepAll
                    && style.word_break != mango_css::values::WordBreak::BreakAll
                {
                    // In CJK scripts (Chinese, Japanese, Korean), characters break across lines freely.
                    if !current_word.is_empty() && !current_word.chars().all(is_cjk_opening_punct) {
                        push_word(std::mem::take(&mut current_word), out);
                    }
                    current_word.push(ch);
                    push_word(std::mem::take(&mut current_word), out);
                } else if crate::shaping::is_thai_char(ch)
                    && style.word_break != mango_css::values::WordBreak::KeepAll
                {
                    let is_syllable_boundary = if let Some(prev) = current_word.chars().last() {
                        matches!(ch, '\u{0E40}'..='\u{0E44}')
                            || (matches!(ch, '\u{0E01}'..='\u{0E2E}')
                                && (matches!(prev, '\u{0E30}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}')))
                    } else {
                        false
                    };
                    if is_syllable_boundary {
                        push_word(std::mem::take(&mut current_word), out);
                    }
                    current_word.push(ch);
                } else if is_combining_mark(ch) {
                    // Combining marks must attach to the current word or previous atom
                    if !current_word.is_empty() {
                        current_word.push(ch);
                    } else if let Some(last_atom) = out.last_mut() {
                        if let InlineAtomKind::Text { text, is_space } = &mut last_atom.kind {
                            if !*is_space {
                                text.push(ch);
                                last_atom.width = measure_text_width_with_style_and_spacing(
                                    text,
                                    font_size,
                                    weight,
                                    family,
                                    letter_spacing_px,
                                );
                            } else {
                                current_word.push(ch);
                            }
                        } else {
                            current_word.push(ch);
                        }
                    } else {
                        current_word.push(ch);
                    }
                } else {
                    current_word.push(ch);
                }
            }

            push_word(current_word, out);
        }

        BoxType::InlineNode
            if box_node.tag_name.as_deref() == Some("ruby")
                || box_node
                    .style
                    .as_ref()
                    .map(|s| s.display == mango_css::values::Display::Ruby)
                    .unwrap_or(false) =>
        {
            collect_ruby_atoms(
                box_node,
                out,
                current_link,
                current_frame,
                container_width,
                container_height,
                next_owner_id,
            );
        }

        BoxType::InlineNode => {
            let font_size = style.font_size;
            let root_font_size = style.root_font_size;
            let (vp_w, _) = mango_css::get_current_viewport();
            let lead_spacing = style
                .margin_left
                .to_px_with_viewport(font_size, root_font_size, container_width, vp_w)
                .max(0.0)
                + style.border_left_width.max(0.0)
                + style
                    .padding_left
                    .to_px_with_viewport(font_size, root_font_size, container_width, vp_w)
                    .max(0.0);
            if lead_spacing > 0.0 {
                out.push(InlineAtom {
                    kind: InlineAtomKind::Spacing,
                    style: style.clone(),
                    width: lead_spacing,
                    height: 0.0,
                    link_target: current_link.map(|s| s.to_string()),
                    link_frame: current_frame.map(|s| s.to_string()),
                    owner_inline_id: next_owner_id,
                });
            }

            for child in &box_node.children {
                collect_inline_atoms_internal(
                    child,
                    out,
                    current_link,
                    current_frame,
                    container_width,
                    container_height,
                    next_owner_id,
                    next_pos_style,
                    current_bidi_override,
                );
            }

            let trail_spacing = style
                .padding_right
                .to_px_with_viewport(font_size, root_font_size, container_width, vp_w)
                .max(0.0)
                + style.border_right_width.max(0.0)
                + style
                    .margin_right
                    .to_px_with_viewport(font_size, root_font_size, container_width, vp_w)
                    .max(0.0);
            if trail_spacing > 0.0 {
                out.push(InlineAtom {
                    kind: InlineAtomKind::Spacing,
                    style: style.clone(),
                    width: trail_spacing,
                    height: 0.0,
                    link_target: current_link.map(|s| s.to_string()),
                    link_frame: current_frame.map(|s| s.to_string()),
                    owner_inline_id: next_owner_id,
                });
            }
        }

        BoxType::InlineBlock
        | BoxType::ReplacedElement { .. }
        | BoxType::IFrame { .. }
        | BoxType::Video { .. }
        | BoxType::Audio { .. }
        | BoxType::Canvas { .. } => {
            let mut box_clone = box_node.clone();
            let cb = Dimensions {
                content: Rect::new(0.0, 0.0, container_width, container_height),
                padding: EdgeSizes::ZERO,
                border: EdgeSizes::ZERO,
                margin: EdgeSizes::ZERO,
                ..Default::default()
            };
            let mut inner_float_ctx = FloatContext::new();
            crate::block_flow::layout_block(&mut box_clone, &cb, &mut inner_float_ctx);

            let outer_w = box_clone.dimensions.margin_box().width();
            let outer_h = box_clone.dimensions.margin_box().height();

            out.push(InlineAtom {
                kind: InlineAtomKind::AtomicBox(Box::new(box_clone)),
                style: style.clone(),
                width: outer_w,
                height: outer_h,
                link_target: current_link.map(|s| s.to_string()),
                link_frame: current_frame.map(|s| s.to_string()),
                owner_inline_id: next_owner_id,
            });
        }

        BoxType::BlockNode => {
            // CSS 2.1 §9.2.1.1 IB-split: A block inside an inline forces a line break before and after,
            // occupying its own line. Per CSS Position 3 §2.1, the block itself is not part of the inline box.
            while let Some(last) = out.last() {
                if last.is_space() {
                    out.pop();
                } else {
                    break;
                }
            }
            if !out.is_empty()
                && !matches!(
                    out.last().map(|a| &a.kind),
                    Some(InlineAtomKind::LineBreak { .. })
                )
            {
                out.push(InlineAtom {
                    kind: InlineAtomKind::LineBreak { is_br: false },
                    style: style.clone(),
                    width: 0.0,
                    height: 0.0,
                    link_target: None,
                    link_frame: None,
                    owner_inline_id: None,
                });
            }

            let mut box_clone = box_node.clone();
            let cb = Dimensions {
                content: Rect::new(0.0, 0.0, container_width, container_height),
                padding: EdgeSizes::ZERO,
                border: EdgeSizes::ZERO,
                margin: EdgeSizes::ZERO,
                ..Default::default()
            };
            let mut inner_float_ctx = FloatContext::new();
            crate::block_flow::layout_block(&mut box_clone, &cb, &mut inner_float_ctx);

            let outer_w = box_clone.dimensions.margin_box().width();
            let outer_h = box_clone.dimensions.margin_box().height();

            out.push(InlineAtom {
                kind: InlineAtomKind::AtomicBox(Box::new(box_clone)),
                style: style.clone(),
                width: outer_w,
                height: outer_h,
                link_target: current_link.map(|s| s.to_string()),
                link_frame: current_frame.map(|s| s.to_string()),
                owner_inline_id: None,
            });

            out.push(InlineAtom {
                kind: InlineAtomKind::LineBreak { is_br: false },
                style: style.clone(),
                width: 0.0,
                height: 0.0,
                link_target: None,
                link_frame: None,
                owner_inline_id: None,
            });
        }

        _ => {}
    }
}

pub(crate) fn collect_ruby_atoms(
    ruby_node: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    current_link: Option<&str>,
    current_frame: Option<&str>,
    _container_width: f32,
    _container_height: f32,
    owner_inline_id: Option<mango_html::dom::NodeId>,
) {
    let ruby_style = ruby_node
        .style
        .clone()
        .unwrap_or_else(|| Arc::new(ComputedStyle::default()));
    let font_size = ruby_style.font_size;

    // Collect base elements and rt elements
    let mut bases: Vec<(String, Arc<ComputedStyle>)> = Vec::new();
    let mut rts: Vec<(String, Arc<ComputedStyle>)> = Vec::new();

    for child in &ruby_node.children {
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
        ruby_pair_box.tag_name = Some("ruby".to_string());
        ruby_pair_box.dimensions.content = Rect::new(0.0, 0.0, pair_w, pair_h);

        // Child 0: RT text box
        let mut rt_box = LayoutBox::new(BoxType::TextNode(rt_txt), Some(rt_style));
        rt_box.tag_name = Some("rt".to_string());
        rt_box.dimensions.content = Rect::new(rt_x, 0.0, rt_w, rt_h);
        ruby_pair_box.children.push(rt_box);

        // Child 1: Base text box
        let mut base_box = LayoutBox::new(BoxType::TextNode(b_txt), Some(b_style.clone()));
        base_box.tag_name = Some("rb".to_string());
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
