//! Parsed `@keyframes` rules and style difference detection.

use std::collections::HashMap;

use super::interpolation::interpolate_value;
use crate::computed::ComputedStyle;
use crate::parser::{KeyframesRule, Rule, Stylesheet};
use crate::properties::Declaration;
use crate::values::Value;

/// A `@keyframes` rule with offsets normalized to the `0.0..=1.0` range.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Keyframes {
    pub name: String,
    /// Keyframes sorted by ascending offset; each holds property → declared value.
    pub frames: Vec<Keyframe>,
}

/// One stop within a [`Keyframes`] rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe {
    /// Normalized offset in `0.0..=1.0`.
    pub offset: f32,
    /// Declared properties at this offset (longhands only, `!important` dropped).
    pub declarations: HashMap<String, Value>,
}

impl Keyframes {
    /// Builds a [`Keyframes`] value from a parsed `@keyframes` rule.
    pub fn from_rule(rule: &KeyframesRule) -> Self {
        let mut frames: Vec<Keyframe> = Vec::new();
        for kf in &rule.keyframes {
            let mut declarations: HashMap<String, Value> = HashMap::new();
            for decl in &kf.declarations {
                // CSS Animations 1 § 3.2: Declarations in keyframe rules cannot be
                // marked !important. A declaration marked !important is ignored.
                if decl.important {
                    continue;
                }
                for expanded in
                    Declaration::new(decl.name.clone(), decl.value.clone(), decl.important)
                        .expand_shorthand()
                {
                    declarations.insert(expanded.name.clone(), expanded.value.clone());
                }
            }
            for offset_pct in &kf.offsets {
                frames.push(Keyframe {
                    offset: (offset_pct / 100.0).clamp(0.0, 1.0),
                    declarations: declarations.clone(),
                });
            }
        }
        frames.sort_by(|a, b| {
            a.offset
                .partial_cmp(&b.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Self {
            name: rule.name.clone(),
            frames,
        }
    }

    /// Samples this rule at `progress` (0.0..=1.0), returning interpolated
    /// declared values keyed by property name.
    pub fn sample(&self, progress: f32) -> HashMap<String, Value> {
        self.sample_with_underlying(progress, None)
    }

    /// Samples this rule at `progress` (0.0..=1.0) with underlying styles for
    /// resolving CSS-wide keywords like `revert-layer` and `revert`.
    pub fn sample_with_underlying(
        &self,
        progress: f32,
        underlying: Option<&HashMap<String, Value>>,
    ) -> HashMap<String, Value> {
        let resolve_revert = |prop: &str, val: &Value| -> Value {
            if let Value::Keyword(k) = val {
                if k.eq_ignore_ascii_case("revert-layer") || k.eq_ignore_ascii_case("revert") {
                    if let Some(und) = underlying {
                        if let Some(v) = und.get(prop) {
                            return v.clone();
                        }
                    }
                }
            }
            val.clone()
        };

        let progress = progress.clamp(0.0, 1.0);
        if self.frames.is_empty() {
            return HashMap::new();
        }

        let before = self
            .frames
            .iter()
            .rfind(|f| f.offset <= progress)
            .unwrap_or(&self.frames[0]);
        let after = self
            .frames
            .iter()
            .find(|f| f.offset >= progress)
            .or_else(|| self.frames.last())
            .unwrap_or(&self.frames[0]);

        if (after.offset - before.offset).abs() < f32::EPSILON {
            let mut out = HashMap::new();
            for (prop, val) in &before.declarations {
                out.insert(prop.clone(), resolve_revert(prop, val));
            }
            return out;
        }
        let span = after.offset - before.offset;
        let t = ((progress - before.offset) / span).clamp(0.0, 1.0);

        let mut out: HashMap<String, Value> = HashMap::new();
        for (prop, raw_a) in &before.declarations {
            let a = resolve_revert(prop, raw_a);
            match after.declarations.get(prop) {
                Some(raw_b) => {
                    let b = resolve_revert(prop, raw_b);
                    if let Some(v) = interpolate_value(&a, &b, t) {
                        out.insert(prop.clone(), v);
                    } else if t < 0.5 {
                        out.insert(prop.clone(), a);
                    } else {
                        out.insert(prop.clone(), b);
                    }
                }
                None => {
                    out.insert(prop.clone(), a);
                }
            }
        }
        for (prop, raw_b) in &after.declarations {
            if !out.contains_key(prop) {
                out.insert(prop.clone(), resolve_revert(prop, raw_b));
            }
        }
        out
    }
}

/// Collects every `@keyframes` rule from a set of stylesheets into a lookup map.
///
/// Later definitions win, matching the CSS cascade for `@keyframes`.
pub fn collect_keyframes<'a, I>(sheets: I) -> HashMap<String, Keyframes>
where
    I: IntoIterator<Item = &'a Stylesheet>,
{
    let mut map: HashMap<String, Keyframes> = HashMap::new();
    for sheet in sheets {
        collect_from_rules(&sheet.rules, &mut map);
    }
    map
}

fn collect_from_rules(rules: &[Rule], map: &mut HashMap<String, Keyframes>) {
    for rule in rules {
        if let Rule::Keyframes(kf) = rule {
            map.insert(kf.name.clone(), Keyframes::from_rule(kf));
        }
    }
}

/// Returns true when `prop`'s value differs between the two computed styles.
pub fn property_differs(a: &ComputedStyle, b: &ComputedStyle, prop: &str) -> bool {
    match prop {
        "opacity" => a.opacity != b.opacity,
        "transform" => a.transform != b.transform,
        "color" => a.color != b.color,
        "background-color" => a.background_color != b.background_color,
        "border-color" => {
            a.border_top_color != b.border_top_color
                || a.border_right_color != b.border_right_color
                || a.border_bottom_color != b.border_bottom_color
                || a.border_left_color != b.border_left_color
        }
        "width" => a.width != b.width,
        "height" => a.height != b.height,
        "min-width" => a.min_width != b.min_width,
        "min-height" => a.min_height != b.min_height,
        "max-width" => a.max_width != b.max_width,
        "max-height" => a.max_height != b.max_height,
        "margin" => {
            a.margin_top != b.margin_top
                || a.margin_right != b.margin_right
                || a.margin_bottom != b.margin_bottom
                || a.margin_left != b.margin_left
        }
        "padding" => {
            a.padding_top != b.padding_top
                || a.padding_right != b.padding_right
                || a.padding_bottom != b.padding_bottom
                || a.padding_left != b.padding_left
        }
        "top" => a.top != b.top,
        "right" => a.right != b.right,
        "bottom" => a.bottom != b.bottom,
        "left" => a.left != b.left,
        "font-size" => a.font_size != b.font_size,
        "letter-spacing" => a.letter_spacing != b.letter_spacing,
        "word-spacing" => a.word_spacing != b.word_spacing,
        "text-indent" => a.text_indent != b.text_indent,
        "border-width" => {
            a.border_top_width != b.border_top_width
                || a.border_right_width != b.border_right_width
                || a.border_bottom_width != b.border_bottom_width
                || a.border_left_width != b.border_left_width
        }
        "border-radius" => a.border_radius() != b.border_radius(),
        "box-shadow" => a.box_shadow != b.box_shadow,
        "text-shadow" => a.text_shadow != b.text_shadow,
        "filter" => a.filter != b.filter,
        "flex-basis" => a.flex_basis != b.flex_basis,
        "flex-grow" => a.flex_grow != b.flex_grow,
        "flex-shrink" => a.flex_shrink != b.flex_shrink,
        "gap" => a.row_gap != b.row_gap || a.column_gap != b.column_gap,
        "outline-width" => a.outline_width != b.outline_width,
        "outline-offset" => a.outline_offset != b.outline_offset,
        "aspect-ratio" => a.aspect_ratio != b.aspect_ratio,
        _ => false,
    }
}

/// Convenience: builds the display-time animation name list for a style.
pub fn animation_names(style: &ComputedStyle) -> Vec<String> {
    style.animations.iter().map(|a| a.name.clone()).collect()
}

/// Returns true when this style declares any transition that could animate.
pub fn has_transitions(style: &ComputedStyle) -> bool {
    style
        .transitions
        .iter()
        .any(|t| t.duration_ms > 0.0 || t.delay_ms > 0.0)
}

/// Returns true when the style declares any runnable animation.
pub fn has_animations(style: &ComputedStyle) -> bool {
    style
        .animations
        .iter()
        .any(|a| !a.name.is_empty() && !a.name.eq_ignore_ascii_case("none"))
}
