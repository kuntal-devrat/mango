use super::*;
use crate::values::{BorderStyle, Length, Value};
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
    // CSS Flexbox Level 1 § 7.1: an omitted flex-basis defaults to 0%.
    assert_eq!(expanded[2].value, Value::Length(Length::Percent(0.0)));

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

#[test]
fn test_expand_inset_shorthands() {
    // 1 value -> all 4 sides
    let decl1 = Declaration::new("inset", Value::Length(Length::Px(10.0)), false);
    let exp1 = decl1.expand_shorthand();
    assert_eq!(exp1.len(), 4);
    assert_eq!(exp1[0].name, "top");
    assert_eq!(exp1[0].value, Value::Length(Length::Px(10.0)));
    assert_eq!(exp1[1].name, "right");
    assert_eq!(exp1[2].name, "bottom");
    assert_eq!(exp1[3].name, "left");

    // 2 values -> top/bottom, left/right
    let decl2 = Declaration::new(
        "inset",
        Value::List(vec![
            Value::Length(Length::Px(5.0)),
            Value::Length(Length::Px(15.0)),
        ]),
        false,
    );
    let exp2 = decl2.expand_shorthand();
    assert_eq!(exp2.len(), 4);
    assert_eq!(exp2[0].name, "top");
    assert_eq!(exp2[0].value, Value::Length(Length::Px(5.0)));
    assert_eq!(exp2[1].name, "right");
    assert_eq!(exp2[1].value, Value::Length(Length::Px(15.0)));
    assert_eq!(exp2[2].name, "bottom");
    assert_eq!(exp2[2].value, Value::Length(Length::Px(5.0)));
    assert_eq!(exp2[3].name, "left");
    assert_eq!(exp2[3].value, Value::Length(Length::Px(15.0)));

    // inset-inline
    let decl_inline = Declaration::new(
        "inset-inline",
        Value::List(vec![
            Value::Length(Length::Px(8.0)),
            Value::Length(Length::Px(12.0)),
        ]),
        false,
    );
    let exp_inline = decl_inline.expand_shorthand();
    assert_eq!(exp_inline.len(), 2);
    // Logical properties are resolved to physical offsets at computed-value time,
    // so the shorthand expands to the logical longhands.
    assert_eq!(exp_inline[0].name, "inset-inline-start");
    assert_eq!(exp_inline[0].value, Value::Length(Length::Px(8.0)));
    assert_eq!(exp_inline[1].name, "inset-inline-end");
    assert_eq!(exp_inline[1].value, Value::Length(Length::Px(12.0)));

    // inset-block
    let decl_block = Declaration::new("inset-block", Value::Length(Length::Px(20.0)), false);
    let exp_block = decl_block.expand_shorthand();
    assert_eq!(exp_block.len(), 2);
    assert_eq!(exp_block[0].name, "inset-block-start");
    assert_eq!(exp_block[0].value, Value::Length(Length::Px(20.0)));
    assert_eq!(exp_block[1].name, "inset-block-end");
    assert_eq!(exp_block[1].value, Value::Length(Length::Px(20.0)));
}

#[test]
fn test_expand_font_shorthand() {
    let decl = Declaration::new(
        "font",
        Value::String("italic bold 16px/1.5 sans-serif".to_string()),
        false,
    );
    let expanded = decl.expand_shorthand();
    let map: std::collections::HashMap<_, _> =
        expanded.into_iter().map(|d| (d.name, d.value)).collect();
    assert_eq!(
        map.get("font-style"),
        Some(&Value::FontStyle(crate::values::FontStyle::Italic))
    );
    assert_eq!(
        map.get("font-weight"),
        Some(&Value::FontWeight(crate::values::FontWeight::Bold))
    );
    assert_eq!(map.get("font-size"), Some(&Value::Length(Length::Px(16.0))));
    assert_eq!(map.get("line-height"), Some(&Value::Number(1.5)));
    assert_eq!(
        map.get("font-family"),
        Some(&Value::String("sans-serif".to_string()))
    );
}

#[test]
fn test_expand_background_shorthand_resets_omitted() {
    let decl = Declaration::new("background", Value::Color(mango_core::Color::RED), false);
    let expanded = decl.expand_shorthand();
    let map: std::collections::HashMap<_, _> =
        expanded.into_iter().map(|d| (d.name, d.value)).collect();
    assert_eq!(
        map.get("background-color"),
        Some(&Value::Color(mango_core::Color::RED))
    );
    assert_eq!(
        map.get("background-image"),
        Some(&Value::Keyword("none".to_string()))
    );
    assert_eq!(
        map.get("background-repeat"),
        Some(&Value::BackgroundRepeat(
            crate::values::BackgroundRepeat::Repeat
        ))
    );
    assert_eq!(
        map.get("background-size"),
        Some(&Value::BackgroundSize(crate::values::BackgroundSize::Auto))
    );
}

#[test]
fn test_expand_logical_box_model_shorthands() {
    let pad_inline = Declaration::new(
        "padding-inline",
        Value::List(vec![
            Value::Length(Length::Px(12.0)),
            Value::Length(Length::Px(16.0)),
        ]),
        false,
    );
    let exp_pi = pad_inline.expand_shorthand();
    assert_eq!(exp_pi.len(), 2);
    assert_eq!(exp_pi[0].name, "padding-left");
    assert_eq!(exp_pi[0].value, Value::Length(Length::Px(12.0)));
    assert_eq!(exp_pi[1].name, "padding-right");
    assert_eq!(exp_pi[1].value, Value::Length(Length::Px(16.0)));

    let pad_block = Declaration::new("padding-block", Value::Length(Length::Px(8.0)), false);
    let exp_pb = pad_block.expand_shorthand();
    assert_eq!(exp_pb.len(), 2);
    assert_eq!(exp_pb[0].name, "padding-top");
    assert_eq!(exp_pb[0].value, Value::Length(Length::Px(8.0)));
    assert_eq!(exp_pb[1].name, "padding-bottom");
    assert_eq!(exp_pb[1].value, Value::Length(Length::Px(8.0)));

    let inline_size = Declaration::new("inline-size", Value::Length(Length::Px(100.0)), false);
    let exp_is = inline_size.expand_shorthand();
    assert_eq!(exp_is.len(), 1);
    assert_eq!(exp_is[0].name, "width");
    assert_eq!(exp_is[0].value, Value::Length(Length::Px(100.0)));
}

#[test]
fn test_expand_border_with_multiple_variables() {
    let decl = Declaration::new(
        "border-top",
        Value::List(vec![
            Value::Var {
                name: "--borderWidth-thin".to_string(),
                fallback: Some(Box::new(Value::Length(Length::Px(1.0)))),
            },
            Value::BorderStyle(BorderStyle::Solid),
            Value::Var {
                name: "--borderColor-muted".to_string(),
                fallback: None,
            },
        ]),
        false,
    );
    let exp = decl.expand_shorthand();
    let map: std::collections::HashMap<_, _> = exp.into_iter().map(|d| (d.name, d.value)).collect();

    assert_eq!(
        map.get("border-top-width"),
        Some(&Value::Var {
            name: "--borderWidth-thin".to_string(),
            fallback: Some(Box::new(Value::Length(Length::Px(1.0)))),
        })
    );
    assert_eq!(
        map.get("border-top-style"),
        Some(&Value::BorderStyle(BorderStyle::Solid))
    );
    assert_eq!(
        map.get("border-top-color"),
        Some(&Value::Var {
            name: "--borderColor-muted".to_string(),
            fallback: None,
        })
    );
}
