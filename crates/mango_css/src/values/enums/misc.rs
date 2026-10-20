//! Miscellaneous CSS enums: containment, scroll timeline, writing mode, content, and counters.

/// CSS `scroll-timeline-axis` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollTimelineAxis {
    #[default]
    Block,
    Inline,
    X,
    Y,
}

impl ScrollTimelineAxis {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "block" => Some(Self::Block),
            "inline" => Some(Self::Inline),
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            _ => None,
        }
    }
}

/// CSS `contain` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Contain {
    #[default]
    None,
    Strict,
    Content,
    Paint,
    Layout,
    Size,
    Style,
    InlineSize,
    Compound {
        size: bool,
        inline_size: bool,
        layout: bool,
        style: bool,
        paint: bool,
    },
}

impl Contain {
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "none" => Some(Self::None),
            "strict" => Some(Self::Strict),
            "content" => Some(Self::Content),
            "paint" => Some(Self::Paint),
            "layout" => Some(Self::Layout),
            "size" => Some(Self::Size),
            "style" => Some(Self::Style),
            "inline-size" => Some(Self::InlineSize),
            _ => {
                let mut size = false;
                let mut inline_size = false;
                let mut layout = false;
                let mut style = false;
                let mut paint = false;
                let mut count = 0;
                for part in trimmed.split_whitespace() {
                    match part {
                        "size" => {
                            size = true;
                            count += 1;
                        }
                        "inline-size" => {
                            inline_size = true;
                            count += 1;
                        }
                        "layout" => {
                            layout = true;
                            count += 1;
                        }
                        "style" => {
                            style = true;
                            count += 1;
                        }
                        "paint" => {
                            paint = true;
                            count += 1;
                        }
                        _ => return None,
                    }
                }
                if count > 0 {
                    Some(Self::Compound {
                        size,
                        inline_size,
                        layout,
                        style,
                        paint,
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn contains_size(&self) -> bool {
        match self {
            Contain::Size | Contain::Strict => true,
            Contain::Compound { size, .. } => *size,
            _ => false,
        }
    }

    pub fn contains_inline_size(&self) -> bool {
        match self {
            Contain::InlineSize | Contain::Size | Contain::Strict => true,
            Contain::Compound {
                size, inline_size, ..
            } => *size || *inline_size,
            _ => false,
        }
    }

    pub fn contains_layout(&self) -> bool {
        match self {
            Contain::Layout | Contain::Content | Contain::Strict => true,
            Contain::Compound { layout, .. } => *layout,
            _ => false,
        }
    }

    pub fn contains_paint(&self) -> bool {
        match self {
            Contain::Paint | Contain::Content | Contain::Strict => true,
            Contain::Compound { paint, .. } => *paint,
            _ => false,
        }
    }

    pub fn contains_style(&self) -> bool {
        match self {
            Contain::Style | Contain::Content | Contain::Strict => true,
            Contain::Compound { style, .. } => *style,
            _ => false,
        }
    }
}

/// CSS `content-visibility` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentVisibility {
    #[default]
    Visible,
    Auto,
    Hidden,
}

/// CSS `writing-mode` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// CSS `resize` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Resize {
    #[default]
    None,
    Both,
    Horizontal,
    Vertical,
}

/// CSS `scrollbar-width` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// CSS `column-span` property values (PRD 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnSpan {
    #[default]
    None,
    All,
}

/// CSS `break-inside` property values (PRD 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakInside {
    #[default]
    Auto,
    Avoid,
    AvoidColumn,
    AvoidPage,
}

/// An individual item in a `content` property value list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentItem {
    /// Literal string text.
    String(String),
    /// `attr(attribute-name)` resolved from the originating element.
    Attr(String),
    /// `counter(name, style?)` formatted from the current counter value.
    Counter { name: String, style: Option<String> },
    /// `counters(name, separator, style?)` formatted from all counter scopes.
    Counters {
        name: String,
        separator: String,
        style: Option<String>,
    },
    /// `open-quote`: emits an opening quote at current quote depth and increases depth.
    OpenQuote,
    /// `close-quote`: decreases quote depth and emits a closing quote.
    CloseQuote,
    /// `no-open-quote`: increases quote depth without emitting text.
    NoOpenQuote,
    /// `no-close-quote`: decreases quote depth without emitting text.
    NoCloseQuote,
    /// `url(...)` replaced image content.
    Url(String),
}

/// An action for `counter-reset` or `counter-increment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterAction {
    pub name: String,
    pub value: i32,
}

/// CSS `container-type` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerType {
    #[default]
    Normal,
    Size,
    InlineSize,
    ScrollState,
}

impl ContainerType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "size" => Some(Self::Size),
            "inline-size" => Some(Self::InlineSize),
            "scroll-state" => Some(Self::ScrollState),
            _ => None,
        }
    }

    pub fn affects_inline(&self) -> bool {
        matches!(self, Self::Size | Self::InlineSize)
    }

    pub fn affects_block(&self) -> bool {
        matches!(self, Self::Size)
    }
}

/// CSS `pointer-events` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    #[default]
    Auto,
    None,
}

impl PointerEvents {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// CSS `user-select` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserSelect {
    #[default]
    Auto,
    None,
    Text,
    All,
}

impl UserSelect {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "none" => Some(Self::None),
            "text" => Some(Self::Text),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// CSS `scroll-behavior` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

impl ScrollBehavior {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "smooth" => Some(Self::Smooth),
            _ => None,
        }
    }
}
