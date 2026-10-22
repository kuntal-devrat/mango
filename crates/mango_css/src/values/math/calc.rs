//! Calculation units and anchor positioning types.

/// A linear combination of dimensional units in a CSS `calc(...)` expression.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcLength {
    pub px: f32,
    pub percent: f32,
    pub has_percent: bool,
    pub em: f32,
    pub ex: f32,
    pub rem: f32,
    pub vw: f32,
    pub vh: f32,
}

impl Eq for CalcLength {}

impl std::hash::Hash for CalcLength {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.px.to_bits().hash(state);
        self.percent.to_bits().hash(state);
        self.has_percent.hash(state);
        self.em.to_bits().hash(state);
        self.ex.to_bits().hash(state);
        self.rem.to_bits().hash(state);
        self.vw.to_bits().hash(state);
        self.vh.to_bits().hash(state);
    }
}

impl CalcLength {
    pub const ZERO: CalcLength = CalcLength {
        px: 0.0,
        percent: 0.0,
        has_percent: false,
        em: 0.0,
        ex: 0.0,
        rem: 0.0,
        vw: 0.0,
        vh: 0.0,
    };

    pub fn is_zero(&self) -> bool {
        !self.has_percent
            && self.px == 0.0
            && self.percent == 0.0
            && self.em == 0.0
            && self.ex == 0.0
            && self.rem == 0.0
            && self.vw == 0.0
            && self.vh == 0.0
    }
}

/// The side of an anchor box to align to in `anchor()` expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AnchorSide {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
    Center,
    Start,
    End,
}

impl AnchorSide {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "top" => Some(AnchorSide::Top),
            "bottom" => Some(AnchorSide::Bottom),
            "left" => Some(AnchorSide::Left),
            "right" => Some(AnchorSide::Right),
            "center" => Some(AnchorSide::Center),
            "start" | "self-start" => Some(AnchorSide::Start),
            "end" | "self-end" => Some(AnchorSide::End),
            _ => None,
        }
    }
}
