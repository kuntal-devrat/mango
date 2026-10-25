//! CSS Length dimension type and viewport metrics.

use super::math::{AnchorSide, CalcLength, MathId, eval_math, intern_anchor_name};

/// A CSS length or dimension value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Exact length in pixels (e.g. `16px`).
    Px(f32),
    /// Relative to current element's font size (e.g. `1.5em`).
    Em(f32),
    /// Relative to element's font x-height (e.g. `2ex`).
    Ex(f32),
    /// Relative to root element's font size (e.g. `2rem`).
    Rem(f32),
    /// Relative to containing block dimension (e.g. `50%`).
    Percent(f32),
    /// Relative to viewport width (e.g. `60vw` = 60% of viewport width).
    Vw(f32),
    /// Relative to viewport height (e.g. `15vh` = 15% of viewport height).
    Vh(f32),
    /// Relative to 1% of viewport minimum dimension (min(vw, vh)).
    Vmin(f32),
    /// Relative to 1% of viewport maximum dimension (max(vw, vh)).
    Vmax(f32),
    /// Relative to 1% of query container width.
    Cqw(f32),
    /// Relative to 1% of query container height.
    Cqh(f32),
    /// Relative to 1% of query container inline size.
    Cqi(f32),
    /// Relative to 1% of query container block size.
    Cqb(f32),
    /// Relative to smaller of query container inline/block size.
    Cqmin(f32),
    /// Relative to larger of query container inline/block size.
    Cqmax(f32),
    /// A computed mathematical length resulting from a `calc(...)` expression.
    Calc(CalcLength),
    /// A dynamic mathematical expression (min, max, clamp, or non-linear calc).
    Math(MathId),
    /// Keyword: content (e.g. `flex-basis: content`).
    Content,
    /// Keyword: min-content.
    MinContent,
    /// Keyword: max-content.
    MaxContent,
    /// Keyword: fit-content.
    FitContent,
    /// CSS Anchor Positioning `anchor()` expression (e.g. `anchor(bottom)` or `anchor(--anchor top)`).
    Anchor {
        name_id: Option<u32>,
        side: AnchorSide,
        offset: f32,
    },
    /// Automatic sizing by the layout engine.
    #[default]
    Auto,
}

impl Eq for Length {}

impl std::hash::Hash for Length {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match *self {
            Length::Px(v) => {
                0u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Em(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Ex(v) => {
                254u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Rem(v) => {
                2u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Percent(v) => {
                3u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Vw(v) => {
                4u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Vh(v) => {
                5u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Vmin(v) => {
                6u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Vmax(v) => {
                7u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqw(v) => {
                8u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqh(v) => {
                9u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqi(v) => {
                10u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqb(v) => {
                11u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqmin(v) => {
                12u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Cqmax(v) => {
                13u8.hash(state);
                v.to_bits().hash(state);
            }
            Length::Calc(c) => {
                14u8.hash(state);
                c.hash(state);
            }
            Length::Math(id) => {
                15u8.hash(state);
                id.hash(state);
            }
            Length::Content => 16u8.hash(state),
            Length::MinContent => 17u8.hash(state),
            Length::MaxContent => 18u8.hash(state),
            Length::FitContent => 19u8.hash(state),
            Length::Anchor {
                name_id,
                side,
                offset,
            } => {
                20u8.hash(state);
                name_id.hash(state);
                side.hash(state);
                offset.to_bits().hash(state);
            }
            Length::Auto => 21u8.hash(state),
        }
    }
}

thread_local! {
    static CURRENT_VIEWPORT: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((800.0, 600.0)) };
}

/// Sets the active viewport width and height used for resolving viewport units (`vw`, `vh`, etc.).
///
/// Uses thread-local storage so that concurrent layouts (e.g. multiple tabs or parallel tests)
/// maintain isolated viewport contexts without race conditions.
pub fn set_current_viewport(width: f32, height: f32) {
    CURRENT_VIEWPORT.with(|v| v.set((width, height)));
}

/// Returns the active viewport width and height for the current thread.
pub fn get_current_viewport() -> (f32, f32) {
    CURRENT_VIEWPORT.with(|v| v.get())
}

impl Length {
    pub const ZERO: Length = Length::Px(0.0);

    /// Parses a CSS length string (e.g. `12px`, `1.5em`, `2rem`, `50%`, `auto`, `anchor(...)`, `min(...)`, `clamp(...)`).
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("calc(")
            || lower.starts_with("min(")
            || lower.starts_with("max(")
            || lower.starts_with("clamp(")
            || lower.starts_with("round(")
            || lower.starts_with("mod(")
            || lower.starts_with("rem(")
            || lower.starts_with("abs(")
            || lower.starts_with("sign(")
            || lower.starts_with("sin(")
            || lower.starts_with("cos(")
            || lower.starts_with("tan(")
            || lower.starts_with("asin(")
            || lower.starts_with("acos(")
            || lower.starts_with("atan(")
            || lower.starts_with("atan2(")
            || lower.starts_with("sqrt(")
            || lower.starts_with("pow(")
            || lower.starts_with("hypot(")
        {
            return crate::parser::parse_math_length_from_str(trimmed);
        }
        if lower.starts_with("anchor(") && lower.ends_with(')') {
            let inner = trimmed[7..trimmed.len() - 1].trim();
            let mut parts = inner.split(',');
            let anchor_spec = parts.next().unwrap_or("").trim();
            let fallback_str = parts.next().map(|s| s.trim());
            let offset = fallback_str
                .and_then(|f| f.strip_suffix("px"))
                .and_then(|n| n.trim().parse::<f32>().ok())
                .unwrap_or(0.0);

            let tokens: Vec<&str> = anchor_spec.split_whitespace().collect();
            if tokens.len() == 1 {
                if let Some(side) = AnchorSide::parse(tokens[0]) {
                    return Some(Length::Anchor {
                        name_id: None,
                        side,
                        offset,
                    });
                }
            } else if tokens.len() >= 2 {
                let name = tokens[0];
                if let Some(side) = AnchorSide::parse(tokens[1]) {
                    let id = intern_anchor_name(name);
                    return Some(Length::Anchor {
                        name_id: Some(id),
                        side,
                        offset,
                    });
                }
            }
        }
        if trimmed.eq_ignore_ascii_case("auto") {
            return Some(Length::Auto);
        }
        if trimmed.eq_ignore_ascii_case("content") {
            return Some(Length::Content);
        }
        if trimmed.eq_ignore_ascii_case("min-content") {
            return Some(Length::MinContent);
        }
        if trimmed.eq_ignore_ascii_case("max-content") {
            return Some(Length::MaxContent);
        }
        if trimmed.eq_ignore_ascii_case("fit-content") {
            return Some(Length::FitContent);
        }
        if let Some(num) = trimmed.strip_suffix("px") {
            num.trim().parse::<f32>().ok().map(Length::Px)
        } else if let Some(num) = trimmed.strip_suffix("rem") {
            // Must check `rem` before `em` — "1rem".strip_suffix("em") would
            // incorrectly match, yielding "1r" which fails to parse.
            num.trim().parse::<f32>().ok().map(Length::Rem)
        } else if let Some(num) = trimmed.strip_suffix("em") {
            num.trim().parse::<f32>().ok().map(Length::Em)
        } else if let Some(num) = trimmed.strip_suffix('%') {
            num.trim().parse::<f32>().ok().map(Length::Percent)
        } else if let Some(num) = trimmed.strip_suffix("vmin") {
            // Must check `vmin`/`vmax` before `vw`/`vh` for the same suffix reason.
            num.trim().parse::<f32>().ok().map(Length::Vmin)
        } else if let Some(num) = trimmed.strip_suffix("vmax") {
            num.trim().parse::<f32>().ok().map(Length::Vmax)
        } else if let Some(num) = trimmed.strip_suffix("vw") {
            num.trim().parse::<f32>().ok().map(Length::Vw)
        } else if let Some(num) = trimmed.strip_suffix("vh") {
            num.trim().parse::<f32>().ok().map(Length::Vh)
        } else if let Some(num) = trimmed.strip_suffix("cqmin") {
            num.trim().parse::<f32>().ok().map(Length::Cqmin)
        } else if let Some(num) = trimmed.strip_suffix("cqmax") {
            num.trim().parse::<f32>().ok().map(Length::Cqmax)
        } else if let Some(num) = trimmed.strip_suffix("cqw") {
            num.trim().parse::<f32>().ok().map(Length::Cqw)
        } else if let Some(num) = trimmed.strip_suffix("cqh") {
            num.trim().parse::<f32>().ok().map(Length::Cqh)
        } else if let Some(num) = trimmed.strip_suffix("cqi") {
            num.trim().parse::<f32>().ok().map(Length::Cqi)
        } else if let Some(num) = trimmed.strip_suffix("cqb") {
            num.trim().parse::<f32>().ok().map(Length::Cqb)
        } else if let Some(num) = trimmed.strip_suffix("pt") {
            num.trim()
                .parse::<f32>()
                .ok()
                .map(|v| Length::Px(v * 4.0 / 3.0))
        } else if let Some(num) = trimmed.strip_suffix("pc") {
            num.trim().parse::<f32>().ok().map(|v| Length::Px(v * 16.0))
        } else if let Some(num) = trimmed.strip_suffix("in") {
            num.trim().parse::<f32>().ok().map(|v| Length::Px(v * 96.0))
        } else if let Some(num) = trimmed.strip_suffix("cm") {
            num.trim()
                .parse::<f32>()
                .ok()
                .map(|v| Length::Px(v * 96.0 / 2.54))
        } else if let Some(num) = trimmed.strip_suffix("mm") {
            num.trim()
                .parse::<f32>()
                .ok()
                .map(|v| Length::Px(v * 9.6 / 2.54))
        } else if let Some(num) = trimmed.strip_suffix('q') {
            num.trim()
                .parse::<f32>()
                .ok()
                .map(|v| Length::Px(v * 96.0 / 101.6))
        } else if let Some(num) = trimmed.strip_suffix("ch") {
            num.trim().parse::<f32>().ok().map(|v| Length::Em(v * 0.5))
        } else if let Some(num) = trimmed.strip_suffix("ex") {
            num.trim().parse::<f32>().ok().map(Length::Ex)
        } else if let Ok(n) = trimmed.parse::<f32>() {
            Some(Length::Px(n))
        } else {
            None
        }
    }

    /// Resolves this length to concrete pixels given contextual metrics.
    pub fn to_px(&self, font_size: f32, root_font_size: f32, container_size: f32) -> f32 {
        let (vp_w, vp_h) = get_current_viewport();
        self.to_px_with_viewports(font_size, root_font_size, container_size, vp_w, vp_h)
    }

    /// Returns true if this length is or contains a percentage.
    pub fn has_percent(&self) -> bool {
        match self {
            Length::Percent(_) => true,
            Length::Calc(calc) => calc.has_percent,
            _ => false,
        }
    }

    /// Resolves this length to concrete pixels given contextual metrics and explicit viewport height.
    pub fn to_px_with_viewport(
        &self,
        font_size: f32,
        root_font_size: f32,
        container_size: f32,
        viewport_height: f32,
    ) -> f32 {
        let (vp_w, vp_h) = get_current_viewport();
        let actual_h = if viewport_height > 0.0 && viewport_height != 600.0 {
            viewport_height
        } else {
            vp_h
        };
        self.to_px_with_viewports(font_size, root_font_size, container_size, vp_w, actual_h)
    }

    /// Resolves this length to concrete pixels given contextual metrics and explicit viewport dimensions.
    pub fn to_px_with_viewports(
        &self,
        font_size: f32,
        root_font_size: f32,
        container_size: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> f32 {
        match *self {
            Length::Px(px) => px,
            Length::Em(em) => em * font_size,
            Length::Ex(ex) => ex * 0.5 * font_size,
            Length::Rem(rem) => rem * root_font_size,
            Length::Percent(pct) => (pct / 100.0) * container_size,
            Length::Vw(vw) => (vw / 100.0) * viewport_width,
            Length::Vh(vh) => (vh / 100.0) * viewport_height,
            Length::Vmin(vmin) => (vmin / 100.0) * viewport_width.min(viewport_height),
            Length::Vmax(vmax) => (vmax / 100.0) * viewport_width.max(viewport_height),
            Length::Cqw(cqw) => (cqw / 100.0) * container_size,
            Length::Cqh(cqh) => {
                (cqh / 100.0)
                    * if viewport_height > 0.0 {
                        viewport_height
                    } else {
                        container_size
                    }
            }
            Length::Cqi(cqi) => (cqi / 100.0) * container_size,
            Length::Cqb(cqb) => {
                (cqb / 100.0)
                    * if viewport_height > 0.0 {
                        viewport_height
                    } else {
                        container_size
                    }
            }
            Length::Cqmin(cqmin) => {
                let h = if viewport_height > 0.0 {
                    viewport_height
                } else {
                    container_size
                };
                (cqmin / 100.0) * container_size.min(h)
            }
            Length::Cqmax(cqmax) => {
                let h = if viewport_height > 0.0 {
                    viewport_height
                } else {
                    container_size
                };
                (cqmax / 100.0) * container_size.max(h)
            }
            Length::Calc(calc) => {
                calc.px
                    + (calc.percent / 100.0) * container_size
                    + calc.em * font_size
                    + calc.ex * 0.5 * font_size
                    + calc.rem * root_font_size
                    + (calc.vw / 100.0) * viewport_width
                    + (calc.vh / 100.0) * viewport_height
            }
            Length::Math(id) => eval_math(
                id,
                font_size,
                root_font_size,
                container_size,
                viewport_width,
                viewport_height,
            ),
            Length::Anchor { offset, .. } => offset,
            Length::Auto
            | Length::Content
            | Length::MinContent
            | Length::MaxContent
            | Length::FitContent => 0.0,
        }
    }

    /// Returns `true` if this length is `Auto` or `Content`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto | Length::Content)
    }

    /// Returns `true` if this length represents intrinsic content sizing.
    pub fn is_content(&self) -> bool {
        matches!(
            self,
            Length::Content | Length::MinContent | Length::MaxContent | Length::FitContent
        )
    }
}
