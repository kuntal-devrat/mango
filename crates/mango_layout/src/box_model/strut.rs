/// Concentric visual areas of the CSS box model (CSS Box Model Module Level 3 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoxArea {
    /// The innermost content area, where text, child elements, or media render.
    Content,
    /// The area bounded by the padding edge.
    Padding,
    /// The area bounded by the border edge.
    Border,
    /// The outermost area bounded by the margin edge.
    Margin,
}

/// Tracks positive and negative margins during vertical margin collapsing
/// conforming to CSS 2.1 §8.3.1 and Blink's `MarginStrut`.
///
/// Under CSS 2.1 §8.3.1:
/// "When two or more margins collapse, the resulting margin width is the maximum
/// of the adjoining positive margins (or zero if there are none), minus the
/// maximum of the absolute values of the adjoining negative margins (or zero if there are none)."
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MarginStrut {
    /// Maximum positive adjoining margin (or 0.0 if none).
    pub positive: f32,
    /// Minimum (most negative) adjoining negative margin (or 0.0 if none).
    pub negative: f32,
    /// Whether any non-zero margins have been contributed to this strut.
    pub has_margins: bool,
}

impl MarginStrut {
    /// Creates a new empty margin strut.
    #[inline]
    pub const fn new() -> Self {
        Self {
            positive: 0.0,
            negative: 0.0,
            has_margins: false,
        }
    }

    /// Appends a single margin value to this strut.
    #[inline]
    pub fn append(&mut self, margin: f32) {
        if margin.is_nan() {
            return;
        }
        self.has_margins = true;
        if margin >= 0.0 {
            self.positive = self.positive.max(margin);
        } else {
            self.negative = self.negative.min(margin);
        }
    }

    /// Appends another margin strut, combining positive and negative bounds.
    #[inline]
    pub fn append_strut(&mut self, other: MarginStrut) {
        if !other.has_margins {
            return;
        }
        self.has_margins = true;
        self.positive = self.positive.max(other.positive);
        self.negative = self.negative.min(other.negative);
    }

    /// Solves the collapsed margin per CSS 2.1 §8.3.1: `max_pos - abs(max_neg)`.
    #[inline]
    pub fn solve(&self) -> f32 {
        if !self.has_margins {
            0.0
        } else {
            self.positive + self.negative
        }
    }

    /// Clears the accumulated margin state.
    #[inline]
    pub fn reset(&mut self) {
        self.positive = 0.0;
        self.negative = 0.0;
        self.has_margins = false;
    }
}
