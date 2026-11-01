//! SVG Document Object Model AST nodes and render state representation.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{FillRule, LineCap, LineJoin, Path, Transform};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SvgPaint {
    None,
    Color(Color),
    CurrentColor,
    Url(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgSpreadMethod {
    #[default]
    Pad,
    Reflect,
    Repeat,
}

#[derive(Debug, Clone)]
pub struct SvgStop {
    pub offset: f32,
    pub color: Color,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub enum SvgGradient {
    Linear {
        id: String,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: Vec<SvgStop>,
        transform: Option<Transform>,
        spread_method: SvgSpreadMethod,
        user_space_on_use: bool,
    },
    Radial {
        id: String,
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: Vec<SvgStop>,
        transform: Option<Transform>,
        spread_method: SvgSpreadMethod,
        user_space_on_use: bool,
    },
}

#[derive(Debug, Clone)]
pub struct SvgClipPath {
    pub id: String,
    pub children: Vec<SvgNode>,
    pub transform: Option<Transform>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatioAlign {
    None,
    #[default]
    XMidYMid,
    XMinYMin,
    XMidYMin,
    XMaxYMin,
    XMinYMid,
    XMaxYMid,
    XMinYMax,
    XMidYMax,
    XMaxYMax,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeetOrSlice {
    #[default]
    Meet,
    Slice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreserveAspectRatio {
    pub align: AspectRatioAlign,
    pub meet_or_slice: MeetOrSlice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SvgTextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone)]
pub struct SvgTspan {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub dx: Option<f32>,
    pub dy: Option<f32>,
    pub text: String,
    pub state: SvgRenderState,
}

#[derive(Debug, Clone)]
pub struct SvgRenderState {
    pub(crate) fill: Option<SvgPaint>,
    pub(crate) fill_rule: FillRule,
    pub(crate) fill_opacity: f32,
    pub(crate) stroke: Option<SvgPaint>,
    pub(crate) stroke_width: f32,
    pub(crate) stroke_opacity: f32,
    pub(crate) stroke_linecap: LineCap,
    pub(crate) stroke_linejoin: LineJoin,
    pub(crate) stroke_dasharray: Option<Vec<f32>>,
    pub(crate) stroke_dashoffset: f32,
    pub(crate) clip_path: Option<String>,
    pub(crate) opacity: f32,
    pub(crate) font_size: f32,
    pub(crate) font_family: String,
    pub(crate) font_weight: String,
    pub(crate) font_style: String,
    pub(crate) text_anchor: SvgTextAnchor,
}

/// A parsed SVG `<symbol>` template or reusable graphic definition.
#[derive(Debug, Clone)]
pub struct SvgSymbol {
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub children: Vec<SvgNode>,
    pub state: SvgRenderState,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
}

#[derive(Debug, Clone)]
pub enum SvgNode {
    Path {
        path: Path,
        state: SvgRenderState,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rx: f32,
        ry: f32,
        state: SvgRenderState,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        state: SvgRenderState,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        state: SvgRenderState,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        state: SvgRenderState,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        state: SvgRenderState,
    },
    Polyline {
        points: Vec<(f32, f32)>,
        state: SvgRenderState,
    },
    Group {
        children: Vec<SvgNode>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
    Use {
        href: String,
        x: f32,
        y: f32,
        width: Option<f32>,
        height: Option<f32>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
    Text {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        text: String,
        font_size: f32,
        font_family: String,
        font_weight: String,
        font_style: String,
        text_anchor: SvgTextAnchor,
        tspans: Vec<SvgTspan>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
}

#[derive(Debug)]
pub struct SvgDocument {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub preserve_aspect_ratio: Option<PreserveAspectRatio>,
    pub children: Vec<SvgNode>,
    pub symbols: HashMap<String, SvgSymbol>,
    pub gradients: HashMap<String, SvgGradient>,
    pub clip_paths: HashMap<String, SvgClipPath>,
    pub root_state: SvgRenderState,
}

#[derive(Debug)]
pub(crate) enum SvgContainer {
    Group {
        children: Vec<SvgNode>,
        state: SvgRenderState,
        transform: Option<Transform>,
        id: Option<String>,
    },
    Defs {
        children: Vec<SvgNode>,
    },
    Symbol {
        id: String,
        view_box: Option<(f32, f32, f32, f32)>,
        children: Vec<SvgNode>,
        state: SvgRenderState,
        preserve_aspect_ratio: Option<PreserveAspectRatio>,
    },
    ClipPath {
        id: String,
        children: Vec<SvgNode>,
        transform: Option<Transform>,
    },
    Text {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        text: String,
        font_size: f32,
        font_family: String,
        font_weight: String,
        font_style: String,
        text_anchor: SvgTextAnchor,
        tspans: Vec<SvgTspan>,
        state: SvgRenderState,
        transform: Option<Transform>,
    },
}

pub(crate) fn get_cur_state(
    container_stack: &[SvgContainer],
    base_state: &SvgRenderState,
) -> SvgRenderState {
    container_stack
        .last()
        .map(|c| match c {
            SvgContainer::Group { state, .. } => state.clone(),
            SvgContainer::Symbol { state, .. } => state.clone(),
            SvgContainer::Text { state, .. } => state.clone(),
            SvgContainer::ClipPath { .. } => base_state.clone(),
            SvgContainer::Defs { .. } => SvgRenderState {
                fill: None,
                stroke: None,
                ..base_state.clone()
            },
        })
        .unwrap_or_else(|| base_state.clone())
}

pub(crate) fn add_node_to_container(
    container_stack: &mut [SvgContainer],
    root_children: &mut Vec<SvgNode>,
    node: SvgNode,
) {
    if let Some(top) = container_stack.last_mut() {
        match top {
            SvgContainer::Group { children, .. } => children.push(node),
            SvgContainer::Defs { children } => children.push(node),
            SvgContainer::Symbol { children, .. } => children.push(node),
            SvgContainer::ClipPath { children, .. } => children.push(node),
            SvgContainer::Text { .. } => {}
        }
    } else {
        root_children.push(node);
    }
}
