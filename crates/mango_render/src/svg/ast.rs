//! SVG Document Object Model AST nodes and render state representation.

use mango_core::Color;
use std::collections::HashMap;
use tiny_skia::{FillRule, LineCap, LineJoin, Path, Transform};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SvgPaint {
    None,
    Color(Color),
    CurrentColor,
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
    pub(crate) opacity: f32,
}

/// A parsed SVG `<symbol>` template or reusable graphic definition.
#[derive(Debug, Clone)]
pub struct SvgSymbol {
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub children: Vec<SvgNode>,
    pub state: SvgRenderState,
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
}

#[derive(Debug)]
pub struct SvgDocument {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub view_box: Option<(f32, f32, f32, f32)>,
    pub children: Vec<SvgNode>,
    pub symbols: HashMap<String, SvgSymbol>,
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
        }
    } else {
        root_children.push(node);
    }
}
