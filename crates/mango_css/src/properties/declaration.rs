//! CSS property declaration and shorthand expansion dispatch.

use super::shorthands::*;
use crate::values::Value;

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
        let name_str = name.into();
        let name = if name_str.starts_with("--") {
            name_str
        } else {
            name_str.to_ascii_lowercase()
        };
        Self {
            name,
            value,
            important,
        }
    }

    /// Returns true if the property name is a recognized CSS shorthand.
    #[inline]
    pub fn is_shorthand_name(name: &str) -> bool {
        matches!(
            name,
            "margin"
                | "margin-inline"
                | "margin-block"
                | "margin-inline-start"
                | "margin-inline-end"
                | "margin-block-start"
                | "margin-block-end"
                | "padding"
                | "padding-inline"
                | "padding-block"
                | "padding-inline-start"
                | "padding-inline-end"
                | "padding-block-start"
                | "padding-block-end"
                | "inset"
                | "inset-inline"
                | "inset-block"
                | "border"
                | "border-top"
                | "border-right"
                | "border-bottom"
                | "border-left"
                | "border-block"
                | "border-inline"
                | "border-block-start"
                | "border-block-end"
                | "border-inline-start"
                | "border-inline-end"
                | "border-width"
                | "border-style"
                | "border-color"
                | "border-block-width"
                | "border-block-style"
                | "border-block-color"
                | "border-inline-width"
                | "border-inline-style"
                | "border-inline-color"
                | "border-block-start-width"
                | "border-block-end-width"
                | "border-inline-start-width"
                | "border-inline-end-width"
                | "border-block-start-style"
                | "border-block-end-style"
                | "border-inline-start-style"
                | "border-inline-end-style"
                | "border-block-start-color"
                | "border-block-end-color"
                | "border-inline-start-color"
                | "border-inline-end-color"
                | "border-start-start-radius"
                | "border-start-end-radius"
                | "border-end-start-radius"
                | "border-end-end-radius"
                | "inline-size"
                | "block-size"
                | "min-inline-size"
                | "max-inline-size"
                | "min-block-size"
                | "max-block-size"
                | "background"
                | "font"
                | "flex"
                | "flex-flow"
                | "gap"
                | "border-radius"
                | "overflow"
                | "overflow-inline"
                | "overflow-block"
                | "list-style"
                | "outline"
                | "columns"
                | "mask"
                | "-webkit-mask"
                | "grid-column"
                | "grid-row"
                | "grid-area"
                | "grid-template"
                | "place-items"
                | "place-content"
                | "place-self"
                | "column-rule"
                | "text-emphasis"
                | "container"
                | "contain-intrinsic-size"
                | "all"
        )
    }

    /// Returns true if this declaration is a shorthand property.
    #[inline]
    pub fn is_shorthand(&self) -> bool {
        Self::is_shorthand_name(&self.name)
    }

    /// Expands shorthand properties into individual longhand declarations by reference.
    pub fn expand_shorthand_ref(&self) -> Vec<Declaration> {
        let important = self.important;
        match self.name.as_str() {
            "all" => expand_all_shorthand(&self.value, important),
            "margin" => expand_four_sides("margin", &self.value, important),
            "margin-inline" => expand_inline_shorthand("margin", &self.value, important),
            "margin-block" => expand_block_shorthand("margin", &self.value, important),
            "margin-inline-start" => vec![Declaration::new(
                "margin-left",
                self.value.clone(),
                important,
            )],
            "margin-inline-end" => vec![Declaration::new(
                "margin-right",
                self.value.clone(),
                important,
            )],
            "margin-block-start" => vec![Declaration::new(
                "margin-top",
                self.value.clone(),
                important,
            )],
            "margin-block-end" => vec![Declaration::new(
                "margin-bottom",
                self.value.clone(),
                important,
            )],

            "padding" => expand_four_sides("padding", &self.value, important),
            "padding-inline" => expand_inline_shorthand("padding", &self.value, important),
            "padding-block" => expand_block_shorthand("padding", &self.value, important),
            "padding-inline-start" => vec![Declaration::new(
                "padding-left",
                self.value.clone(),
                important,
            )],
            "padding-inline-end" => vec![Declaration::new(
                "padding-right",
                self.value.clone(),
                important,
            )],
            "padding-block-start" => vec![Declaration::new(
                "padding-top",
                self.value.clone(),
                important,
            )],
            "padding-block-end" => vec![Declaration::new(
                "padding-bottom",
                self.value.clone(),
                important,
            )],

            "inset" => expand_inset_shorthand(&self.value, important),
            "inset-inline" => expand_inset_inline_shorthand(&self.value, important),
            "inset-block" => expand_inset_block_shorthand(&self.value, important),

            "border" => {
                expand_border_sides(&["top", "right", "bottom", "left"], &self.value, important)
            }
            "border-top" => expand_border_sides(&["top"], &self.value, important),
            "border-right" => expand_border_sides(&["right"], &self.value, important),
            "border-bottom" => expand_border_sides(&["bottom"], &self.value, important),
            "border-left" => expand_border_sides(&["left"], &self.value, important),
            "border-block" => expand_border_sides(&["top", "bottom"], &self.value, important),
            "border-inline" => expand_border_sides(&["left", "right"], &self.value, important),
            "border-block-start" => expand_border_sides(&["top"], &self.value, important),
            "border-block-end" => expand_border_sides(&["bottom"], &self.value, important),
            "border-inline-start" => expand_border_sides(&["left"], &self.value, important),
            "border-inline-end" => expand_border_sides(&["right"], &self.value, important),

            "border-width" => expand_four_sides_scalar("border", "width", &self.value, important),
            "border-style" => expand_four_sides_scalar("border", "style", &self.value, important),
            "border-color" => expand_four_sides_scalar("border", "color", &self.value, important),
            "border-block-width" => {
                expand_two_sides_scalar("border", "top", "bottom", "width", &self.value, important)
            }
            "border-block-style" => {
                expand_two_sides_scalar("border", "top", "bottom", "style", &self.value, important)
            }
            "border-block-color" => {
                expand_two_sides_scalar("border", "top", "bottom", "color", &self.value, important)
            }
            "border-inline-width" => {
                expand_two_sides_scalar("border", "left", "right", "width", &self.value, important)
            }
            "border-inline-style" => {
                expand_two_sides_scalar("border", "left", "right", "style", &self.value, important)
            }
            "border-inline-color" => {
                expand_two_sides_scalar("border", "left", "right", "color", &self.value, important)
            }
            "border-block-start-width" => vec![Declaration::new(
                "border-top-width",
                self.value.clone(),
                important,
            )],
            "border-block-end-width" => vec![Declaration::new(
                "border-bottom-width",
                self.value.clone(),
                important,
            )],
            "border-inline-start-width" => vec![Declaration::new(
                "border-left-width",
                self.value.clone(),
                important,
            )],
            "border-inline-end-width" => vec![Declaration::new(
                "border-right-width",
                self.value.clone(),
                important,
            )],
            "border-block-start-style" => vec![Declaration::new(
                "border-top-style",
                self.value.clone(),
                important,
            )],
            "border-block-end-style" => vec![Declaration::new(
                "border-bottom-style",
                self.value.clone(),
                important,
            )],
            "border-inline-start-style" => vec![Declaration::new(
                "border-left-style",
                self.value.clone(),
                important,
            )],
            "border-inline-end-style" => vec![Declaration::new(
                "border-right-style",
                self.value.clone(),
                important,
            )],
            "border-block-start-color" => vec![Declaration::new(
                "border-top-color",
                self.value.clone(),
                important,
            )],
            "border-block-end-color" => vec![Declaration::new(
                "border-bottom-color",
                self.value.clone(),
                important,
            )],
            "border-inline-start-color" => vec![Declaration::new(
                "border-left-color",
                self.value.clone(),
                important,
            )],
            "border-inline-end-color" => vec![Declaration::new(
                "border-right-color",
                self.value.clone(),
                important,
            )],

            "border-start-start-radius" => vec![Declaration::new(
                "border-top-left-radius",
                self.value.clone(),
                important,
            )],
            "border-start-end-radius" => vec![Declaration::new(
                "border-top-right-radius",
                self.value.clone(),
                important,
            )],
            "border-end-start-radius" => vec![Declaration::new(
                "border-bottom-left-radius",
                self.value.clone(),
                important,
            )],
            "border-end-end-radius" => vec![Declaration::new(
                "border-bottom-right-radius",
                self.value.clone(),
                important,
            )],

            "inline-size" => vec![Declaration::new("width", self.value.clone(), important)],
            "block-size" => vec![Declaration::new("height", self.value.clone(), important)],
            "min-inline-size" => vec![Declaration::new("min-width", self.value.clone(), important)],
            "max-inline-size" => vec![Declaration::new("max-width", self.value.clone(), important)],
            "min-block-size" => vec![Declaration::new(
                "min-height",
                self.value.clone(),
                important,
            )],
            "max-block-size" => vec![Declaration::new(
                "max-height",
                self.value.clone(),
                important,
            )],

            "overflow-inline" => vec![Declaration::new(
                "overflow-x",
                self.value.clone(),
                important,
            )],
            "overflow-block" => vec![Declaration::new(
                "overflow-y",
                self.value.clone(),
                important,
            )],

            "background" => expand_background_shorthand(&self.value, important),
            "font" => expand_font_shorthand(&self.value, important),

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
            "contain-intrinsic-size" => {
                expand_contain_intrinsic_size_shorthand(&self.value, important)
            }

            _ => vec![self.clone()],
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
        self.expand_shorthand_ref()
    }
}

const ALL_SHORTHAND_LONGHANDS: &[&str] = &[
    "color",
    "background-color",
    "background-image",
    "background-repeat",
    "background-size",
    "background-position",
    "background-attachment",
    "background-clip",
    "font-size",
    "font-weight",
    "font-style",
    "font-family",
    "line-height",
    "text-align",
    "text-decoration",
    "text-decoration-thickness",
    "text-transform",
    "letter-spacing",
    "word-spacing",
    "text-indent",
    "text-overflow",
    "text-shadow",
    "text-wrap",
    "white-space",
    "word-break",
    "overflow-wrap",
    "hyphens",
    "cursor",
    "list-style-type",
    "list-style-position",
    "visibility",
    "opacity",
    "display",
    "position",
    "float",
    "clear",
    "z-index",
    "vertical-align",
    "box-sizing",
    "width",
    "height",
    "min-width",
    "max-width",
    "min-height",
    "max-height",
    "top",
    "right",
    "bottom",
    "left",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-top-style",
    "border-right-style",
    "border-bottom-style",
    "border-left-style",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
    "overflow-x",
    "overflow-y",
    "box-shadow",
    "outline-width",
    "outline-style",
    "outline-color",
    "outline-offset",
    "flex-direction",
    "flex-wrap",
    "flex-grow",
    "flex-shrink",
    "flex-basis",
    "order",
    "align-items",
    "align-self",
    "align-content",
    "justify-content",
    "justify-items",
    "justify-self",
    "row-gap",
    "column-gap",
    "grid-template-columns",
    "grid-template-rows",
    "grid-auto-flow",
    "grid-column-start",
    "grid-column-end",
    "grid-row-start",
    "grid-row-end",
    "aspect-ratio",
    "object-fit",
    "object-position",
    "will-change",
    "scrollbar-width",
    "scrollbar-color",
    "transform",
    "filter",
    "backdrop-filter",
    "quotes",
    "accent-color",
    "appearance",
    "border-collapse",
    "border-spacing",
    "table-layout",
    "caption-side",
    "resize",
];

pub fn expand_all_shorthand(value: &Value, important: bool) -> Vec<Declaration> {
    ALL_SHORTHAND_LONGHANDS
        .iter()
        .map(|&prop| Declaration::new(prop, value.clone(), important))
        .collect()
}
