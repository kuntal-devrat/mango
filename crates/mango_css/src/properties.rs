//! CSS property declarations and shorthand expansion logic.

use crate::values::{BorderStyle, Display, Length, Value};

/// A single CSS property-value declaration pair (e.g. `color: red !important;`).
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    /// Normalized lowercase property name.
    pub name: String,
    /// Parsed property value.
    pub value: Value,
    /// Whether the declaration was annotated with `!important`.
    pub important: bool,
}

impl Declaration {
    pub fn new(name: impl Into<String>, value: Value, important: bool) -> Self {
        Self {
            name: name.into().to_ascii_lowercase(),
            value,
            important,
        }
    }

    /// Expands shorthand properties into individual longhand declarations.
    ///
    /// For example:
    /// - `margin: 10px 20px` -> `margin-top`, `margin-right`, `margin-bottom`, `margin-left`
    /// - `padding: 5px` -> `padding-top`, `padding-right`, `padding-bottom`, `padding-left`
    /// - `border: 1px solid red` -> `border-*-width`, `border-*-style`, `border-*-color`
    /// - `background: #fff` -> `background-color: #fff`
    pub fn expand_shorthand(self) -> Vec<Declaration> {
        let important = self.important;
        match self.name.as_str() {
            "margin" => expand_four_sides("margin", &self.value, important),
            "padding" => expand_four_sides("padding", &self.value, important),

            "border" => {
                expand_border_sides(&["top", "right", "bottom", "left"], &self.value, important)
            }
            "border-top" => expand_border_sides(&["top"], &self.value, important),
            "border-right" => expand_border_sides(&["right"], &self.value, important),
            "border-bottom" => expand_border_sides(&["bottom"], &self.value, important),
            "border-left" => expand_border_sides(&["left"], &self.value, important),

            "border-width" => expand_four_sides_scalar("border", "width", &self.value, important),
            "border-style" => expand_four_sides_scalar("border", "style", &self.value, important),
            "border-color" => expand_four_sides_scalar("border", "color", &self.value, important),

            "background" => expand_background_shorthand(&self.value, important),

            "flex" => expand_flex_shorthand(&self.value, important),
            "flex-flow" => expand_flex_flow_shorthand(&self.value, important),
            "gap" => expand_gap_shorthand(&self.value, important),

            "border-radius" => expand_border_radius_shorthand(&self.value, important),
            "overflow" => expand_overflow_shorthand(&self.value, important),
            "list-style" => expand_list_style_shorthand(&self.value, important),
            "outline" => expand_outline_shorthand(&self.value, important),
            "columns" => expand_columns_shorthand(&self.value, important),
            "mask" => expand_mask_shorthand("mask", &self.value, important),
            "-webkit-mask" => expand_mask_shorthand("-webkit-mask", &self.value, important),
            "grid-column" => expand_grid_placement_shorthand("grid-column", &self.value, important),
            "grid-row" => expand_grid_placement_shorthand("grid-row", &self.value, important),
            "grid-area" => expand_grid_area_shorthand(&self.value, important),
            "grid-template" => expand_grid_template_shorthand(&self.value, important),
            "place-items" => {
                expand_place_shorthand("align-items", "justify-items", &self.value, important)
            }
            "place-content" => {
                expand_place_shorthand("align-content", "justify-content", &self.value, important)
            }
            "place-self" => {
                expand_place_shorthand("align-self", "justify-self", &self.value, important)
            }
            "column-rule" => expand_column_rule_shorthand(&self.value, important),
            "text-emphasis" => expand_text_emphasis_shorthand(&self.value, important),
            "container" => expand_container_shorthand(&self.value, important),

            _ => vec![self],
        }
    }
}

/// Expands `container` shorthand into `container-name` and `container-type`.
fn expand_container_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) if items.len() >= 2 => {
            vec![
                Declaration::new("container-name", items[0].clone(), important),
                Declaration::new("container-type", items[1].clone(), important),
            ]
        }
        Value::ContainerType(ct) => {
            vec![
                Declaration::new(
                    "container-name",
                    Value::Keyword("none".to_string()),
                    important,
                ),
                Declaration::new("container-type", Value::ContainerType(*ct), important),
            ]
        }
        Value::String(s) | Value::Keyword(s) => {
            if let Some(ct) = crate::values::ContainerType::parse(s) {
                vec![
                    Declaration::new(
                        "container-name",
                        Value::Keyword("none".to_string()),
                        important,
                    ),
                    Declaration::new("container-type", Value::ContainerType(ct), important),
                ]
            } else {
                vec![
                    Declaration::new("container-name", Value::String(s.clone()), important),
                    Declaration::new(
                        "container-type",
                        Value::ContainerType(crate::values::ContainerType::Normal),
                        important,
                    ),
                ]
            }
        }
        _ => vec![Declaration::new("container", value.clone(), important)],
    }
}

/// Expands grid-column or grid-row shorthand into -start and -end declarations.
fn expand_grid_placement_shorthand(
    prefix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let start_name = format!("{prefix}-start");
    let end_name = format!("{prefix}-end");

    match value {
        Value::List(items) if items.len() >= 2 => {
            vec![
                Declaration::new(start_name, items[0].clone(), important),
                Declaration::new(end_name, items[1].clone(), important),
            ]
        }
        single => {
            vec![Declaration::new(start_name, single.clone(), important)]
        }
    }
}

/// Expands `grid-area` into `grid-row-start`, `grid-column-start`, `grid-row-end`, `grid-column-end`.
fn expand_grid_area_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) => match items.len() {
            1 => {
                let v = items[0].clone();
                vec![
                    Declaration::new("grid-row-start", v.clone(), important),
                    Declaration::new("grid-column-start", v.clone(), important),
                    Declaration::new("grid-row-end", v.clone(), important),
                    Declaration::new("grid-column-end", v, important),
                ]
            }
            2 => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new(
                    "grid-row-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
                Declaration::new(
                    "grid-column-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
            ],
            3 => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new("grid-row-end", items[2].clone(), important),
                Declaration::new(
                    "grid-column-end",
                    Value::GridPlacement(crate::values::GridPlacement::Auto),
                    important,
                ),
            ],
            _ => vec![
                Declaration::new("grid-row-start", items[0].clone(), important),
                Declaration::new("grid-column-start", items[1].clone(), important),
                Declaration::new("grid-row-end", items[2].clone(), important),
                Declaration::new("grid-column-end", items[3].clone(), important),
            ],
        },
        single => vec![
            Declaration::new("grid-row-start", single.clone(), important),
            Declaration::new("grid-column-start", single.clone(), important),
            Declaration::new("grid-row-end", single.clone(), important),
            Declaration::new("grid-column-end", single.clone(), important),
        ],
    }
}

/// Expands `grid-template: <rows> / <columns>` into `grid-template-rows` and `grid-template-columns`.
fn expand_grid_template_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    match value {
        Value::List(items) if items.len() >= 2 => vec![
            Declaration::new("grid-template-rows", items[0].clone(), important),
            Declaration::new("grid-template-columns", items[1].clone(), important),
        ],
        single => vec![Declaration::new(
            "grid-template-rows",
            single.clone(),
            important,
        )],
    }
}

/// Expands border-radius shorthand into four corner longhands.
fn expand_border_radius_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (tl, tr, br, bl) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return vec![Declaration::new("border-radius", value.clone(), important)],
    };

    vec![
        Declaration::new("border-top-left-radius", tl, important),
        Declaration::new("border-top-right-radius", tr, important),
        Declaration::new("border-bottom-right-radius", br, important),
        Declaration::new("border-bottom-left-radius", bl, important),
    ]
}

/// Expands overflow shorthand into overflow-x and overflow-y.
fn expand_overflow_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (ox, oy) = match values.len() {
        1 => (values[0].clone(), values[0].clone()),
        2 => (values[0].clone(), values[1].clone()),
        _ => return vec![Declaration::new("overflow", value.clone(), important)],
    };

    vec![
        Declaration::new("overflow-x", ox, important),
        Declaration::new("overflow-y", oy, important),
    ]
}

/// Expands list-style shorthand into list-style-type and list-style-position.
fn expand_list_style_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let mut decls = Vec::new();
    for v in values {
        match &v {
            Value::ListStyleType(_) => {
                decls.push(Declaration::new("list-style-type", v, important));
            }
            Value::ListStylePosition(_) => {
                decls.push(Declaration::new("list-style-position", v, important));
            }
            _ => {}
        }
    }
    if decls.is_empty() {
        vec![Declaration::new(
            "list-style-type",
            value.clone(),
            important,
        )]
    } else {
        decls
    }
}

/// Expands 1, 2, 3, or 4-value box shorthands (margin / padding).
fn expand_four_sides(prefix: &str, value: &Value, important: bool) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (top, right, bottom, left) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => return vec![Declaration::new(prefix, value.clone(), important)],
    };

    vec![
        Declaration::new(format!("{prefix}-top"), top, important),
        Declaration::new(format!("{prefix}-right"), right, important),
        Declaration::new(format!("{prefix}-bottom"), bottom, important),
        Declaration::new(format!("{prefix}-left"), left, important),
    ]
}

/// Expands border scalar properties (e.g. `border-width: 1px 2px`).
fn expand_four_sides_scalar(
    prefix: &str,
    suffix: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let values = match value {
        Value::List(items) => items.clone(),
        single => vec![single.clone()],
    };

    let (top, right, bottom, left) = match values.len() {
        1 => (
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
            values[0].clone(),
        ),
        2 => (
            values[0].clone(),
            values[1].clone(),
            values[0].clone(),
            values[1].clone(),
        ),
        3 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[1].clone(),
        ),
        4 => (
            values[0].clone(),
            values[1].clone(),
            values[2].clone(),
            values[3].clone(),
        ),
        _ => {
            return vec![Declaration::new(
                format!("{prefix}-{suffix}"),
                value.clone(),
                important,
            )];
        }
    };

    vec![
        Declaration::new(format!("{prefix}-top-{suffix}"), top, important),
        Declaration::new(format!("{prefix}-right-{suffix}"), right, important),
        Declaration::new(format!("{prefix}-bottom-{suffix}"), bottom, important),
        Declaration::new(format!("{prefix}-left-{suffix}"), left, important),
    ]
}

/// Expands `background` shorthand into background-color, background-image, background-repeat, background-size, background-position.
fn expand_background_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    // Check if there are comma separators indicating multiple background layers
    let layer_chunks: Vec<&[Value]> = items
        .split(|item| matches!(item, Value::Keyword(k) if k == ","))
        .collect();

    if layer_chunks.len() > 1 {
        let mut images = Vec::new();
        let mut repeats = Vec::new();
        let mut sizes = Vec::new();
        let mut positions = Vec::new();
        let mut attachments = Vec::new();
        let mut clips = Vec::new();
        let mut color = None;

        for chunk in layer_chunks {
            let mut layer_img = None;
            let mut layer_repeat = None;
            let mut layer_size = None;
            let mut layer_pos = Vec::new();
            let mut layer_sizes = Vec::new();
            let mut in_size = false;
            let mut layer_att = None;
            let mut layer_clip = None;

            for item in chunk {
                if let Value::Keyword(k) = item
                    && k == "/"
                {
                    in_size = true;
                    continue;
                }
                match item {
                    Value::Url(_) | Value::Gradient(_) => {
                        if layer_img.is_none() {
                            layer_img = Some(item.clone());
                        }
                    }
                    Value::BackgroundRepeat(_) => layer_repeat = Some(item.clone()),
                    Value::BackgroundSize(_) => layer_size = Some(item.clone()),
                    Value::BackgroundAttachment(_) => layer_att = Some(item.clone()),
                    Value::BackgroundClip(_) => layer_clip = Some(item.clone()),
                    Value::Color(_) | Value::Var { .. } | Value::CurrentColor => {
                        color = Some(item.clone())
                    }
                    Value::Length(_) | Value::Percentage(_) => {
                        if in_size {
                            layer_sizes.push(item.clone());
                        } else {
                            layer_pos.push(item.clone());
                        }
                    }
                    Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                        "none" => {
                            layer_img = Some(Value::Keyword("none".to_string()));
                        }
                        "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => {
                            layer_repeat = Some(item.clone())
                        }
                        "cover" | "contain" => layer_size = Some(item.clone()),
                        "auto" => {
                            if in_size {
                                layer_sizes.push(item.clone());
                            } else {
                                layer_pos.push(item.clone());
                            }
                        }
                        "top" | "bottom" | "left" | "right" | "center" => {
                            layer_pos.push(item.clone());
                        }
                        "scroll" | "fixed" | "local" => {
                            if let Some(a) = crate::values::BackgroundAttachment::parse(k) {
                                layer_att = Some(Value::BackgroundAttachment(a));
                            }
                        }
                        "border-box" | "padding-box" | "content-box" => {
                            if let Some(c) = crate::values::BackgroundClip::parse(k) {
                                layer_clip = Some(Value::BackgroundClip(c));
                            }
                        }
                        "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                        "currentcolor" => color = Some(Value::CurrentColor),
                        _ => {}
                    },
                    _ => {}
                }
            }

            images.push(layer_img.unwrap_or(Value::Keyword("none".to_string())));
            if let Some(r) = layer_repeat {
                repeats.push(r);
            }
            if let Some(s) = layer_size {
                sizes.push(s);
            } else if !layer_sizes.is_empty() {
                sizes.push(if layer_sizes.len() == 1 {
                    layer_sizes[0].clone()
                } else {
                    Value::List(layer_sizes)
                });
            }
            if !layer_pos.is_empty() {
                positions.push(if layer_pos.len() == 1 {
                    layer_pos[0].clone()
                } else {
                    Value::List(layer_pos)
                });
            }
            if let Some(a) = layer_att {
                attachments.push(a);
            }
            if let Some(c) = layer_clip {
                clips.push(c);
            }
        }

        let mut decls = Vec::new();
        if let Some(c) = color {
            decls.push(Declaration::new("background-color", c, important));
        }
        if !images.is_empty() {
            decls.push(Declaration::new(
                "background-image",
                Value::List(images),
                important,
            ));
        }
        if !repeats.is_empty() {
            decls.push(Declaration::new(
                "background-repeat",
                Value::List(repeats),
                important,
            ));
        }
        if !sizes.is_empty() {
            decls.push(Declaration::new(
                "background-size",
                Value::List(sizes),
                important,
            ));
        }
        if !positions.is_empty() {
            decls.push(Declaration::new(
                "background-position",
                Value::List(positions),
                important,
            ));
        }
        if !attachments.is_empty() {
            decls.push(Declaration::new(
                "background-attachment",
                Value::List(attachments),
                important,
            ));
        }
        if !clips.is_empty() {
            decls.push(Declaration::new(
                "background-clip",
                Value::List(clips),
                important,
            ));
        }
        return decls;
    }

    let mut decls = Vec::new();
    let mut image = None;
    let mut gradient = None;
    let mut repeat = None;
    let mut color = None;
    let mut size = None;
    let mut positions = Vec::new();
    let mut sizes = Vec::new();
    let mut in_size = false;
    let mut attachment = None;
    let mut clip = None;

    for item in items {
        if let Value::Keyword(k) = item
            && k == "/"
        {
            in_size = true;
            continue;
        }
        match item {
            Value::Url(_) => {
                if image.is_none() {
                    image = Some(item.clone());
                }
            }
            Value::Gradient(_) => {
                if gradient.is_none() {
                    gradient = Some(item.clone());
                }
            }
            Value::BackgroundRepeat(_) => repeat = Some(item.clone()),
            Value::BackgroundSize(_) => size = Some(item.clone()),
            Value::BackgroundAttachment(_) => attachment = Some(item.clone()),
            Value::BackgroundClip(_) => clip = Some(item.clone()),
            Value::Color(_) | Value::Var { .. } | Value::CurrentColor => color = Some(item.clone()),
            Value::Length(_) | Value::Percentage(_) => {
                if in_size {
                    sizes.push(item.clone());
                } else {
                    positions.push(item.clone());
                }
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" => {
                    image = Some(Value::Keyword("none".to_string()));
                    if color.is_none() {
                        color = Some(Value::Color(mango_core::Color::TRANSPARENT));
                    }
                }
                "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => repeat = Some(item.clone()),
                "cover" | "contain" => size = Some(item.clone()),
                "auto" => {
                    if in_size {
                        sizes.push(item.clone());
                    } else {
                        positions.push(item.clone());
                    }
                }
                "top" | "bottom" | "left" | "right" | "center" => {
                    positions.push(item.clone());
                }
                "scroll" | "fixed" | "local" => {
                    if let Some(a) = crate::values::BackgroundAttachment::parse(k) {
                        attachment = Some(Value::BackgroundAttachment(a));
                    }
                }
                "border-box" | "padding-box" | "content-box" => {
                    if let Some(c) = crate::values::BackgroundClip::parse(k) {
                        clip = Some(Value::BackgroundClip(c));
                    }
                }
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                _ => {}
            },
            Value::Display(Display::None) => {
                image = Some(Value::Keyword("none".to_string()));
                if color.is_none() {
                    color = Some(Value::Color(mango_core::Color::TRANSPARENT));
                }
            }
            _ => {}
        }
    }

    if let Some(c) = color {
        decls.push(Declaration::new("background-color", c, important));
    }
    if let Some(img) = image {
        decls.push(Declaration::new("background-image", img, important));
    }
    if let Some(grad) = gradient {
        decls.push(Declaration::new("background-gradient", grad, important));
    }
    if let Some(r) = repeat {
        decls.push(Declaration::new("background-repeat", r, important));
    }
    if let Some(s) = size {
        decls.push(Declaration::new("background-size", s, important));
    } else if !sizes.is_empty() {
        if sizes.len() == 1 {
            decls.push(Declaration::new(
                "background-size",
                sizes[0].clone(),
                important,
            ));
        } else {
            decls.push(Declaration::new(
                "background-size",
                Value::List(sizes),
                important,
            ));
        }
    }
    if !positions.is_empty() {
        if positions.len() == 1 {
            decls.push(Declaration::new(
                "background-position",
                positions[0].clone(),
                important,
            ));
        } else {
            decls.push(Declaration::new(
                "background-position",
                Value::List(positions),
                important,
            ));
        }
    }
    if let Some(a) = attachment {
        decls.push(Declaration::new("background-attachment", a, important));
    }
    if let Some(c) = clip {
        decls.push(Declaration::new("background-clip", c, important));
    }

    if decls.is_empty() {
        vec![Declaration::new("background", value.clone(), important)]
    } else {
        decls
    }
}

/// Expands `border`, `border-top`, etc. into width, style, color across the specified edges.
fn expand_border_sides(sides: &[&str], value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(len) => {
                if let Length::Px(px) = len
                    && *px == 0.0
                {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(item.clone());
            }
            Value::Number(n) => {
                if *n == 0.0 {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(Value::Length(Length::Px(*n)));
            }
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Var { .. } if color.is_none() => {
                color = Some(item.clone());
            }
            Value::Display(Display::None) => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                    width = Some(Value::Length(Length::Px(0.0)));
                }
                "solid" | "groove" | "ridge" | "inset" | "outset" => {
                    style = Some(Value::BorderStyle(BorderStyle::Solid));
                }
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    let is_none_style = style
        .as_ref()
        .map(|s| {
            matches!(
                s,
                Value::BorderStyle(BorderStyle::None) | Value::BorderStyle(BorderStyle::Hidden)
            )
        })
        .unwrap_or(false)
        || width
            .as_ref()
            .map(|w| matches!(w, Value::Length(Length::Px(p)) if *p == 0.0))
            .unwrap_or(false);
    let default_width = if is_none_style {
        Length::Px(0.0)
    } else {
        Length::Px(1.0)
    };
    let default_style = if is_none_style {
        BorderStyle::None
    } else {
        BorderStyle::Solid
    };
    let default_color = if is_none_style {
        Value::Color(mango_core::Color::TRANSPARENT)
    } else {
        Value::CurrentColor
    };

    let width_val = width.unwrap_or(Value::Length(default_width));
    let style_val = style.unwrap_or(Value::BorderStyle(default_style));
    let color_val = color.unwrap_or(default_color);

    let mut decls = Vec::with_capacity(sides.len() * 3);
    for side in sides {
        decls.push(Declaration::new(
            format!("border-{side}-width"),
            width_val.clone(),
            important,
        ));
        decls.push(Declaration::new(
            format!("border-{side}-style"),
            style_val.clone(),
            important,
        ));
        decls.push(Declaration::new(
            format!("border-{side}-color"),
            color_val.clone(),
            important,
        ));
    }
    decls
}

/// Expands `text-emphasis: <style> || <color>` into `text-emphasis-style` and `text-emphasis-color`.
fn expand_text_emphasis_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    use crate::values::TextEmphasisStyle;
    let mut style_val = None;
    let mut color_val = None;

    let items = match value {
        Value::List(l) => l.clone(),
        single => vec![single.clone()],
    };

    let mut words = Vec::new();
    for item in &items {
        match item {
            Value::Color(c) => color_val = Some(Value::Color(*c)),
            Value::TextEmphasisStyle(tes) => {
                style_val = Some(Value::TextEmphasisStyle(tes.clone()))
            }
            Value::Keyword(k) => {
                if let Some(c) = Value::parse_color(k) {
                    color_val = Some(Value::Color(c));
                } else {
                    words.push(k.as_str());
                }
            }
            Value::String(s) => {
                if let Some(tes) = TextEmphasisStyle::parse(s) {
                    style_val = Some(Value::TextEmphasisStyle(tes));
                } else {
                    words.push(s.as_str());
                }
            }
            Value::ListStyleType(crate::values::ListStyleType::Circle) => {
                words.push("circle");
            }
            Value::ListStyleType(crate::values::ListStyleType::Disc) => {
                words.push("dot");
            }
            Value::ListStyleType(crate::values::ListStyleType::Square) => {
                words.push("square");
            }
            _ => {}
        }
    }

    if style_val.is_none() && !words.is_empty() {
        let combined = words.join(" ");
        if let Some(tes) = TextEmphasisStyle::parse(&combined) {
            style_val = Some(Value::TextEmphasisStyle(tes));
        } else {
            for word in &words {
                if let Some(tes) = TextEmphasisStyle::parse(word) {
                    style_val = Some(Value::TextEmphasisStyle(tes));
                    break;
                }
            }
        }
    }

    let mut decls = Vec::new();
    if let Some(sv) = style_val {
        decls.push(Declaration::new("text-emphasis-style", sv, important));
    }
    if let Some(cv) = color_val {
        decls.push(Declaration::new("text-emphasis-color", cv, important));
    }
    if decls.is_empty() {
        vec![Declaration::new(
            "text-emphasis-style",
            Value::Keyword("none".to_string()),
            important,
        )]
    } else {
        decls
    }
}

/// Expands `flex: <grow> <shrink> <basis>` or keywords.
fn expand_flex_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    if items.len() == 1 {
        match items[0] {
            Value::Keyword(ref k) if k == "none" => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(0.0), important),
                    Declaration::new("flex-shrink", Value::Number(0.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Auto), important),
                ];
            }
            Value::Keyword(ref k) if k == "auto" => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(1.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Auto), important),
                ];
            }
            Value::Number(n) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(n), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
                ];
            }
            Value::Length(Length::Px(0.0)) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(0.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
                ];
            }
            Value::Length(len) => {
                return vec![
                    Declaration::new("flex-grow", Value::Number(1.0), important),
                    Declaration::new("flex-shrink", Value::Number(1.0), important),
                    Declaration::new("flex-basis", Value::Length(len), important),
                ];
            }
            _ => {}
        }
    } else if items.len() == 2 {
        let grow = match items[0] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let shrink = match items[1] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        return vec![
            Declaration::new("flex-grow", Value::Number(grow), important),
            Declaration::new("flex-shrink", Value::Number(shrink), important),
            Declaration::new("flex-basis", Value::Length(Length::Px(0.0)), important),
        ];
    } else if items.len() >= 3 {
        let grow = match items[0] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let shrink = match items[1] {
            Value::Number(n) => n,
            Value::Length(Length::Px(px)) => px,
            _ => 1.0,
        };
        let basis = match items[2] {
            Value::Length(len) => len,
            _ => Length::Auto,
        };
        return vec![
            Declaration::new("flex-grow", Value::Number(grow), important),
            Declaration::new("flex-shrink", Value::Number(shrink), important),
            Declaration::new("flex-basis", Value::Length(basis), important),
        ];
    }

    vec![Declaration::new("flex", value.clone(), important)]
}

/// Expands `flex-flow: <'flex-direction'> || <'flex-wrap'>`
fn expand_flex_flow_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut dir = None;
    let mut wrap = None;

    for item in items {
        match item {
            Value::FlexDirection(fd) => dir = Some(Value::FlexDirection(*fd)),
            Value::FlexWrap(fw) => wrap = Some(Value::FlexWrap(*fw)),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "row" => dir = Some(Value::FlexDirection(crate::values::FlexDirection::Row)),
                "row-reverse" => {
                    dir = Some(Value::FlexDirection(
                        crate::values::FlexDirection::RowReverse,
                    ))
                }
                "column" => dir = Some(Value::FlexDirection(crate::values::FlexDirection::Column)),
                "column-reverse" => {
                    dir = Some(Value::FlexDirection(
                        crate::values::FlexDirection::ColumnReverse,
                    ))
                }
                "nowrap" => wrap = Some(Value::FlexWrap(crate::values::FlexWrap::NoWrap)),
                "wrap" => wrap = Some(Value::FlexWrap(crate::values::FlexWrap::Wrap)),
                "wrap-reverse" => {
                    wrap = Some(Value::FlexWrap(crate::values::FlexWrap::WrapReverse))
                }
                _ => {}
            },
            _ => {}
        }
    }

    let dir_val = dir.unwrap_or(Value::FlexDirection(crate::values::FlexDirection::Row));
    let wrap_val = wrap.unwrap_or(Value::FlexWrap(crate::values::FlexWrap::NoWrap));

    vec![
        Declaration::new("flex-direction", dir_val, important),
        Declaration::new("flex-wrap", wrap_val, important),
    ]
}

/// Expands `gap: <row-gap> <column-gap>`.
fn expand_gap_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let (row, col) = match items.len() {
        1 => (items[0].clone(), items[0].clone()),
        2.. => (items[0].clone(), items[1].clone()),
        _ => return vec![Declaration::new("gap", value.clone(), important)],
    };

    vec![
        Declaration::new("row-gap", row, important),
        Declaration::new("column-gap", col, important),
    ]
}

/// Expands place-items, place-content, or place-self shorthand.
fn expand_place_shorthand(
    align_prop: &str,
    justify_prop: &str,
    value: &Value,
    important: bool,
) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let (align, justify) = match items.len() {
        1 => (items[0].clone(), items[0].clone()),
        2.. => (items[0].clone(), items[1].clone()),
        _ => return vec![Declaration::new(align_prop, value.clone(), important)],
    };

    vec![
        Declaration::new(align_prop, align, important),
        Declaration::new(justify_prop, justify, important),
    ]
}

/// Expands `column-rule: <width> || <style> || <color>`.
fn expand_column_rule_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(_) => width = Some(item.clone()),
            Value::Number(n) if *n == 0.0 => width = Some(Value::Length(Length::Px(0.0))),
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => style = Some(Value::BorderStyle(BorderStyle::None)),
                "solid" => style = Some(Value::BorderStyle(BorderStyle::Solid)),
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "groove" => style = Some(Value::BorderStyle(BorderStyle::Groove)),
                "ridge" => style = Some(Value::BorderStyle(BorderStyle::Ridge)),
                "inset" => style = Some(Value::BorderStyle(BorderStyle::Inset)),
                "outset" => style = Some(Value::BorderStyle(BorderStyle::Outset)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    vec![
        Declaration::new(
            "column-rule-width",
            width.unwrap_or(Value::Length(Length::Px(1.0))),
            important,
        ),
        Declaration::new(
            "column-rule-style",
            style.unwrap_or(Value::BorderStyle(BorderStyle::Solid)),
            important,
        ),
        Declaration::new(
            "column-rule-color",
            color.unwrap_or(Value::Color(mango_core::Color::BLACK)),
            important,
        ),
    ]
}

/// Expands `outline: 1px solid red`, `outline: none`, `outline: 0`, etc.
fn expand_outline_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut width = None;
    let mut style = None;
    let mut color = None;

    for item in items {
        match item {
            Value::Length(len) => {
                if let Length::Px(px) = len
                    && *px == 0.0
                {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                }
                width = Some(item.clone());
            }
            Value::Number(n) if *n == 0.0 => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::BorderStyle(_) => style = Some(item.clone()),
            Value::Color(_) => color = Some(item.clone()),
            Value::Var { .. } if color.is_none() => {
                color = Some(item.clone());
            }
            Value::Display(Display::None) => {
                style = Some(Value::BorderStyle(BorderStyle::None));
                width = Some(Value::Length(Length::Px(0.0)));
            }
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" | "hidden" => {
                    style = Some(Value::BorderStyle(BorderStyle::None));
                    width = Some(Value::Length(Length::Px(0.0)));
                }
                "solid" | "groove" | "ridge" | "inset" | "outset" => {
                    style = Some(Value::BorderStyle(BorderStyle::Solid));
                }
                "dashed" => style = Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => style = Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => style = Some(Value::BorderStyle(BorderStyle::Double)),
                "thin" => width = Some(Value::Length(Length::Px(1.0))),
                "medium" => width = Some(Value::Length(Length::Px(3.0))),
                "thick" => width = Some(Value::Length(Length::Px(5.0))),
                "transparent" => color = Some(Value::Color(mango_core::Color::TRANSPARENT)),
                "currentcolor" => color = Some(Value::CurrentColor),
                "invert" => color = Some(Value::Color(mango_core::Color::BLACK)),
                _ => {
                    if let Some(c) = Value::parse_color(k) {
                        color = Some(Value::Color(c));
                    }
                }
            },
            _ => {}
        }
    }

    let is_none_style = style
        .as_ref()
        .map(|s| {
            matches!(
                s,
                Value::BorderStyle(BorderStyle::None) | Value::BorderStyle(BorderStyle::Hidden)
            )
        })
        .unwrap_or(false)
        || width
            .as_ref()
            .map(|w| matches!(w, Value::Length(Length::Px(p)) if *p == 0.0))
            .unwrap_or(false);
    let default_width = if is_none_style {
        Length::Px(0.0)
    } else {
        Length::Px(1.0)
    };
    let default_style = if is_none_style {
        BorderStyle::None
    } else {
        BorderStyle::Solid
    };
    let default_color = if is_none_style {
        mango_core::Color::TRANSPARENT
    } else {
        mango_core::Color::BLACK
    };

    let width_val = width.unwrap_or(Value::Length(default_width));
    let style_val = style.unwrap_or(Value::BorderStyle(default_style));
    let color_val = color.unwrap_or(Value::Color(default_color));

    vec![
        Declaration::new("outline-width", width_val, important),
        Declaration::new("outline-style", style_val, important),
        Declaration::new("outline-color", color_val, important),
    ]
}

/// Expands `columns: <column-width> || <column-count>` (e.g. `columns: 2`, `columns: 20em`, `columns: 2 20em`, `columns: auto`).
fn expand_columns_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut col_width = None;
    let mut col_count = None;

    for item in items {
        match item {
            Value::Length(len) => {
                col_width = Some(Value::Length(*len));
            }
            Value::Number(n) => {
                col_count = Some(Value::Number(*n));
            }
            Value::Keyword(k) if k.eq_ignore_ascii_case("auto") => {
                if col_width.is_none() {
                    col_width = Some(Value::Keyword("auto".to_string()));
                } else {
                    col_count = Some(Value::Keyword("auto".to_string()));
                }
            }
            _ => {}
        }
    }

    let width_val = col_width.unwrap_or(Value::Keyword("auto".to_string()));
    let count_val = col_count.unwrap_or(Value::Keyword("auto".to_string()));

    vec![
        Declaration::new("column-width", width_val, important),
        Declaration::new("column-count", count_val, important),
    ]
}

/// Expands `mask` or `-webkit-mask` shorthand into mask-image, mask-mode, mask-repeat, mask-position, mask-size.
fn expand_mask_shorthand(prefix: &str, value: &Value, important: bool) -> Vec<Declaration> {
    let items = match value {
        Value::List(list) => list.as_slice(),
        single => std::slice::from_ref(single),
    };

    let mut decls = Vec::new();
    let mut image = None;
    let mut mode = None;
    let mut repeat = None;
    let mut size = None;
    let mut positions = Vec::new();

    let img_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-image"
    } else {
        "mask-image"
    };
    let mode_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-mode"
    } else {
        "mask-mode"
    };
    let repeat_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-repeat"
    } else {
        "mask-repeat"
    };
    let size_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-size"
    } else {
        "mask-size"
    };
    let pos_prop = if prefix.starts_with("-webkit") {
        "-webkit-mask-position"
    } else {
        "mask-position"
    };

    for item in items {
        match item {
            Value::Url(_) | Value::Gradient(_) => image = Some(item.clone()),
            Value::MaskMode(_) => mode = Some(item.clone()),
            Value::BackgroundRepeat(_) => repeat = Some(item.clone()),
            Value::BackgroundSize(_) => size = Some(item.clone()),
            Value::Length(_) | Value::Percentage(_) => positions.push(item.clone()),
            Value::Keyword(k) => match k.to_ascii_lowercase().as_str() {
                "none" => image = Some(Value::Keyword("none".to_string())),
                "alpha" | "luminance" | "match-source" => {
                    if let Some(m) = crate::values::MaskMode::parse(k) {
                        mode = Some(Value::MaskMode(m));
                    }
                }
                "repeat" | "repeat-x" | "repeat-y" | "no-repeat" => repeat = Some(item.clone()),
                "cover" | "contain" => size = Some(item.clone()),
                _ => {}
            },
            _ => {}
        }
    }

    if let Some(img) = image {
        decls.push(Declaration::new(img_prop, img, important));
    }
    if let Some(m) = mode {
        decls.push(Declaration::new(mode_prop, m, important));
    }
    if let Some(r) = repeat {
        decls.push(Declaration::new(repeat_prop, r, important));
    }
    if let Some(s) = size {
        decls.push(Declaration::new(size_prop, s, important));
    }
    if !positions.is_empty() {
        if positions.len() == 1 {
            decls.push(Declaration::new(pos_prop, positions[0].clone(), important));
        } else {
            decls.push(Declaration::new(
                pos_prop,
                Value::List(positions),
                important,
            ));
        }
    }

    if decls.is_empty() {
        vec![Declaration::new(prefix, value.clone(), important)]
    } else {
        decls
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Color;

    #[test]
    fn test_expand_flex_and_gap_shorthands() {
        let flex_decl = Declaration::new("flex", Value::Number(1.0), false);
        let expanded = flex_decl.expand_shorthand();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "flex-grow");
        assert_eq!(expanded[0].value, Value::Number(1.0));
        assert_eq!(expanded[1].name, "flex-shrink");
        assert_eq!(expanded[1].value, Value::Number(1.0));
        assert_eq!(expanded[2].name, "flex-basis");
        assert_eq!(expanded[2].value, Value::Length(Length::Px(0.0)));

        let gap_decl = Declaration::new(
            "gap",
            Value::List(vec![
                Value::Length(Length::Px(10.0)),
                Value::Length(Length::Px(20.0)),
            ]),
            false,
        );
        let expanded_gap = gap_decl.expand_shorthand();
        assert_eq!(expanded_gap.len(), 2);
        assert_eq!(expanded_gap[0].name, "row-gap");
        assert_eq!(expanded_gap[0].value, Value::Length(Length::Px(10.0)));
        assert_eq!(expanded_gap[1].name, "column-gap");
        assert_eq!(expanded_gap[1].value, Value::Length(Length::Px(20.0)));
    }

    #[test]
    fn test_expand_margin_shorthand() {
        let decl = Declaration::new(
            "margin",
            Value::List(vec![
                Value::Length(Length::Px(10.0)),
                Value::Length(Length::Px(20.0)),
            ]),
            false,
        );
        let expanded = decl.expand_shorthand();
        assert_eq!(expanded.len(), 4);
        assert_eq!(expanded[0].name, "margin-top");
        assert_eq!(expanded[0].value, Value::Length(Length::Px(10.0)));
        assert_eq!(expanded[1].name, "margin-right");
        assert_eq!(expanded[1].value, Value::Length(Length::Px(20.0)));
        assert_eq!(expanded[2].name, "margin-bottom");
        assert_eq!(expanded[2].value, Value::Length(Length::Px(10.0)));
        assert_eq!(expanded[3].name, "margin-left");
        assert_eq!(expanded[3].value, Value::Length(Length::Px(20.0)));
    }

    #[test]
    fn test_expand_border_shorthand() {
        let decl = Declaration::new(
            "border",
            Value::List(vec![
                Value::Length(Length::Px(2.0)),
                Value::BorderStyle(BorderStyle::Solid),
                Value::Color(Color::RED),
            ]),
            true,
        );
        let expanded = decl.expand_shorthand();
        assert_eq!(expanded.len(), 12);
        assert!(expanded.iter().all(|d| d.important));
        assert_eq!(expanded[0].name, "border-top-width");
        assert_eq!(expanded[0].value, Value::Length(Length::Px(2.0)));
    }

    #[test]
    fn test_expand_outline_shorthand() {
        let decl = Declaration::new(
            "outline",
            Value::List(vec![
                Value::Length(Length::Px(2.0)),
                Value::BorderStyle(BorderStyle::Solid),
                Value::Color(Color::RED),
            ]),
            false,
        );
        let expanded = decl.expand_shorthand();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "outline-width");
        assert_eq!(expanded[0].value, Value::Length(Length::Px(2.0)));
        assert_eq!(expanded[1].name, "outline-style");
        assert_eq!(expanded[1].value, Value::BorderStyle(BorderStyle::Solid));
        assert_eq!(expanded[2].name, "outline-color");
        assert_eq!(expanded[2].value, Value::Color(Color::RED));

        let none_decl = Declaration::new("outline", Value::Keyword("none".to_string()), false);
        let expanded_none = none_decl.expand_shorthand();
        assert_eq!(expanded_none.len(), 3);
        assert_eq!(expanded_none[0].value, Value::Length(Length::Px(0.0)));
        assert_eq!(
            expanded_none[1].value,
            Value::BorderStyle(BorderStyle::None)
        );
    }

    #[test]
    fn test_expand_columns_shorthand() {
        let decl = Declaration::new(
            "columns",
            Value::List(vec![Value::Length(Length::Px(250.0)), Value::Number(2.0)]),
            false,
        );
        let expanded = decl.expand_shorthand();
        assert_eq!(expanded.len(), 2);
        assert_eq!(expanded[0].name, "column-width");
        assert_eq!(expanded[0].value, Value::Length(Length::Px(250.0)));
        assert_eq!(expanded[1].name, "column-count");
        assert_eq!(expanded[1].value, Value::Number(2.0));
    }

    #[test]
    fn test_expand_mask_shorthand() {
        let decl = Declaration::new(
            "mask",
            Value::List(vec![
                Value::Url("mask.png".to_string()),
                Value::Keyword("alpha".to_string()),
                Value::Keyword("no-repeat".to_string()),
            ]),
            false,
        );
        let expanded = decl.expand_shorthand();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "mask-image");
        assert_eq!(expanded[0].value, Value::Url("mask.png".to_string()));
        assert_eq!(expanded[1].name, "mask-mode");
        assert_eq!(
            expanded[1].value,
            Value::MaskMode(crate::values::MaskMode::Alpha)
        );
        assert_eq!(expanded[2].name, "mask-repeat");
    }
}
