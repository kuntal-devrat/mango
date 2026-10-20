//! Layout, box model, flexbox, grid, tables, and cursor enums.

use crate::values::length::Length;

/// CSS `display` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Inline,
    Block,
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    InlineGrid,
    FlowRoot,
    Contents,
    Table,
    TableRow,
    TableCell,
    TableCaption,
    ListItem,
    Ruby,
    RubyBase,
    RubyText,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableColumn,
    TableColumnGroup,
    None,
}

impl Display {
    /// Returns `true` if this display value represents an inline-level box.
    #[inline]
    pub fn is_inline_level(&self) -> bool {
        matches!(
            self,
            Display::Inline
                | Display::InlineBlock
                | Display::InlineFlex
                | Display::InlineGrid
                | Display::Ruby
                | Display::RubyBase
                | Display::RubyText
        )
    }
}

/// CSS `position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// CSS `float` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

/// CSS `clear` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

/// CSS `box-sizing` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// CSS `appearance` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Auto,
    None,
}

/// CSS `flex-direction` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

/// CSS `flex-wrap` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// CSS `justify-content` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS `justify-items` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `justify-self` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifySelf {
    #[default]
    Auto,
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `align-items` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `align-self` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignSelf {
    #[default]
    Auto,
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// CSS `align-content` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignContent {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// CSS `overflow` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// CSS `visibility` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
    Collapse,
}

/// CSS `table-layout` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableLayout {
    #[default]
    Auto,
    Fixed,
}

/// CSS `border-collapse` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderCollapse {
    #[default]
    Separate,
    Collapse,
}

/// CSS `caption-side` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptionSide {
    #[default]
    Top,
    Bottom,
}

/// CSS `list-style-type` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStyleType {
    #[default]
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    None,
}

/// CSS `list-style-position` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStylePosition {
    #[default]
    Outside,
    Inside,
}

/// CSS `cursor` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    #[default]
    Auto,
    Default,
    Pointer,
    Text,
    Move,
    NotAllowed,
    Crosshair,
    Wait,
    Help,
}

/// CSS `vertical-align` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    Top,
    Middle,
    Bottom,
    TextTop,
    TextBottom,
    Sub,
    Super,
}

/// A sizing specification for a CSS Grid track (column or row).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum GridTrackSize {
    /// Exact or relative length (px, em, rem, %, vw, vh).
    Length(Length),
    /// Flexible fraction of remaining space (e.g. `1fr`, `2.5fr`).
    Fr(f32),
    /// Auto sizing based on content.
    #[default]
    Auto,
    /// Minimum content size.
    MinContent,
    /// Maximum content size.
    MaxContent,
    /// minmax(min, max) sizing constraint.
    MinMax(Box<GridTrackSize>, Box<GridTrackSize>),
    /// Subgrid: adopts tracks from parent grid.
    Subgrid,
    /// `repeat(auto-fill, ...)`
    RepeatAutoFill(Box<GridTrackSize>),
    /// `repeat(auto-fit, ...)`
    RepeatAutoFit(Box<GridTrackSize>),
}

/// CSS `grid-auto-flow` property values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

impl GridAutoFlow {
    pub fn is_row(self) -> bool {
        matches!(self, GridAutoFlow::Row | GridAutoFlow::RowDense)
    }

    pub fn is_column(self) -> bool {
        matches!(self, GridAutoFlow::Column | GridAutoFlow::ColumnDense)
    }

    pub fn is_dense(self) -> bool {
        matches!(self, GridAutoFlow::RowDense | GridAutoFlow::ColumnDense)
    }
}

/// Placement coordinate for a grid item along a grid axis (start or end).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GridPlacement {
    #[default]
    Auto,
    /// 1-based grid line index (e.g. 1, 2, -1).
    Line(i32),
    /// Span a number of tracks (e.g. `span 2`).
    Span(u16),
    /// Named grid area (e.g. `titlebar`, `content`, `columnStart`).
    Area(String),
}
