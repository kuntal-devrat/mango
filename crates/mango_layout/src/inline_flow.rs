//! Inline formatting context: line breaking, word wrapping, text measurement, and text-align.
//!
//! Implements CSS 2.1 §9.4.2: Inline formatting contexts.

use mango_core::{EdgeSizes, Point, Rect};
use mango_css::computed::ComputedStyle;
use mango_css::values::TextAlign;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

/// An atomic inline fragment kind: either text (word/space), an atomic box (inline-block/replaced), or a forced line break.
#[derive(Debug, Clone)]
enum InlineAtomKind {
    Text { text: String, is_space: bool },
    AtomicBox(Box<LayoutBox>),
    LineBreak,
    WordBreakOpportunity,
    SoftHyphen,
    Spacing,
}

/// An atomic inline fragment (a word, a whitespace gap, or an inline element box).
#[derive(Debug, Clone)]
struct InlineAtom {
    kind: InlineAtomKind,
    style: ComputedStyle,
    width: f32,
    height: f32,
    link_target: Option<String>,
}

impl InlineAtom {
    fn is_space(&self) -> bool {
        match &self.kind {
            InlineAtomKind::Text { is_space, .. } => *is_space,
            InlineAtomKind::AtomicBox(_)
            | InlineAtomKind::LineBreak
            | InlineAtomKind::WordBreakOpportunity
            | InlineAtomKind::SoftHyphen
            | InlineAtomKind::Spacing => false,
        }
    }
}

/// Returns true if a character is a combining diacritical mark or vowel/tone mark.
pub fn is_combining_mark(ch: char) -> bool {
    matches!(ch,
        '\u{0300}'..='\u{036F}' // Combining Diacritical Marks
        | '\u{1AB0}'..='\u{1AFF}' // Combining Diacritical Marks Extended
        | '\u{1DC0}'..='\u{1DFF}' // Combining Diacritical Marks Supplement
        | '\u{20D0}'..='\u{20FF}' // Combining Diacritical Marks for Symbols
        | '\u{FE20}'..='\u{FE2F}' // Combining Half Marks
        | '\u{0E31}' | '\u{0E34}'..='\u{0E3A}' | '\u{0E47}'..='\u{0E4E}' // Thai combining vowels and tones
        | '\u{064B}'..='\u{065F}' | '\u{0670}' // Arabic Tashkeel (Fatha, Damma, Kasra, Shadda, Sukun)
        | '\u{0591}'..='\u{05BD}' | '\u{05BF}' | '\u{05C1}' | '\u{05C2}' | '\u{05C4}' | '\u{05C5}' | '\u{05C7}' // Hebrew Niqqud / Cantillation
        | '\u{0901}'..='\u{0903}' | '\u{093C}' | '\u{093E}'..='\u{094C}' | '\u{094D}' | '\u{0951}'..='\u{0954}' // Devanagari vowel signs & virama
    )
}

/// Returns true if a character belongs to CJK writing systems (which break lines at character boundaries).
pub fn is_cjk_char(ch: char) -> bool {
    if is_combining_mark(ch) {
        return false;
    }
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{20000}'..='\u{2FA1F}' // CJK Extension B-I
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{31F0}'..='\u{31FF}' // Katakana Extensions
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' // Hangul Jamo
        | '\u{3100}'..='\u{312F}' // Bopomofo
    )
}

/// Returns true if a character is CJK closing punctuation (which cannot begin a line).
pub fn is_cjk_closing_punct(ch: char) -> bool {
    matches!(
        ch,
        '）' | '」'
            | '』'
            | '】'
            | '》'
            | '”'
            | '’'
            | '〕'
            | '〗'
            | '〙'
            | '。'
            | '，'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | ')'
            | ']'
            | '}'
            | '>'
            | '!'
            | '?'
            | ','
            | '.'
            | ':'
            | ';'
    )
}

/// Returns true if a character is CJK opening punctuation (which cannot end a line).
pub fn is_cjk_opening_punct(ch: char) -> bool {
    matches!(
        ch,
        '（' | '「'
            | '『'
            | '【'
            | '《'
            | '“'
            | '‘'
            | '〔'
            | '〖'
            | '〘'
            | '('
            | '['
            | '{'
            | '<'
    )
}

/// Returns true if a character belongs to an RTL script (Arabic or Hebrew).
pub fn is_rtl_char(ch: char) -> bool {
    matches!(ch,
        '\u{0590}'..='\u{05FF}' // Hebrew
        | '\u{0600}'..='\u{06FF}' // Arabic
        | '\u{0750}'..='\u{077F}' // Arabic Supplement
        | '\u{08A0}'..='\u{08FF}' // Arabic Extended-A
        | '\u{FB50}'..='\u{FDFF}' // Arabic Presentation Forms-A
        | '\u{FE70}'..='\u{FEFF}' // Arabic Presentation Forms-B
    )
}

/// Shapes complex scripts (Arabic, Devanagari, Thai) and visually orders RTL text with paragraph direction using UAX#9.
pub fn shape_and_bidi_text_with_dir(text: &str, is_rtl_dir: bool) -> String {
    let shaped = crate::shaping::shape_complex_script(text);
    if !shaped.chars().any(is_rtl_char) && !is_rtl_dir {
        return shaped;
    }
    crate::bidi::reorder_bidi_text(&shaped, is_rtl_dir)
}

/// Shapes complex scripts and visually orders RTL text for LTR rasterization.
pub fn shape_and_bidi_text(text: &str) -> String {
    shape_and_bidi_text_with_dir(text, false)
}

/// Resolves the render weight for a computed style (CSS 400/700 → Regular/Bold).
fn font_weight_for(style: &ComputedStyle) -> mango_render::FontWeight {
    let bold = match style.font_weight {
        mango_css::values::FontWeight::Bold | mango_css::values::FontWeight::Bolder => true,
        mango_css::values::FontWeight::Numeric(w) => w >= 600,
        _ => false,
    };
    if bold {
        mango_render::FontWeight::Bold
    } else {
        mango_render::FontWeight::Regular
    }
}

/// Resolves the render font family for a computed style.
fn font_family_for(style: &ComputedStyle) -> mango_render::FontFamily {
    mango_render::FontFamily::from_css_name(&style.font_family)
}

/// The used line height of a run: the declared `line-height`, or the font's own
/// `ascent + descent + line-gap` for `normal` — which is how Chromium computes it,
/// rather than a fixed multiple of the font size (CSS 2.1 §10.8.1).
fn used_line_height(
    style: &ComputedStyle,
    family: mango_render::FontFamily,
    weight: mango_render::FontWeight,
) -> f32 {
    if let Some(explicit) = style.line_height {
        return explicit.max(0.0);
    }
    let (ascent, descent, gap) =
        mango_render::font::font_manager().font_metrics(family, weight, style.font_size);
    (ascent + descent + gap).max(1.0)
}

/// Measures the width of a string in pixels using `fontdue` proportional advance widths with default styling.
pub fn measure_text_width(text: &str, font_size: f32) -> f32 {
    let fm = mango_render::font::font_manager();
    let (width, _height) = fm.measure_text(
        text,
        font_size,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    );
    width
}

/// Measures text width using explicit font size, weight, and family.
pub fn measure_text_width_with_style(
    text: &str,
    font_size: f32,
    weight: mango_render::FontWeight,
    family: mango_render::FontFamily,
) -> f32 {
    measure_text_width_with_style_and_spacing(text, font_size, weight, family, 0.0)
}

/// Measures text width using explicit font size, weight, family, and letter spacing.
pub fn measure_text_width_with_style_and_spacing(
    text: &str,
    font_size: f32,
    weight: mango_render::FontWeight,
    family: mango_render::FontFamily,
    letter_spacing: f32,
) -> f32 {
    let fm = mango_render::font::font_manager();
    let (width, _height) =
        fm.measure_text_with_spacing(text, font_size, weight, family, letter_spacing);
    width
}

/// Executes inline formatting context layout for the children of a block container.
///
/// Converts the inline children into positioned line boxes, breaking lines at word boundaries
/// according to available width and applying horizontal text alignment.
pub fn layout_inline_children(container: &mut LayoutBox, float_ctx: &mut FloatContext) -> f32 {
    let container_x = container.dimensions.content.x();
    let container_y = container.dimensions.content.y();
    let container_width = container.dimensions.content.width();
    let text_align = container
        .style
        .as_ref()
        .map(|s| {
            if s.direction == mango_css::values::Direction::Rtl && s.text_align == TextAlign::Left {
                TextAlign::Right
            } else {
                s.text_align
            }
        })
        .unwrap_or(TextAlign::Left);

    let container_height = if container.dimensions.content.height() > 0.0 {
        container.dimensions.content.height()
    } else if let Some(style) = &container.style {
        let font_size = style.font_size;
        let root_font_size = style.root_font_size;
        let (_, vp_h) = mango_css::get_current_viewport();
        let h = style
            .height
            .to_px_with_viewport(font_size, root_font_size, vp_h, vp_h);
        if h > 0.0 {
            h
        } else {
            let max_h = style
                .max_height
                .to_px_with_viewport(font_size, root_font_size, vp_h, vp_h);
            if max_h > 0.0 && style.max_height != mango_css::values::Length::Auto {
                max_h
            } else {
                0.0
            }
        }
    } else {
        0.0
    };

    let writing_mode = container
        .style
        .as_ref()
        .map(|s| s.writing_mode)
        .unwrap_or(mango_css::values::WritingMode::HorizontalTb);
    if matches!(
        writing_mode,
        mango_css::values::WritingMode::VerticalRl | mango_css::values::WritingMode::VerticalLr
    ) {
        return layout_vertical_inline_children(container, float_ctx, writing_mode);
    }

    // 1. Preserve unflattened source children for idempotent relayout passes (GAP-003)
    let source_children = container
        .raw_children
        .get_or_insert_with(|| container.children.clone())
        .clone();

    // Collect all inline atoms from children (no synthetic spaces between TextNodes)
    let mut atoms: Vec<InlineAtom> = Vec::new();
    for child in &source_children {
        collect_inline_atoms(
            child,
            &mut atoms,
            child.link_target.as_deref(),
            container_width,
            container_height,
        );
    }

    // 1b. Prepend list marker atom if container is a list item
    let is_list_item = container.style.as_ref().map(|s| s.display)
        == Some(mango_css::values::Display::ListItem)
        || container.tag_name.as_deref() == Some("li");

    if is_list_item {
        let mut style = container.style.clone().unwrap_or_default();
        if let Some(c) = container
            .get_attribute("_mango_marker_color")
            .and_then(mango_css::values::Value::parse_color)
        {
            style.color = c;
        }

        let marker_str = if let Some(custom) = container.get_attribute("_mango_marker_content") {
            Some(custom.to_string())
        } else {
            match style.list_style_type {
                mango_css::values::ListStyleType::None => None,
                mango_css::values::ListStyleType::Disc => Some("• ".to_string()),
                mango_css::values::ListStyleType::Circle => Some("○ ".to_string()),
                mango_css::values::ListStyleType::Square => Some("▪ ".to_string()),
                mango_css::values::ListStyleType::Decimal => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .or_else(|| container.get_attribute("value"))
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", idx))
                }
                mango_css::values::ListStyleType::LowerAlpha => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    let ch = (b'a' + ((idx - 1) % 26) as u8) as char;
                    Some(format!("{}. ", ch))
                }
                mango_css::values::ListStyleType::UpperAlpha => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    let ch = (b'A' + ((idx - 1) % 26) as u8) as char;
                    Some(format!("{}. ", ch))
                }
                mango_css::values::ListStyleType::LowerRoman => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", to_roman(idx).to_lowercase()))
                }
                mango_css::values::ListStyleType::UpperRoman => {
                    let idx = container
                        .get_attribute("_mango_list_index")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1);
                    Some(format!("{}. ", to_roman(idx)))
                }
            }
        };

        if let Some(m_text) = marker_str {
            let already_has_bullet = atoms
                .iter()
                .find_map(|a| match &a.kind {
                    InlineAtomKind::Text { text, .. } if !text.trim().is_empty() => {
                        let trimmed = text.trim_start();
                        Some(
                            trimmed.starts_with('•')
                                || trimmed.starts_with('\u{2022}')
                                || trimmed.starts_with('○')
                                || trimmed.starts_with('▪')
                                || trimmed.starts_with(&m_text),
                        )
                    }
                    _ => None,
                })
                .unwrap_or(false);

            if !already_has_bullet {
                let m_font_size = style.font_size;
                let m_w = measure_text_width(&m_text, m_font_size);
                let m_h = (m_font_size * 1.2).ceil();
                atoms.insert(
                    0,
                    InlineAtom {
                        kind: InlineAtomKind::Text {
                            text: m_text,
                            is_space: false,
                        },
                        style,
                        width: m_w,
                        height: m_h,
                        link_target: None,
                    },
                );
            }
        }
    }

    if atoms.is_empty() {
        let mut oof_children = Vec::new();
        for sc in &source_children {
            let is_oof = sc.style.as_ref().is_some_and(|s| {
                matches!(
                    s.position,
                    mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                )
            });
            if is_oof {
                oof_children.push(sc.clone());
            }
        }
        container.children = oof_children;
        return 0.0;
    }

    // 2. Break atoms into lines
    let mut positioned_boxes = Vec::new();
    let mut current_y = container_y;

    let is_container_rtl = container
        .style
        .as_ref()
        .map(|s| s.direction == mango_css::values::Direction::Rtl)
        .unwrap_or(false);

    let text_indent = container
        .style
        .as_ref()
        .map(|s| s.text_indent.to_px(s.font_size, 16.0, container_width))
        .unwrap_or(0.0);
    let mut is_first_line = true;

    let get_line_x_and_width = |min_x: f32, max_x: f32, is_first: bool| -> (f32, f32) {
        let line_indent = if is_first { text_indent } else { 0.0 };
        let avail_w = (max_x - min_x - line_indent).max(0.0);
        let start_x = if is_container_rtl {
            min_x
        } else {
            min_x + line_indent
        };
        (start_x, avail_w)
    };

    let max_lines = container
        .style
        .as_ref()
        .and_then(|s| match s.line_clamp {
            mango_css::values::LineClamp::Lines(n) if n > 0 => Some(n as usize),
            _ => None,
        })
        .or_else(|| {
            atoms.first().and_then(|a| match a.style.line_clamp {
                mango_css::values::LineClamp::Lines(n) if n > 0 => Some(n as usize),
                _ => None,
            })
        });
    let mut line_count = 0;

    let mut line_atoms: Vec<InlineAtom> = Vec::new();
    let mut line_width = 0.0;

    let mut atom_queue: std::collections::VecDeque<InlineAtom> = atoms.into();

    while let Some(atom) = atom_queue.pop_front() {
        if matches!(atom.kind, InlineAtomKind::LineBreak) {
            let (min_x, max_x) =
                float_ctx.available_span(current_y, atom.height, container_x, container_width);
            let (line_x, available_width) = get_line_x_and_width(min_x, max_x, is_first_line);
            let line_h = finalize_line(
                &line_atoms,
                line_x,
                available_width,
                current_y,
                text_align,
                &mut positioned_boxes,
                true,
            );
            current_y += line_h.max(atom.height);
            is_first_line = false;
            line_atoms.clear();
            line_width = 0.0;
            line_count += 1;
            if let Some(max) = max_lines
                && line_count >= max
            {
                break;
            }
            continue;
        }

        if matches!(atom.kind, InlineAtomKind::SoftHyphen) {
            line_atoms.push(atom);
            continue;
        }

        let atom_height = atom.height;
        let (mut min_x, mut max_x) =
            float_ctx.available_span(current_y, atom_height, container_x, container_width);
        let (mut line_x, mut available_width) = get_line_x_and_width(min_x, max_x, is_first_line);

        // CSS 2.1 §9.5: If a line box is too small to contain any content due to floats, shift downward
        if line_atoms.is_empty()
            && (available_width <= 0.0
                || (atom.width > available_width && float_ctx.has_floats_at(current_y, atom_height)))
        {
            if let Some(next_y) = float_ctx.next_vertical_opportunity(current_y) {
                current_y = next_y;
                let (new_min, new_max) =
                    float_ctx.available_span(current_y, atom_height, container_x, container_width);
                min_x = new_min;
                max_x = new_max;
                let (lx, aw) = get_line_x_and_width(min_x, max_x, is_first_line);
                line_x = lx;
                available_width = aw;
            }
        }

        let is_pre = matches!(
            atom.style.white_space,
            mango_css::values::WhiteSpace::Pre
                | mango_css::values::WhiteSpace::PreWrap
                | mango_css::values::WhiteSpace::BreakSpaces
        );
        // Discard leading space on a new line (unless pre/pre-wrap/break-spaces)
        if line_atoms.is_empty() && atom.is_space() && !is_pre {
            continue;
        }

        let is_nowrap = atom.style.white_space == mango_css::values::WhiteSpace::Nowrap;

        // Check overflow-wrap: break-word / anywhere or word-break: break-word
        let can_break_word = (atom.style.overflow_wrap != mango_css::values::OverflowWrap::Normal
            || atom.style.word_break == mango_css::values::WordBreak::BreakWord)
            && !is_nowrap;

        if can_break_word
            && (atom.width > available_width)
            && let InlineAtomKind::Text {
                text,
                is_space: false,
            } = &atom.kind
            && text.chars().count() > 1
            && available_width > 0.0
        {
            let mut chunks = split_overflow_text(
                text.as_str(),
                &atom.style,
                available_width,
                atom.height,
                atom.link_target.as_deref(),
                container_width,
            );
            if !chunks.is_empty() {
                let first = chunks.remove(0);
                for chk in chunks.into_iter().rev() {
                    atom_queue.push_front(chk);
                }
                atom_queue.push_front(first);
                continue;
            }
        }

        // Automatic hyphenation check
        let is_auto_hyphens = atom.style.hyphens == mango_css::values::Hyphens::Auto && !is_nowrap;
        if is_auto_hyphens
            && (line_width + atom.width > available_width)
            && let InlineAtomKind::Text {
                text,
                is_space: false,
            } = &atom.kind
            && text.chars().count() >= 6
        {
            let (can_fit_here, remaining_w) = if line_atoms.is_empty() {
                (true, available_width)
            } else if available_width > line_width + 20.0 {
                (true, available_width - line_width)
            } else {
                (false, 0.0)
            };

            if can_fit_here
                && let Some((prefix, suffix)) =
                    try_auto_hyphenate(text.as_str(), &atom.style, remaining_w, container_width)
            {
                let hyphen_atom = InlineAtom {
                    kind: InlineAtomKind::Text {
                        text: format!("{}-", prefix),
                        is_space: false,
                    },
                    style: atom.style.clone(),
                    width: remaining_w,
                    height: atom.height,
                    link_target: atom.link_target.clone(),
                };
                line_atoms.push(hyphen_atom);
                let line_h = finalize_line(
                    &line_atoms,
                    line_x,
                    available_width,
                    current_y,
                    text_align,
                    &mut positioned_boxes,
                    false,
                );
                current_y += line_h;
                is_first_line = false;
                line_atoms.clear();
                line_width = 0.0;
                line_count += 1;

                if let Some(max) = max_lines
                    && line_count >= max
                {
                    if let Some(last_box) = positioned_boxes.last_mut()
                        && let BoxType::TextNode(t) = &mut last_box.box_type
                        && !t.ends_with('…')
                    {
                        t.push('…');
                        if let Some(st) = &last_box.style {
                            let font_size = st.font_size;
                            let weight = font_weight_for(st);
                            let family = font_family_for(st);
                            let letter_spacing_px =
                                st.letter_spacing.to_px(font_size, 16.0, container_width);
                            last_box.dimensions.content.size.width =
                                measure_text_width_with_style_and_spacing(
                                    t,
                                    font_size,
                                    weight,
                                    family,
                                    letter_spacing_px,
                                );
                        }
                    }
                    break;
                }

                let suffix_w = measure_text_width_with_style(
                    &suffix,
                    atom.style.font_size,
                    font_weight_for(&atom.style),
                    font_family_for(&atom.style),
                );
                let suffix_atom = InlineAtom {
                    kind: InlineAtomKind::Text {
                        text: suffix,
                        is_space: false,
                    },
                    style: atom.style.clone(),
                    width: suffix_w,
                    height: atom.height,
                    link_target: atom.link_target,
                };
                atom_queue.push_front(suffix_atom);
                continue;
            }
        }

        if !line_atoms.is_empty() && !is_nowrap && (line_width + atom.width > available_width) {
            // Check if there was a SoftHyphen in line_atoms
            if let Some(sh_pos) = line_atoms
                .iter()
                .rposition(|a| matches!(a.kind, InlineAtomKind::SoftHyphen))
            {
                let has_space_after_sh = line_atoms[sh_pos + 1..].iter().any(|a| {
                    a.is_space() || matches!(a.kind, InlineAtomKind::WordBreakOpportunity)
                });
                if !has_space_after_sh {
                    let carryover: Vec<InlineAtom> = line_atoms.drain(sh_pos + 1..).collect();
                    for c in carryover.into_iter().rev() {
                        atom_queue.push_front(c);
                    }
                    line_atoms.pop(); // Remove SoftHyphen
                    if let Some(prev) = line_atoms.last_mut()
                        && let InlineAtomKind::Text { text, .. } = &mut prev.kind
                        && !text.ends_with('-')
                    {
                        text.push('-');
                        let font_size = prev.style.font_size;
                        let weight = font_weight_for(&prev.style);
                        let family = font_family_for(&prev.style);
                        let letter_spacing_px =
                            prev.style.letter_spacing.to_px(font_size, 16.0, container_width);
                        prev.width = measure_text_width_with_style_and_spacing(
                            text,
                            font_size,
                            weight,
                            family,
                            letter_spacing_px,
                        );
                    }
                }
            }

            // Line break
            let line_h = finalize_line(
                &line_atoms,
                line_x,
                available_width,
                current_y,
                text_align,
                &mut positioned_boxes,
                false,
            );
            current_y += line_h;
            is_first_line = false;
            line_atoms.clear();
            line_width = 0.0;
            line_count += 1;

            if let Some(max) = max_lines
                && line_count >= max
            {
                if let Some(last_box) = positioned_boxes.last_mut()
                    && let BoxType::TextNode(t) = &mut last_box.box_type
                    && !t.ends_with('…')
                {
                    t.push('…');
                    if let Some(st) = &last_box.style {
                        let font_size = st.font_size;
                        let weight = font_weight_for(st);
                        let family = font_family_for(st);
                        let letter_spacing_px =
                            st.letter_spacing.to_px(font_size, 16.0, container_width);
                        last_box.dimensions.content.size.width =
                            measure_text_width_with_style_and_spacing(
                                t,
                                font_size,
                                weight,
                                family,
                                letter_spacing_px,
                            );
                    }
                }
                break;
            }

            if atom.is_space() && !is_pre {
                continue;
            }
        }

        line_width += atom.width;
        line_atoms.push(atom);
    }

    if !line_atoms.is_empty() && max_lines.is_none_or(|max| line_count < max) {
        let line_height = line_atoms.iter().map(|a| a.height).fold(0.0f32, f32::max);
        let (min_x, max_x) =
            float_ctx.available_span(current_y, line_height, container_x, container_width);
        let (line_x, available_width) = get_line_x_and_width(min_x, max_x, is_first_line);

        let line_h = finalize_line(
            &line_atoms,
            line_x,
            available_width,
            current_y,
            text_align,
            &mut positioned_boxes,
            true,
        );
        current_y += line_h;
    }

    for sc in &source_children {
        let is_oof = sc.style.as_ref().is_some_and(|s| {
            matches!(
                s.position,
                mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
            )
        });
        if is_oof {
            positioned_boxes.push(sc.clone());
        }
    }

    let total_height = current_y - container_y;
    container.children = positioned_boxes;
    total_height
}

fn layout_vertical_inline_children(
    container: &mut LayoutBox,
    _float_ctx: &mut FloatContext,
    writing_mode: mango_css::values::WritingMode,
) -> f32 {
    let container_x = container.dimensions.content.x();
    let container_y = container.dimensions.content.y();
    let container_width = container.dimensions.content.width();
    let max_col_h = if container.dimensions.content.height() > 0.0 {
        container.dimensions.content.height()
    } else {
        400.0
    };

    let source_children = container
        .raw_children
        .get_or_insert_with(|| container.children.clone())
        .clone();

    let mut atoms: Vec<InlineAtom> = Vec::new();
    for child in &source_children {
        collect_inline_atoms(
            child,
            &mut atoms,
            child.link_target.as_deref(),
            container_width,
            max_col_h,
        );
    }

    if atoms.is_empty() {
        let mut oof_children = Vec::new();
        for sc in &source_children {
            let is_oof = sc.style.as_ref().is_some_and(|s| {
                matches!(
                    s.position,
                    mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                )
            });
            if is_oof {
                oof_children.push(sc.clone());
            }
        }
        container.children = oof_children;
        return 0.0;
    }

    struct VertLine {
        boxes: Vec<LayoutBox>,
        height: f32,
    }

    let mut lines: Vec<VertLine> = Vec::new();
    let mut cur_line_boxes: Vec<LayoutBox> = Vec::new();
    let mut cur_y = container_y;
    let base_col_w = container
        .style
        .as_ref()
        .map_or(24.0, |s| s.font_size * 1.5)
        .max(16.0);

    for atom in atoms {
        if matches!(atom.kind, InlineAtomKind::LineBreak) {
            lines.push(VertLine {
                boxes: std::mem::take(&mut cur_line_boxes),
                height: cur_y - container_y,
            });
            cur_y = container_y;
            continue;
        }

        match atom.kind {
            InlineAtomKind::Text { text, is_space } => {
                if is_space && cur_line_boxes.is_empty() {
                    continue;
                }
                let font_size = atom.style.font_size;
                let glyph_adv = (font_size * 1.2).ceil();

                for ch in text.chars() {
                    if cur_y + glyph_adv > container_y + max_col_h && !cur_line_boxes.is_empty() {
                        lines.push(VertLine {
                            boxes: std::mem::take(&mut cur_line_boxes),
                            height: cur_y - container_y,
                        });
                        cur_y = container_y;
                    }

                    let mut box_node =
                        LayoutBox::new(BoxType::TextNode(ch.to_string()), Some(atom.style.clone()));
                    box_node.link_target = atom.link_target.clone();
                    box_node.dimensions.content.origin = Point::new(0.0, cur_y);
                    box_node.dimensions.content.size = mango_core::Size::new(base_col_w, glyph_adv);
                    cur_line_boxes.push(box_node);
                    cur_y += glyph_adv;
                }
            }
            InlineAtomKind::AtomicBox(mut atomic_box) => {
                let adv = atomic_box.dimensions.margin_box().height().max(16.0);
                if cur_y + adv > container_y + max_col_h && !cur_line_boxes.is_empty() {
                    lines.push(VertLine {
                        boxes: std::mem::take(&mut cur_line_boxes),
                        height: cur_y - container_y,
                    });
                    cur_y = container_y;
                }
                atomic_box.dimensions.content.origin = Point::new(0.0, cur_y);
                cur_line_boxes.push(*atomic_box);
                cur_y += adv;
            }
            InlineAtomKind::Spacing => {
                cur_y += atom.width;
            }
            _ => {}
        }
    }

    if !cur_line_boxes.is_empty() {
        lines.push(VertLine {
            boxes: cur_line_boxes,
            height: cur_y - container_y,
        });
    }

    let num_lines = lines.len().max(1);
    let total_width = num_lines as f32 * base_col_w;
    let eff_container_w = if container_width > 0.0 {
        container_width
    } else {
        total_width
    };

    let mut positioned_boxes = Vec::new();
    let mut max_line_h = 0.0f32;

    for (line_idx, line) in lines.into_iter().enumerate() {
        max_line_h = max_line_h.max(line.height);
        let line_x = match writing_mode {
            mango_css::values::WritingMode::VerticalRl => {
                container_x + eff_container_w - (line_idx as f32 + 1.0) * base_col_w
            }
            _ => {
                // VerticalLr or fallback
                container_x + line_idx as f32 * base_col_w
            }
        };

        for mut b in line.boxes {
            b.dimensions.content.origin.x = line_x;
            positioned_boxes.push(b);
        }
    }

    if container.dimensions.content.size.width <= 0.0 {
        container.dimensions.content.size.width = total_width;
    }

    for sc in &source_children {
        let is_oof = sc.style.as_ref().is_some_and(|s| {
            matches!(
                s.position,
                mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
            )
        });
        if is_oof {
            positioned_boxes.push(sc.clone());
        }
    }

    container.children = positioned_boxes;
    max_line_h
}

fn collect_inline_atoms(
    box_node: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    link_target: Option<&str>,
    container_width: f32,
    container_height: f32,
) {
    let style = box_node.style.clone().unwrap_or_default();
    if style.float != mango_css::values::Float::None {
        return;
    }
    if matches!(
        style.position,
        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
    ) {
        return;
    }
    let font_size = style.font_size;
    let weight = font_weight_for(&style);
    let family = font_family_for(&style);
    let line_height = used_line_height(&style, family, weight);

    if box_node.tag_name.as_deref() == Some("br") {
        out.push(InlineAtom {
            kind: InlineAtomKind::LineBreak,
            style: style.clone(),
            width: 0.0,
            height: line_height,
            link_target: None,
        });
        return;
    }

    let current_link = box_node.link_target.as_deref().or(link_target);

    if box_node.tag_name.as_deref() == Some("wbr") {
        out.push(InlineAtom {
            kind: InlineAtomKind::WordBreakOpportunity,
            style: style.clone(),
            width: 0.0,
            height: 0.0,
            link_target: current_link.map(|s| s.to_string()),
        });
        return;
    }

    match &box_node.box_type {
        BoxType::TextNode(raw_text) => {
            let transformed_text = style.text_transform.apply(raw_text);
            let shaped_text = if transformed_text
                .chars()
                .any(crate::shaping::is_devanagari_char)
            {
                crate::shaping::shape_devanagari(&transformed_text)
            } else if transformed_text.chars().any(crate::shaping::is_thai_char) {
                crate::shaping::shape_thai(&transformed_text)
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
                    });
                } else if ch == '\n' {
                    push_word(std::mem::take(&mut current_word), out);
                    if preserve_newlines {
                        out.push(InlineAtom {
                            kind: InlineAtomKind::LineBreak,
                            style: style.clone(),
                            width: 0.0,
                            height: line_height,
                            link_target: current_link.map(|s| s.to_string()),
                        });
                    } else if out.last().map(|a| !a.is_space()).unwrap_or(true) {
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
                        });
                    } else if preserve_spaces || out.last().map(|a| !a.is_space()).unwrap_or(true) {
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
                    let is_syllable_boundary = if !current_word.is_empty() {
                        let prev = current_word.chars().last().unwrap();
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
                container_width,
                container_height,
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
                });
            }

            for child in &box_node.children {
                collect_inline_atoms(child, out, current_link, container_width, container_height);
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
            });
        }

        _ => {}
    }
}

fn collect_ruby_atoms(
    ruby_node: &LayoutBox,
    out: &mut Vec<InlineAtom>,
    current_link: Option<&str>,
    _container_width: f32,
    _container_height: f32,
) {
    let ruby_style = ruby_node.style.clone().unwrap_or_default();
    let font_size = ruby_style.font_size;

    // Collect base elements and rt elements
    let mut bases: Vec<(String, ComputedStyle)> = Vec::new();
    let mut rts: Vec<(String, ComputedStyle)> = Vec::new();

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
            let mut s = child.style.clone().unwrap_or_else(|| ruby_style.clone());
            if s.font_size >= font_size {
                s.font_size = (font_size * 0.5).max(8.0);
            }
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
    let pairs: Vec<(String, ComputedStyle, String, ComputedStyle)> = if bases.len() == rts.len() {
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
        });
    }
}

fn extract_text_content(node: &LayoutBox, out: &mut String) {
    match &node.box_type {
        BoxType::TextNode(t) => out.push_str(t),
        _ => {
            for child in &node.children {
                extract_text_content(child, out);
            }
        }
    }
}

fn finalize_line(
    line_atoms: &[InlineAtom],
    start_x: f32,
    available_width: f32,
    line_y: f32,
    text_align: TextAlign,
    out: &mut Vec<LayoutBox>,
    is_last_line: bool,
) -> f32 {
    if line_atoms.is_empty() {
        return 16.0;
    }

    // Strip trailing space from the line measurement and display (unless break-spaces)
    let effective_atoms = if line_atoms
        .last()
        .map(|a| a.is_space() && a.style.white_space != mango_css::values::WhiteSpace::BreakSpaces)
        .unwrap_or(false)
    {
        &line_atoms[..line_atoms.len() - 1]
    } else {
        line_atoms
    };

    let atoms_for_metrics = if effective_atoms.is_empty() {
        line_atoms
    } else {
        effective_atoms
    };

    let line_content_width: f32 = effective_atoms.iter().map(|a| a.width).sum();
    let min_line_height = atoms_for_metrics
        .iter()
        .map(|a| a.height)
        .fold(0.0f32, f32::max);

    // CSS 2.1 §10.8: Calculate unified baseline and total line box height across all inline boxes and text runs
    let mut max_above = 0.0f32;
    let mut max_below = 0.0f32;

    for atom in atoms_for_metrics {
        match &atom.kind {
            InlineAtomKind::Text { .. } => {
                let family = font_family_for(&atom.style);
                let weight = font_weight_for(&atom.style);
                let (ascent, descent, _) = mango_render::font::font_manager().font_metrics(
                    family,
                    weight,
                    atom.style.font_size,
                );
                let lh = used_line_height(&atom.style, family, weight);
                let natural_h = ascent + descent;
                let half_leading = (lh - natural_h) / 2.0;
                let above = half_leading + ascent;
                let below = half_leading + descent;
                max_above = max_above.max(above);
                max_below = max_below.max(below);
            }
            InlineAtomKind::AtomicBox(atomic_box) => {
                if atomic_box.tag_name.as_deref() == Some("ruby") {
                    let rt_h = atomic_box
                        .children
                        .first()
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(0.0);
                    let base_h = atomic_box
                        .children
                        .get(1)
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(atom.height - rt_h);
                    let base_ascent = base_h * 0.8;
                    let base_descent = base_h * 0.2;
                    max_above = max_above.max(rt_h + base_ascent);
                    max_below = max_below.max(base_descent);
                } else {
                    match atom.style.vertical_align {
                        mango_css::values::VerticalAlign::Baseline => {
                            max_above = max_above.max(atom.height);
                        }
                        mango_css::values::VerticalAlign::Middle => {
                            max_above = max_above.max(atom.height / 2.0);
                            max_below = max_below.max(atom.height / 2.0);
                        }
                        _ => {
                            max_above = max_above.max(atom.height);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if max_above + max_below <= 0.0 {
        max_above = min_line_height * 0.8;
        max_below = min_line_height * 0.2;
    }
    let baseline_from_line_top = max_above;
    let actual_line_height = (max_above + max_below).max(min_line_height).max(1.0);

    let is_rtl = atoms_for_metrics
        .first()
        .map(|a| a.style.direction == mango_css::values::Direction::Rtl)
        .unwrap_or(false);

    let mut space_expansion = 0.0f32;
    if text_align == TextAlign::Justify && !is_last_line {
        let free_space = (available_width - line_content_width).max(0.0);
        let space_count = effective_atoms.iter().filter(|a| a.is_space()).count();
        if space_count > 0 && free_space > 0.0 {
            space_expansion = free_space / space_count as f32;
        }
    }

    let offset_x = match text_align {
        TextAlign::Left => 0.0,
        TextAlign::Center => ((available_width - line_content_width) / 2.0).max(0.0),
        TextAlign::Right => (available_width - line_content_width).max(0.0),
        TextAlign::Justify => {
            if is_last_line && is_rtl {
                (available_width - line_content_width).max(0.0)
            } else {
                0.0
            }
        }
    };

    // Merge contiguous text atoms on this line that share identical style and link target.
    struct MergedRun {
        text: String,
        style: ComputedStyle,
        start_x: f32,
        width: f32,
        link_target: Option<String>,
        /// Font ascent, in pixels, at this run's font size.
        ascent: f32,
        /// Font descent (positive), in pixels, at this run's font size.
        descent: f32,
    }

    let mut runs: Vec<MergedRun> = Vec::new();
    let mut cursor_x = start_x + offset_x;

    let flush_text_runs = |runs: &mut Vec<MergedRun>, out: &mut Vec<LayoutBox>| {
        for mut run in runs.drain(..) {
            let is_rtl_dir = run.style.direction == mango_css::values::Direction::Rtl;
            let is_override = matches!(
                run.style.unicode_bidi,
                mango_css::values::UnicodeBidi::BidiOverride
                    | mango_css::values::UnicodeBidi::IsolateOverride
            );
            if is_override && is_rtl_dir {
                run.text = run.text.chars().rev().collect();
            } else if run.text.chars().any(is_rtl_char) || is_rtl_dir {
                run.text = shape_and_bidi_text_with_dir(&run.text, is_rtl_dir);
            }
            let ascent = run.ascent;
            let descent = run.descent;
            let natural_h = ascent + descent;
            // Top of this run's inline box: the line baseline minus its own ascent.
            let text_top = line_y + baseline_from_line_top - ascent;

            let run_y = match run.style.vertical_align {
                mango_css::values::VerticalAlign::Baseline => text_top,
                mango_css::values::VerticalAlign::Top => line_y,
                mango_css::values::VerticalAlign::Bottom => {
                    line_y + (actual_line_height - natural_h).max(0.0)
                }
                mango_css::values::VerticalAlign::Middle => {
                    line_y + baseline_from_line_top - (ascent * 0.5) - (natural_h * 0.5)
                }
                mango_css::values::VerticalAlign::Super => text_top - (ascent * 0.35),
                mango_css::values::VerticalAlign::Sub => text_top + (descent * 0.5),
                mango_css::values::VerticalAlign::TextTop => text_top,
                mango_css::values::VerticalAlign::TextBottom => {
                    line_y + (actual_line_height - natural_h).max(0.0)
                }
            };

            let mut box_node = LayoutBox::new(BoxType::TextNode(run.text), Some(run.style));
            box_node.dimensions = Dimensions {
                content: Rect::new(run.start_x, run_y, run.width, natural_h),
                padding: EdgeSizes::ZERO,
                border: EdgeSizes::ZERO,
                margin: EdgeSizes::ZERO,
            };
            box_node.link_target = run.link_target;
            out.push(box_node);
        }
    };

    for atom in effective_atoms {
        let atom_w = if atom.is_space() {
            atom.width + space_expansion
        } else {
            atom.width
        };

        match &atom.kind {
            InlineAtomKind::Text { text, .. } => {
                let can_merge = if let Some(last_run) = runs.last_mut() {
                    last_run.link_target == atom.link_target
                        && last_run.style.color == atom.style.color
                        && last_run.style.font_size == atom.style.font_size
                        && last_run.style.font_weight == atom.style.font_weight
                        && last_run.style.font_family == atom.style.font_family
                        && last_run.style.font_style == atom.style.font_style
                        && last_run.style.text_decoration == atom.style.text_decoration
                        && last_run.style.letter_spacing == atom.style.letter_spacing
                        && last_run.style.vertical_align == atom.style.vertical_align
                        && (last_run.style.word_spacing == mango_css::values::Length::Px(0.0)
                            && atom.style.word_spacing == mango_css::values::Length::Px(0.0))
                } else {
                    false
                };

                if can_merge {
                    let last_run = runs.last_mut().unwrap();
                    last_run.text.push_str(text);
                    last_run.width += atom_w;
                } else {
                    let family = font_family_for(&atom.style);
                    let weight = font_weight_for(&atom.style);
                    let (ascent, descent, _) = mango_render::font::font_manager().font_metrics(
                        family,
                        weight,
                        atom.style.font_size,
                    );
                    runs.push(MergedRun {
                        text: text.clone(),
                        start_x: cursor_x,
                        width: atom_w,
                        link_target: atom.link_target.clone(),
                        ascent,
                        descent,
                        style: atom.style.clone(),
                    });
                }
                cursor_x += atom_w;
            }
            InlineAtomKind::AtomicBox(atomic_box) => {
                flush_text_runs(&mut runs, out);

                let mut placed = (**atomic_box).clone();
                let box_outer_x = cursor_x;
                let box_outer_y = if atomic_box.tag_name.as_deref() == Some("ruby") {
                    let rt_h = atomic_box
                        .children
                        .first()
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(0.0);
                    let base_h = atomic_box
                        .children
                        .get(1)
                        .map(|c| c.dimensions.content.height())
                        .unwrap_or(atom.height - rt_h);
                    let base_ascent = base_h * 0.8;
                    line_y + baseline_from_line_top - (rt_h + base_ascent)
                } else {
                    match atom.style.vertical_align {
                        mango_css::values::VerticalAlign::Baseline => {
                            line_y + baseline_from_line_top - atom.height
                        }
                        mango_css::values::VerticalAlign::Top => line_y,
                        mango_css::values::VerticalAlign::Bottom => {
                            line_y + (actual_line_height - atom.height).max(0.0)
                        }
                        mango_css::values::VerticalAlign::Middle => {
                            line_y + baseline_from_line_top - (atom.height / 2.0)
                        }
                        mango_css::values::VerticalAlign::Super => {
                            line_y + baseline_from_line_top - atom.height - 4.0
                        }
                        mango_css::values::VerticalAlign::Sub => {
                            line_y + baseline_from_line_top - atom.height + 4.0
                        }
                        mango_css::values::VerticalAlign::TextTop => line_y,
                        mango_css::values::VerticalAlign::TextBottom => {
                            line_y + (actual_line_height - atom.height).max(0.0)
                        }
                    }
                };

                let new_content_x = box_outer_x
                    + placed.dimensions.margin.left
                    + placed.dimensions.border.left
                    + placed.dimensions.padding.left;
                let new_content_y = box_outer_y
                    + placed.dimensions.margin.top
                    + placed.dimensions.border.top
                    + placed.dimensions.padding.top;

                let dx = new_content_x - placed.dimensions.content.x();
                let dy = new_content_y - placed.dimensions.content.y();
                placed.dimensions.content.origin = Point::new(new_content_x, new_content_y);
                crate::block_flow::shift_descendants(&mut placed, dx, dy);

                if placed.link_target.is_none() && atom.link_target.is_some() {
                    placed.link_target = atom.link_target.clone();
                }

                out.push(placed);
                cursor_x += atom.width;
            }
            InlineAtomKind::Spacing => {
                flush_text_runs(&mut runs, out);
                cursor_x += atom.width;
            }
            InlineAtomKind::LineBreak => {
                flush_text_runs(&mut runs, out);
            }
            InlineAtomKind::WordBreakOpportunity | InlineAtomKind::SoftHyphen => {}
        }
    }

    flush_text_runs(&mut runs, out);
    actual_line_height
}

fn split_overflow_text(
    text: &str,
    style: &ComputedStyle,
    available_width: f32,
    height: f32,
    link_target: Option<&str>,
    container_width: f32,
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
        });
    }

    chunks
}

fn try_auto_hyphenate(
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

fn to_roman(mut n: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_line_height_defines_block_height() {
        // A single line of 40px text with `line-height: 40px` must produce a 40px
        // tall block. Chromium lets the glyphs overflow the line box rather than
        // growing the block, and matching that keeps stacked blocks aligned.
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

        let mut style = ComputedStyle::default();
        style.font_size = 40.0;
        style.font_family = "Arial, sans-serif".to_string();
        style.line_height = Some(40.0);

        container.children.push(LayoutBox::new(
            BoxType::TextNode("H1".to_string()),
            Some(style),
        ));

        let mut float_ctx = FloatContext::new();
        let height = layout_inline_children(&mut container, &mut float_ctx);
        assert!(
            (height - 40.0).abs() < 0.01,
            "line-height 40px must yield a 40px block, got {height}"
        );
    }

    #[test]
    fn test_negative_half_leading_not_clamped() {
        // `line-height` below the font's natural height produces negative half-leading,
        // which lifts the text above the line box (CSS 2.1 §10.8). Clamping it to zero
        // pushes tight-line-height text down by half the shortfall.
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

        let mut style = ComputedStyle::default();
        style.font_size = 40.0;
        style.font_family = "Arial, sans-serif".to_string();
        style.line_height = Some(40.0);

        container.children.push(LayoutBox::new(
            BoxType::TextNode("H1".to_string()),
            Some(style),
        ));

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        let run = &container.children[0];
        let text_top = run.dimensions.content.y();
        assert!(
            text_top < -1.0,
            "tight line-height must lift the run above the line box top, got {text_top}"
        );
    }

    #[test]
    fn test_normal_line_height_uses_font_metrics() {
        // `line-height: normal` is the font's own ascent + descent + line gap, not a
        // fixed 1.3x multiplier, which would make every wrapped paragraph taller
        // than Chromium's.
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

        let mut style = ComputedStyle::default();
        style.font_size = 16.0;
        style.font_family = "Arial, sans-serif".to_string();

        container.children.push(LayoutBox::new(
            BoxType::TextNode("one line".to_string()),
            Some(style),
        ));

        let mut float_ctx = FloatContext::new();
        let height = layout_inline_children(&mut container, &mut float_ctx);
        // Arial's normal line-height in Chromium is 18.4px at 16px (1.15em).
        assert!(
            (18.0..19.0).contains(&height),
            "normal line-height at 16px must be the font's own 1.15em (18.4px), got {height}"
        );
    }

    #[test]
    fn test_measure_text_width() {
        let w = measure_text_width("Hello", 16.0);
        assert!(
            w > 30.0 && w < 50.0,
            "proportional width for 'Hello' should be ~40px, got {w}"
        );
    }

    #[test]
    fn test_layout_inline_children_wrapping() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        // Container width 100px
        container.dimensions.content = Rect::new(0.0, 0.0, 100.0, 0.0);

        let mut style = ComputedStyle::default();
        style.font_size = 16.0;

        // "Hello World Mango" -> "Hello World" > 100px => wrapping must occur
        let text_child = LayoutBox::new(
            BoxType::TextNode("Hello World Mango".to_string()),
            Some(style),
        );
        container.children.push(text_child);

        let mut float_ctx = FloatContext::new();
        let total_h = layout_inline_children(&mut container, &mut float_ctx);

        // Must produce at least 2 lines of text
        assert!(total_h >= 32.0);
        assert!(
            container.children.len() >= 2,
            "Wrapped lines should produce boxes"
        );
    }

    #[test]
    fn test_heading_words_do_not_overlap() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 800.0, 0.0);

        let mut style = ComputedStyle::default();
        style.font_size = 32.0;
        style.font_weight = mango_css::values::FontWeight::Bold;

        let text_child = LayoutBox::new(
            BoxType::TextNode("Herman Melville - Moby-Dick".to_string()),
            Some(style),
        );
        container.children.push(text_child);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert_eq!(
            container.children.len(),
            1,
            "Single line with identical style should be merged into 1 text run"
        );
        if let BoxType::TextNode(t) = &container.children[0].box_type {
            assert_eq!(t, "Herman Melville - Moby-Dick");
            assert!(container.children[0].dimensions.content.width() > 300.0);
        } else {
            panic!("Expected TextNode");
        }
    }

    #[test]
    fn test_letter_spacing_expands_text_width() {
        let base_w = measure_text_width_with_style_and_spacing(
            "Hello",
            16.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
            0.0,
        );
        let spaced_w = measure_text_width_with_style_and_spacing(
            "Hello",
            16.0,
            mango_render::FontWeight::Regular,
            mango_render::FontFamily::SansSerif,
            2.0,
        );
        assert!(
            (spaced_w - base_w - 10.0).abs() < 0.1,
            "5 characters * 2px spacing should add 10px"
        );
    }

    #[test]
    fn test_text_indent_offsets_first_line() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 120.0, 0.0);

        let mut container_style = ComputedStyle::default();
        container_style.text_indent = mango_css::values::Length::Px(25.0);
        container.style = Some(container_style);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;

        // "First line wrapping into second line"
        let text_child = LayoutBox::new(
            BoxType::TextNode("First long line that wraps".to_string()),
            Some(child_style),
        );
        container.children.push(text_child);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() >= 2,
            "Should wrap into at least 2 lines"
        );
        // First line must start at x >= 25.0
        assert_eq!(container.children[0].dimensions.content.x(), 25.0);
        // Subsequent line must start at x == 0.0
        assert_eq!(container.children[1].dimensions.content.x(), 0.0);
    }

    #[test]
    fn test_vertical_align_super_and_middle() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 500.0, 0.0);

        let mut normal_style = ComputedStyle::default();
        normal_style.font_size = 16.0;
        let normal_child = LayoutBox::new(
            BoxType::TextNode("Normal text".to_string()),
            Some(normal_style),
        );

        let mut super_style = ComputedStyle::default();
        super_style.font_size = 12.0;
        super_style.vertical_align = mango_css::values::VerticalAlign::Super;
        let super_child = LayoutBox::new(BoxType::TextNode("[1]".to_string()), Some(super_style));

        container.children.push(normal_child);
        container.children.push(super_child);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert_eq!(
            container.children.len(),
            2,
            "Normal and super text runs should remain separate"
        );
        let normal_y = container.children[0].dimensions.content.y();
        let super_y = container.children[1].dimensions.content.y();
        assert!(
            super_y < normal_y,
            "Superscript text must be raised above normal baseline (super_y={super_y} < normal_y={normal_y})"
        );
    }

    #[test]
    fn test_word_break_opportunity_wbr_and_zero_width_space() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        // Container width narrow enough to force breaking at wbr
        container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;

        // Long unbroken word with <wbr> in the middle
        let text1 = LayoutBox::new(
            BoxType::TextNode("Super".to_string()),
            Some(child_style.clone()),
        );
        let mut wbr_box = LayoutBox::new(BoxType::InlineNode, Some(child_style.clone()));
        wbr_box.tag_name = Some("wbr".to_string());
        let text2 = LayoutBox::new(
            BoxType::TextNode("califragilistic".to_string()),
            Some(child_style.clone()),
        );

        container.children.push(text1);
        container.children.push(wbr_box);
        container.children.push(text2);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        // Due to narrow width (60px), it should break at the <wbr> into 2 lines!
        assert!(
            container.children.len() >= 2,
            "Long word with wbr should wrap across lines"
        );
        assert_eq!(container.children[0].text(), Some("Super"));

        // Test with \u{200B} inside a single TextNode
        let mut container2 = LayoutBox::new(BoxType::BlockNode, None);
        container2.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);
        let text_zwsp = LayoutBox::new(
            BoxType::TextNode("Zero\u{200B}Width".to_string()),
            Some(child_style),
        );
        container2.children.push(text_zwsp);
        layout_inline_children(&mut container2, &mut float_ctx);
        assert!(
            container2.children.len() >= 2,
            "Text with U+200B should wrap across lines"
        );
        assert_eq!(container2.children[0].text(), Some("Zero"));
    }

    #[test]
    fn test_bidi_override_visual_reversal() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 0.0);

        let mut bdo_style = ComputedStyle::default();
        bdo_style.font_size = 16.0;
        bdo_style.direction = mango_css::values::Direction::Rtl;
        bdo_style.unicode_bidi = mango_css::values::UnicodeBidi::BidiOverride;

        let bdo_text = LayoutBox::new(BoxType::TextNode("Hello".to_string()), Some(bdo_style));
        container.children.push(bdo_text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert_eq!(container.children.len(), 1);
        // Characters must be reversed visually: "Hello" -> "olleH"
        assert_eq!(container.children[0].text(), Some("olleH"));
    }

    #[test]
    fn test_word_break_break_all() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 45.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;
        child_style.word_break = mango_css::values::WordBreak::BreakAll;

        let text = LayoutBox::new(
            BoxType::TextNode("Supercalifragilistic".to_string()),
            Some(child_style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() > 1,
            "word-break: break-all must break word across lines"
        );
    }

    #[test]
    fn test_overflow_wrap_break_word() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;
        child_style.overflow_wrap = mango_css::values::OverflowWrap::BreakWord;

        let text = LayoutBox::new(
            BoxType::TextNode("UnbreakableLongWordThatMustWrap".to_string()),
            Some(child_style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() > 1,
            "overflow-wrap: break-word must split unbroken word"
        );
    }

    #[test]
    fn test_hyphens_auto() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 80.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;
        child_style.hyphens = mango_css::values::Hyphens::Auto;

        let text = LayoutBox::new(
            BoxType::TextNode("Internationalization".to_string()),
            Some(child_style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() > 1,
            "hyphens: auto must hyphenate long word across lines"
        );
        let first_text = container.children[0].text().unwrap();
        assert!(
            first_text.ends_with('-'),
            "hyphenated prefix must end with hyphen '-': got {}",
            first_text
        );
    }

    #[test]
    fn test_soft_hyphen() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 70.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;
        child_style.hyphens = mango_css::values::Hyphens::Manual;

        let text = LayoutBox::new(
            BoxType::TextNode("hy\u{00AD}phen\u{00AD}ation".to_string()),
            Some(child_style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() > 1,
            "Soft hyphen must wrap word across lines"
        );
        let first_text = container.children[0].text().unwrap();
        assert!(
            first_text.ends_with('-'),
            "Soft hyphen break must insert visible hyphen: got {}",
            first_text
        );
    }

    #[test]
    fn test_line_clamp() {
        let mut container_style = ComputedStyle::default();
        container_style.line_clamp = mango_css::values::LineClamp::Lines(2);
        let mut container = LayoutBox::new(BoxType::BlockNode, Some(container_style));
        container.dimensions.content = Rect::new(0.0, 0.0, 100.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;
        child_style.line_clamp = mango_css::values::LineClamp::Lines(2);

        let text = LayoutBox::new(
            BoxType::TextNode("Line one text that wraps easily. Line two text that continues. Line three text that should be clamped.".to_string()),
            Some(child_style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() <= 2,
            "line-clamp: 2 must clamp children to at most 2 boxes"
        );
    }

    #[test]
    fn test_rust_text_layout() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 360.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 18.0;

        let raw = "Rust’s rich type system and ownership model guarantee memory-safety\nand thread-safety — enabling you to eliminate many classes of\nbugs at compile-time.";
        let collapsed = crate::style_tree::collapse_whitespace(raw);
        let text = LayoutBox::new(BoxType::TextNode(collapsed), Some(child_style));
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        for (i, child) in container.children.iter().enumerate() {
            println!("Line {}: {:?}", i, child.text());
        }
    }

    #[test]
    fn test_debug_spaces() {
        let html = "<p>Predictable performance. Tiny resource footprint. Rock-solid reliability. Rust is great for network services.</p>";
        let doc = mango_html::parse_html(html);
        let st = crate::style_tree::build_style_tree(&doc, &[]).unwrap();
        let mut bt = crate::box_tree::build_box_tree(&st);
        let mut float_ctx = FloatContext::new();
        let cb = Dimensions::new(Rect::new(0.0, 0.0, 237.0, 0.0));
        crate::block_flow::layout_block(&mut bt, &cb, &mut float_ctx);
        // Call a second time to ensure multi-pass layout doesn't merge words across line boxes:
        let mut float_ctx2 = FloatContext::new();
        crate::block_flow::layout_block(&mut bt, &cb, &mut float_ctx2);

        fn collect_all_text(b: &LayoutBox, out: &mut Vec<String>) {
            if let Some(t) = b.text() {
                out.push(t.to_string());
            }
            for c in &b.children {
                collect_all_text(c, out);
            }
        }
        let mut texts = Vec::new();
        collect_all_text(&bt, &mut texts);
        let full_text = texts.join(" ");
        assert!(
            !full_text.contains("Tinyresource"),
            "Tiny and resource merged: {}",
            full_text
        );
        assert!(
            !full_text.contains("Rock-solidreliability"),
            "Rock-solid and reliability merged: {}",
            full_text
        );
        assert!(
            !full_text.contains("fornetwork"),
            "for and network merged: {}",
            full_text
        );
    }

    #[test]
    fn test_cjk_line_wrapping() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        // 60px container fits at most ~3-4 CJK characters per line at 16px
        container.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);

        let mut child_style = ComputedStyle::default();
        child_style.font_size = 16.0;

        let raw = "这是一个测试，包含中文。";
        let text = LayoutBox::new(
            BoxType::TextNode(raw.to_string()),
            Some(child_style.clone()),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        // Must wrap across multiple lines because CJK allows character-level breaking
        assert!(
            container.children.len() >= 3,
            "CJK text without spaces must wrap into multiple lines in narrow container; got {} lines",
            container.children.len()
        );

        // Kinsoku Shori: verify closing punctuation does not start any line
        for (i, child) in container.children.iter().enumerate() {
            if let Some(t) = child.text() {
                assert!(
                    !t.starts_with('，') && !t.starts_with('。'),
                    "Line {} starts with closing punctuation: {:?}",
                    i,
                    t
                );
            }
        }

        // Now test word-break: keep-all (CJK should NOT break without spaces or punctuation)
        let mut container_keep = LayoutBox::new(BoxType::BlockNode, None);
        container_keep.dimensions.content = Rect::new(0.0, 0.0, 60.0, 0.0);
        let mut keep_style = ComputedStyle::default();
        keep_style.font_size = 16.0;
        keep_style.word_break = mango_css::values::WordBreak::KeepAll;

        let raw_unbroken = "这是一个测试包含中文";
        let text_keep = LayoutBox::new(
            BoxType::TextNode(raw_unbroken.to_string()),
            Some(keep_style),
        );
        container_keep.children.push(text_keep);

        let mut float_ctx2 = FloatContext::new();
        layout_inline_children(&mut container_keep, &mut float_ctx2);
        assert_eq!(
            container_keep.children.len(),
            1,
            "word-break: keep-all must keep unbroken CJK text on a single line"
        );

        // Verify Thai combining marks (like Mai Ek '\u{0E48}' and Sara I '\u{0E34}') stay attached to consonants
        let mut container_thai = LayoutBox::new(BoxType::BlockNode, None);
        container_thai.dimensions.content = Rect::new(0.0, 0.0, 30.0, 0.0);
        let thai_raw = "ที่นี่"; // "thi-ni" (at this place): Consonant 'ท' + Sara I + Mai Ek, Consonant 'น' + Sara I + Mai Ek
        let text_thai = LayoutBox::new(BoxType::TextNode(thai_raw.to_string()), Some(child_style));
        container_thai.children.push(text_thai);
        let mut float_ctx3 = FloatContext::new();
        layout_inline_children(&mut container_thai, &mut float_ctx3);
        for child in &container_thai.children {
            if let Some(t) = child.text() {
                assert!(
                    !t.starts_with('\u{0E34}') && !t.starts_with('\u{0E48}'),
                    "Combining mark cannot start a line by itself: {:?}",
                    t
                );
            }
        }
    }

    #[test]
    fn test_rtl_shaping_and_bidi() {
        // Test Arabic cursive shaping and Lam-Alef ligature for "سلام" (Seen, Lam, Alef, Meem)
        let salam = "سلام";
        let shaped_salam = shape_and_bidi_text(salam);
        // Must contain Final Lam-Alef ligature (\u{FEFC}) and Initial Seen (\u{FEB3})
        assert!(
            shaped_salam.contains('\u{FEFC}'),
            "Shaped 'سلام' must contain Lam-Alef ligature \\u{{FEFC}}, got: {:?}",
            shaped_salam
        );
        assert!(
            shaped_salam.contains('\u{FEB3}'),
            "Shaped 'سلام' must contain Initial Seen \\u{{FEB3}}, got: {:?}",
            shaped_salam
        );

        // Test multi-word Arabic Bidi ordering: "مرحبا بكم"
        let phrase = "مرحبا بكم";
        let shaped_phrase = shape_and_bidi_text(phrase);
        // In visual LTR blitting, the second word "بكم" is drawn on the left, and "مرحبا" on the right!
        let words: Vec<&str> = shaped_phrase.split_whitespace().collect();
        assert_eq!(words.len(), 2, "Shaped phrase should have 2 words");

        // Verify both visual words are Arabic
        assert!(
            words[0].chars().any(is_rtl_char),
            "Visual word 0 should be Arabic"
        );
        assert!(
            words[1].chars().any(is_rtl_char),
            "Visual word 1 should be Arabic"
        );

        // Test mixed LTR + RTL text: "Hello سلام World"
        let mixed = "Hello سلام World";
        let shaped_mixed = shape_and_bidi_text(mixed);
        assert!(
            shaped_mixed.starts_with("Hello "),
            "Mixed text must start with LTR 'Hello '"
        );
        assert!(
            shaped_mixed.ends_with(" World"),
            "Mixed text must end with LTR ' World'"
        );
        assert!(
            shaped_mixed.contains('\u{FEFC}'),
            "Mixed text must contain shaped Arabic ligature"
        );
    }

    #[test]
    fn test_text_align_justify_space_expansion() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 200.0, 0.0);
        let mut style = ComputedStyle::default();
        style.font_size = 16.0;
        style.text_align = TextAlign::Justify;
        container.style = Some(style.clone());

        // "Hello world from mango browser engine" - long enough to wrap across 2 lines
        let text = LayoutBox::new(
            BoxType::TextNode("Hello world from mango browser engine".to_string()),
            Some(style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(
            container.children.len() >= 2,
            "Expected multiple lines, got {}",
            container.children.len()
        );
        // The first line should be justified: start at 0.0 and span the full container width
        let first_line = &container.children[0];
        assert_eq!(first_line.dimensions.content.origin.x, 0.0);
        let first_line_w = first_line.dimensions.content.width();
        assert!(
            (first_line_w - 200.0).abs() < 5.0,
            "First line of justified text should span ~200px, got {}",
            first_line_w
        );
    }

    #[test]
    fn test_inline_node_horizontal_spacing() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);
        let mut cont_style = ComputedStyle::default();
        cont_style.font_size = 16.0;
        container.style = Some(cont_style.clone());

        // Inline span with margin-left: 20px, padding-left: 10px
        let mut span_style = ComputedStyle::default();
        span_style.font_size = 16.0;
        span_style.margin_left = mango_css::values::Length::Px(20.0);
        span_style.padding_left = mango_css::values::Length::Px(10.0);

        let mut span = LayoutBox::new(BoxType::InlineNode, Some(span_style));
        let text = LayoutBox::new(
            BoxType::TextNode("Inside Span".to_string()),
            Some(cont_style.clone()),
        );
        span.children.push(text);
        container.children.push(span);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert!(!container.children.is_empty());
        let child_x = container.children[0].dimensions.content.origin.x;
        assert!(
            (child_x - 30.0).abs() < 0.1,
            "Text inside span should be shifted by 30px (margin + padding), got {}",
            child_x
        );
    }

    #[test]
    fn test_whitespace_only_line_height_not_collapsed() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 400.0, 0.0);
        let mut style = ComputedStyle::default();
        style.font_size = 20.0;
        style.white_space = mango_css::values::WhiteSpace::Pre;
        container.style = Some(style.clone());

        // TextNode containing only a space
        let text = LayoutBox::new(BoxType::TextNode("   ".to_string()), Some(style));
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        let h = layout_inline_children(&mut container, &mut float_ctx);

        // Height should be based on font metrics (~24px for 20px font), not collapsed to 1.0px
        assert!(
            h >= 16.0,
            "Whitespace line should retain font line-height, got {}",
            h
        );
    }

    #[test]
    fn test_rtl_text_indent_start_edge() {
        let mut container = LayoutBox::new(BoxType::BlockNode, None);
        container.dimensions.content = Rect::new(0.0, 0.0, 300.0, 0.0);
        let mut style = ComputedStyle::default();
        style.font_size = 16.0;
        style.direction = mango_css::values::Direction::Rtl;
        style.text_indent = mango_css::values::Length::Px(40.0);
        container.style = Some(style.clone());

        let text = LayoutBox::new(
            BoxType::TextNode("مرحبا".to_string()),
            Some(style),
        );
        container.children.push(text);

        let mut float_ctx = FloatContext::new();
        layout_inline_children(&mut container, &mut float_ctx);

        assert_eq!(container.children.len(), 1);
        let child = &container.children[0];
        let child_right = child.dimensions.content.origin.x + child.dimensions.content.width();
        // In 300px container with 40px text-indent in RTL, right edge should be at 300 - 40 = 260px
        assert!(
            (child_right - 260.0).abs() < 1.0,
            "RTL text with text-indent: 40px should end at 260px, got {}",
            child_right
        );
    }
}
