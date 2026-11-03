use mango_css::computed::ComputedStyle;
use mango_css::values::{
    AlignContent as MangoAlignContent, AlignItems as MangoAlignItems, AlignSelf as MangoAlignSelf,
    BoxSizing as MangoBoxSizing, Display as MangoDisplay, FlexDirection as MangoFlexDirection,
    FlexWrap as MangoFlexWrap, GridAutoFlow as MangoGridAutoFlow,
    GridPlacement as MangoGridPlacement, GridTrackSize as MangoGridTrackSize,
    JustifyContent as MangoJustifyContent, Length as MangoLength, Position as MangoPosition,
};
use taffy::geometry::{Line, Rect, Size};
use taffy::prelude::*;
use taffy::style::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Dimension, Display, FlexDirection, FlexWrap,
    GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent, LengthPercentage,
    LengthPercentageAuto, MaxTrackSizingFunction, MinTrackSizingFunction, Position, Style,
    TrackSizingFunction,
};
use taffy::style_helpers::{auto, fr, length, line, minmax, percent, repeat, span, zero};

pub type CheapStr = String;

/// Converts a `mango_css::values::Length` to a Taffy `Dimension`.
pub fn to_taffy_dimension(
    len: &MangoLength,
    font_size: f32,
    root_font_size: f32,
    ref_dim: f32,
) -> Dimension {
    match len {
        MangoLength::Auto => auto(),
        MangoLength::Percent(p) => percent(p / 100.0),
        MangoLength::Px(px) => length(*px),
        _ => {
            let px = len.to_px_with_viewport(font_size, root_font_size, ref_dim, ref_dim);
            length(px)
        }
    }
}

/// Converts a `mango_css::values::Length` to a Taffy `LengthPercentageAuto`.
pub fn to_taffy_length_percentage_auto(
    len: &MangoLength,
    font_size: f32,
    root_font_size: f32,
    ref_dim: f32,
) -> LengthPercentageAuto {
    match len {
        MangoLength::Auto => auto(),
        MangoLength::Percent(p) => percent(p / 100.0),
        MangoLength::Px(px) => length(*px),
        _ => {
            let px = len.to_px_with_viewport(font_size, root_font_size, ref_dim, ref_dim);
            length(px)
        }
    }
}

/// Converts a `mango_css::values::Length` to a Taffy `LengthPercentage`.
pub fn to_taffy_length_percentage(
    len: &MangoLength,
    font_size: f32,
    root_font_size: f32,
    ref_dim: f32,
) -> LengthPercentage {
    match len {
        MangoLength::Auto => zero(),
        MangoLength::Percent(p) => percent(p / 100.0),
        MangoLength::Px(px) => length(*px),
        _ => {
            let px = len.to_px_with_viewport(font_size, root_font_size, ref_dim, ref_dim);
            length(px)
        }
    }
}

/// Converts Mango `FlexDirection` to Taffy `FlexDirection`.
pub fn to_taffy_flex_direction(dir: MangoFlexDirection) -> FlexDirection {
    match dir {
        MangoFlexDirection::Row => FlexDirection::Row,
        MangoFlexDirection::RowReverse => FlexDirection::RowReverse,
        MangoFlexDirection::Column => FlexDirection::Column,
        MangoFlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

/// Converts Mango `FlexWrap` to Taffy `FlexWrap`.
pub fn to_taffy_flex_wrap(wrap: MangoFlexWrap) -> FlexWrap {
    match wrap {
        MangoFlexWrap::NoWrap => FlexWrap::NoWrap,
        MangoFlexWrap::Wrap | MangoFlexWrap::Balance => FlexWrap::Wrap,
        MangoFlexWrap::WrapReverse | MangoFlexWrap::WrapReverseBalance => FlexWrap::WrapReverse,
    }
}

/// Converts Mango `JustifyContent` to Taffy `JustifyContent`.
pub fn to_taffy_justify_content(jc: MangoJustifyContent) -> JustifyContent {
    match jc {
        MangoJustifyContent::Normal
        | MangoJustifyContent::FlexStart
        | MangoJustifyContent::Start
        | MangoJustifyContent::Left => JustifyContent::FLEX_START,
        MangoJustifyContent::FlexEnd | MangoJustifyContent::End | MangoJustifyContent::Right => {
            JustifyContent::FLEX_END
        }
        MangoJustifyContent::Center => JustifyContent::CENTER,
        MangoJustifyContent::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        MangoJustifyContent::SpaceAround => JustifyContent::SPACE_AROUND,
        MangoJustifyContent::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    }
}

/// Converts Mango `AlignItems` to Taffy `AlignItems`.
pub fn to_taffy_align_items(ai: MangoAlignItems) -> AlignItems {
    match ai {
        MangoAlignItems::Normal | MangoAlignItems::Stretch => AlignItems::STRETCH,
        MangoAlignItems::FlexStart
        | MangoAlignItems::SafeFlexStart
        | MangoAlignItems::SafeStart => AlignItems::FLEX_START,
        MangoAlignItems::FlexEnd | MangoAlignItems::SafeFlexEnd | MangoAlignItems::SafeEnd => {
            AlignItems::FLEX_END
        }
        MangoAlignItems::Center | MangoAlignItems::SafeCenter => AlignItems::CENTER,
        MangoAlignItems::Baseline | MangoAlignItems::LastBaseline => AlignItems::BASELINE,
    }
}

/// Converts Mango `AlignSelf` to Taffy `AlignSelf`.
pub fn to_taffy_align_self(aself: MangoAlignSelf) -> Option<AlignSelf> {
    match aself {
        MangoAlignSelf::Auto => None,
        MangoAlignSelf::Stretch => Some(AlignSelf::STRETCH),
        MangoAlignSelf::FlexStart | MangoAlignSelf::SafeFlexStart | MangoAlignSelf::SafeStart => {
            Some(AlignSelf::FLEX_START)
        }
        MangoAlignSelf::FlexEnd | MangoAlignSelf::SafeFlexEnd | MangoAlignSelf::SafeEnd => {
            Some(AlignSelf::FLEX_END)
        }
        MangoAlignSelf::Center | MangoAlignSelf::SafeCenter => Some(AlignSelf::CENTER),
        MangoAlignSelf::Baseline | MangoAlignSelf::LastBaseline => Some(AlignSelf::BASELINE),
    }
}

/// Converts Mango `AlignContent` to Taffy `AlignContent`.
pub fn to_taffy_align_content(ac: MangoAlignContent) -> AlignContent {
    match ac {
        MangoAlignContent::Normal | MangoAlignContent::Stretch => AlignContent::STRETCH,
        MangoAlignContent::FlexStart => AlignContent::FLEX_START,
        MangoAlignContent::FlexEnd => AlignContent::FLEX_END,
        MangoAlignContent::Center => AlignContent::CENTER,
        MangoAlignContent::SpaceBetween => AlignContent::SPACE_BETWEEN,
        MangoAlignContent::SpaceAround => AlignContent::SPACE_AROUND,
        MangoAlignContent::SpaceEvenly => AlignContent::SPACE_EVENLY,
    }
}

/// Converts Mango `GridAutoFlow` to Taffy `GridAutoFlow`.
pub fn to_taffy_grid_auto_flow(flow: MangoGridAutoFlow) -> GridAutoFlow {
    match flow {
        MangoGridAutoFlow::Row => GridAutoFlow::Row,
        MangoGridAutoFlow::Column => GridAutoFlow::Column,
        MangoGridAutoFlow::RowDense => GridAutoFlow::RowDense,
        MangoGridAutoFlow::ColumnDense => GridAutoFlow::ColumnDense,
    }
}

/// Converts Mango `GridPlacement` to Taffy `GridPlacement`.
pub fn to_taffy_grid_placement(gp: &MangoGridPlacement) -> GridPlacement<CheapStr> {
    match gp {
        MangoGridPlacement::Auto => GridPlacement::Auto,
        MangoGridPlacement::Line(idx) => line(*idx as i16),
        MangoGridPlacement::Span(s) => span(*s),
        MangoGridPlacement::Area(name) => {
            GridPlacement::NamedLine(CheapStr::from(name.as_str()), 1)
        }
    }
}

fn convert_min_track(
    track: &MangoGridTrackSize,
    fs: f32,
    root_fs: f32,
    ref_dim: f32,
) -> MinTrackSizingFunction {
    match track {
        MangoGridTrackSize::Auto => MinTrackSizingFunction::AUTO,
        MangoGridTrackSize::MinContent => MinTrackSizingFunction::MIN_CONTENT,
        MangoGridTrackSize::MaxContent => MinTrackSizingFunction::MAX_CONTENT,
        MangoGridTrackSize::Length(MangoLength::Percent(p)) => {
            MinTrackSizingFunction::percent(p / 100.0)
        }
        MangoGridTrackSize::Length(MangoLength::Px(px)) => MinTrackSizingFunction::length(*px),
        MangoGridTrackSize::Length(other) => {
            let px = other.to_px_with_viewport(fs, root_fs, ref_dim, ref_dim);
            MinTrackSizingFunction::length(px)
        }
        _ => MinTrackSizingFunction::AUTO,
    }
}

fn convert_max_track(
    track: &MangoGridTrackSize,
    fs: f32,
    root_fs: f32,
    ref_dim: f32,
) -> MaxTrackSizingFunction {
    match track {
        MangoGridTrackSize::Auto => MaxTrackSizingFunction::AUTO,
        MangoGridTrackSize::MinContent => MaxTrackSizingFunction::MIN_CONTENT,
        MangoGridTrackSize::MaxContent => MaxTrackSizingFunction::MAX_CONTENT,
        MangoGridTrackSize::Fr(f) => fr(*f),
        MangoGridTrackSize::Length(MangoLength::Percent(p)) => {
            MaxTrackSizingFunction::percent(p / 100.0)
        }
        MangoGridTrackSize::Length(MangoLength::Px(px)) => MaxTrackSizingFunction::length(*px),
        MangoGridTrackSize::Length(other) => {
            let px = other.to_px_with_viewport(fs, root_fs, ref_dim, ref_dim);
            MaxTrackSizingFunction::length(px)
        }
        _ => MaxTrackSizingFunction::AUTO,
    }
}

/// Converts a single Mango `GridTrackSize` to a Taffy `TrackSizingFunction`.
pub fn to_taffy_track_sizing_function(
    track: &MangoGridTrackSize,
    fs: f32,
    root_fs: f32,
    ref_dim: f32,
) -> TrackSizingFunction {
    match track {
        MangoGridTrackSize::MinMax(min_t, max_t) => {
            let min_fn = convert_min_track(min_t, fs, root_fs, ref_dim);
            let max_fn = convert_max_track(max_t, fs, root_fs, ref_dim);
            minmax(min_fn, max_fn)
        }
        _ => {
            let min_fn = convert_min_track(track, fs, root_fs, ref_dim);
            let max_fn = convert_max_track(track, fs, root_fs, ref_dim);
            minmax(min_fn, max_fn)
        }
    }
}

/// Converts a list of Mango grid track templates to Taffy track template components.
pub fn to_taffy_grid_template(
    tracks: &[MangoGridTrackSize],
    fs: f32,
    root_fs: f32,
    ref_dim: f32,
) -> Vec<GridTemplateComponent<CheapStr>> {
    let mut out = Vec::with_capacity(tracks.len());
    for t in tracks {
        match t {
            MangoGridTrackSize::RepeatAutoFit(inner) => {
                let track_fn = to_taffy_track_sizing_function(inner, fs, root_fs, ref_dim);
                out.push(repeat(
                    taffy::style::RepetitionCount::AutoFit,
                    vec![track_fn],
                ));
            }
            MangoGridTrackSize::RepeatAutoFill(inner) => {
                let track_fn = to_taffy_track_sizing_function(inner, fs, root_fs, ref_dim);
                out.push(repeat(
                    taffy::style::RepetitionCount::AutoFill,
                    vec![track_fn],
                ));
            }
            other => {
                let track_fn = to_taffy_track_sizing_function(other, fs, root_fs, ref_dim);
                out.push(GridTemplateComponent::Single(track_fn));
            }
        }
    }
    out
}

/// Maps a `ComputedStyle` to a `taffy::style::Style`.
pub fn to_taffy_style(style: &ComputedStyle, cb_w: f32, cb_h: f32) -> Style<CheapStr> {
    let fs = style.font_size;
    let root_fs = style.root_font_size;

    let display = match style.display {
        MangoDisplay::Flex | MangoDisplay::InlineFlex => Display::Flex,
        MangoDisplay::Grid | MangoDisplay::InlineGrid => Display::Grid,
        MangoDisplay::None => Display::None,
        _ => Display::Block,
    };

    let box_sizing = match style.box_sizing {
        MangoBoxSizing::BorderBox => BoxSizing::BorderBox,
        MangoBoxSizing::ContentBox => BoxSizing::ContentBox,
    };

    let position = match style.position {
        MangoPosition::Absolute => Position::Absolute,
        MangoPosition::Fixed => Position::Fixed,
        MangoPosition::Relative => Position::Relative,
        MangoPosition::Sticky => Position::Sticky,
        MangoPosition::Static => Position::Static,
    };

    let size = Size {
        width: to_taffy_dimension(&style.width, fs, root_fs, cb_w),
        height: to_taffy_dimension(&style.height, fs, root_fs, cb_h),
    };

    let min_size = Size {
        width: to_taffy_length_percentage_auto(&style.min_width, fs, root_fs, cb_w),
        height: to_taffy_length_percentage_auto(&style.min_height, fs, root_fs, cb_h),
    };

    let max_size = Size {
        width: to_taffy_length_percentage_auto(&style.max_width, fs, root_fs, cb_w),
        height: to_taffy_length_percentage_auto(&style.max_height, fs, root_fs, cb_h),
    };

    let margin = Rect {
        top: to_taffy_length_percentage_auto(&style.margin_top, fs, root_fs, cb_h),
        right: to_taffy_length_percentage_auto(&style.margin_right, fs, root_fs, cb_w),
        bottom: to_taffy_length_percentage_auto(&style.margin_bottom, fs, root_fs, cb_h),
        left: to_taffy_length_percentage_auto(&style.margin_left, fs, root_fs, cb_w),
    };

    let padding = Rect {
        top: to_taffy_length_percentage(&style.padding_top, fs, root_fs, cb_h),
        right: to_taffy_length_percentage(&style.padding_right, fs, root_fs, cb_w),
        bottom: to_taffy_length_percentage(&style.padding_bottom, fs, root_fs, cb_h),
        left: to_taffy_length_percentage(&style.padding_left, fs, root_fs, cb_w),
    };

    let border = Rect {
        top: length(style.border_top_width),
        right: length(style.border_right_width),
        bottom: length(style.border_bottom_width),
        left: length(style.border_left_width),
    };

    let gap = Size {
        width: to_taffy_length_percentage(&style.column_gap, fs, root_fs, cb_w),
        height: to_taffy_length_percentage(&style.row_gap, fs, root_fs, cb_h),
    };

    let flex_direction = to_taffy_flex_direction(style.flex_direction);
    let flex_wrap = to_taffy_flex_wrap(style.flex_wrap);
    let flex_basis = to_taffy_dimension(&style.flex_basis, fs, root_fs, cb_w);
    let flex_grow = style.flex_grow.max(0.0);
    let flex_shrink = style.flex_shrink.max(0.0);

    let justify_content = to_taffy_justify_content(style.justify_content);
    let align_items = to_taffy_align_items(style.align_items);
    let align_self = to_taffy_align_self(style.align_self);
    let align_content = to_taffy_align_content(style.align_content);

    let grid_template_columns =
        to_taffy_grid_template(&style.grid_template_columns, fs, root_fs, cb_w);
    let grid_template_rows = to_taffy_grid_template(&style.grid_template_rows, fs, root_fs, cb_h);
    let grid_auto_flow = to_taffy_grid_auto_flow(style.grid_auto_flow);

    let grid_column = Line {
        start: to_taffy_grid_placement(&style.grid_column_start),
        end: to_taffy_grid_placement(&style.grid_column_end),
    };
    let grid_row = Line {
        start: to_taffy_grid_placement(&style.grid_row_start),
        end: to_taffy_grid_placement(&style.grid_row_end),
    };

    Style {
        display,
        box_sizing,
        position,
        size,
        min_size,
        max_size,
        aspect_ratio: style.aspect_ratio.filter(|&r| r > 0.0),
        margin,
        padding,
        border,
        gap,
        flex_direction,
        flex_wrap,
        flex_basis,
        flex_grow,
        flex_shrink,
        justify_content,
        align_items,
        align_self,
        align_content,
        grid_template_columns,
        grid_template_rows,
        grid_auto_flow,
        grid_column,
        grid_row,
        ..Default::default()
    }
}
