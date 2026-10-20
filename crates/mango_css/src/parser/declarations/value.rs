use crate::parser::declarations::functions::{parse_color_fn, parse_url_fn, parse_var_call};
use crate::parser::declarations::gradient::{parse_gradient_fn, token_to_string};
use crate::parser::declarations::math::parse_math_call;
use crate::tokenizer::Token;
use crate::values::{
    AlignItems, BackgroundRepeat, BackgroundSize, BorderCollapse, BorderStyle, BoxSizing,
    BreakInside, Clear, ColumnSpan, Cursor, Direction, Display, FlexDirection, FlexWrap, Float,
    FontStyle, FontWeight, JustifyContent, Length, ListStylePosition, ListStyleType, ObjectFit,
    Overflow, Position, TextAlign, TextDecoration, TextOverflow, TextTransform, UnicodeBidi, Value,
    VerticalAlign, Visibility, WhiteSpace,
};

pub fn parse_value_from_tokens(tokens: &[Token]) -> Option<Value> {
    let filtered: Vec<&Token> = tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .collect();

    if filtered.is_empty() {
        return None;
    }

    // Check if a single var(...) covers all non-whitespace tokens
    if let Some((var_val, consumed)) = parse_var_call(tokens) {
        let remaining = &tokens[consumed..];
        if remaining
            .iter()
            .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
        {
            return Some(var_val);
        }
    }

    // Check if a single url(...) covers all non-whitespace tokens
    if let Some((url_val, consumed)) = parse_url_fn(tokens) {
        let remaining = &tokens[consumed..];
        if remaining
            .iter()
            .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
        {
            return Some(url_val);
        }
    }

    // Check if a single math call calc(...), min(...), max(...), clamp(...) covers all non-whitespace tokens
    if let Some((calc_val, consumed)) = parse_math_call(tokens) {
        let remaining = &tokens[consumed..];
        if remaining
            .iter()
            .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
        {
            return Some(calc_val);
        }
    }

    // Check for gradient functions: linear-gradient / radial-gradient
    if let Some((grad_val, consumed)) = parse_gradient_fn(tokens) {
        let remaining = &tokens[consumed..];
        if remaining
            .iter()
            .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
        {
            return Some(grad_val);
        }
    }

    // NOTE: `transform`, `text-shadow`, and `filter` values are parsed by
    // property-specific branches in `parse_declarations_until`. Parsing them
    // generically here would let e.g. `display: none` or `content: none` be
    // mis-read as `transform: none`.

    if filtered.len() == 1 {
        return parse_single_value(filtered[0]);
    }

    // Check for rgb()/rgba()/hsl()/hsla()/color()/oklab()/oklch()/color-mix() pattern spanning the full value
    if let Token::Ident(fn_name) = filtered[0]
        && (fn_name.eq_ignore_ascii_case("rgb")
            || fn_name.eq_ignore_ascii_case("rgba")
            || fn_name.eq_ignore_ascii_case("hsl")
            || fn_name.eq_ignore_ascii_case("hsla")
            || fn_name.eq_ignore_ascii_case("color")
            || fn_name.eq_ignore_ascii_case("oklab")
            || fn_name.eq_ignore_ascii_case("oklch")
            || fn_name.eq_ignore_ascii_case("color-mix"))
        && filtered.last() == Some(&&Token::CloseParen)
    {
        let mut call_str = fn_name.clone();
        for t in &filtered[1..] {
            call_str.push_str(&token_to_string(t));
        }
        if let Some(col) = Value::parse_color(&call_str) {
            return Some(Value::Color(col));
        }
    }

    // Check for anchor(...) function pattern
    if let Token::Ident(fn_name) = filtered[0]
        && fn_name.eq_ignore_ascii_case("anchor")
        && filtered.last() == Some(&&Token::CloseParen)
    {
        let mut call_str = fn_name.clone();
        for t in &filtered[1..] {
            call_str.push_str(&token_to_string(t));
        }
        if let Some(l) = Length::parse(&call_str) {
            return Some(Value::Length(l));
        }
    }

    // Multi-value list (e.g. `10px 20px`, `1px solid black`, `url(...) no-repeat`)
    let mut list = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
            i += 1;
            continue;
        }

        if let Some((var_val, consumed)) = parse_var_call(&tokens[i..]) {
            list.push(var_val);
            i += consumed;
            continue;
        }

        if let Some((color_val, consumed)) = parse_color_fn(&tokens[i..]) {
            list.push(color_val);
            i += consumed;
            continue;
        }

        if let Some((url_val, consumed)) = parse_url_fn(&tokens[i..]) {
            list.push(url_val);
            i += consumed;
            continue;
        }

        if let Some((grad_val, consumed)) = parse_gradient_fn(&tokens[i..]) {
            list.push(grad_val);
            i += consumed;
            continue;
        }

        if let Some((calc_val, consumed)) = parse_math_call(&tokens[i..]) {
            list.push(calc_val);
            i += consumed;
            continue;
        }

        if matches!(tokens[i], Token::Comma) {
            i += 1;
            continue;
        }

        {
            let val = parse_single_value(&tokens[i])?;
            list.push(val);
        }
        i += 1;
    }

    if list.is_empty() {
        None
    } else if list.len() == 1 {
        Some(list.remove(0))
    } else {
        Some(Value::List(list))
    }
}

pub fn parse_single_value(token: &Token) -> Option<Value> {
    match token {
        Token::Dimension { value, unit } => {
            if unit.eq_ignore_ascii_case("fr") {
                return Some(Value::Fr(*value));
            }
            // CSS time units are normalized to milliseconds (used by transitions/animations)
            match unit.to_ascii_lowercase().as_str() {
                "s" => return Some(Value::Time(*value * 1000.0)),
                "ms" => return Some(Value::Time(*value)),
                "deg" => return Some(Value::Angle(*value)),
                "rad" => return Some(Value::Angle(value.to_degrees())),
                "grad" => return Some(Value::Angle(*value * 0.9)),
                "turn" => return Some(Value::Angle(*value * 360.0)),
                _ => {}
            }
            let len = match unit.to_ascii_lowercase().as_str() {
                "px" => Length::Px(*value),
                "em" => Length::Em(*value),
                "rem" => Length::Rem(*value),
                "vw" => Length::Vw(*value),
                "vh" => Length::Vh(*value),
                "vmin" => Length::Vmin(*value),
                "vmax" => Length::Vmax(*value),
                "pt" => Length::Px(*value * 4.0 / 3.0),
                "in" => Length::Px(*value * 96.0),
                "cm" => Length::Px(*value * 96.0 / 2.54),
                "mm" => Length::Px(*value * 9.6 / 2.54),
                "pc" => Length::Px(*value * 16.0),
                "q" => Length::Px(*value * 96.0 / 101.6),
                "ch" => Length::Em(*value * 0.5),
                "ex" => Length::Em(*value * 0.5),
                _ => return None, // CSS spec requires dropping unknown units
            };
            Some(Value::Length(len))
        }
        Token::Percentage(pct) => Some(Value::Length(Length::Percent(*pct))),
        Token::Number(n) => {
            if *n == 0.0 {
                Some(Value::Length(Length::Px(0.0)))
            } else {
                Some(Value::Number(*n))
            }
        }
        Token::Hash(hex) => {
            let hex_str = format!("#{hex}");
            Value::parse_color(&hex_str).map(Value::Color)
        }
        Token::String(s) => Some(Value::String(s.clone())),
        Token::Ident(word) => {
            let lower = word.to_ascii_lowercase();

            if lower == "currentcolor" {
                return Some(Value::CurrentColor);
            }

            // Timing functions (transition-timing-function / animation-timing-function)
            if let Some(tf) = crate::values::parse_timing_function(&lower) {
                return Some(Value::TimingFunction(tf));
            }

            // Font size keywords
            match lower.as_str() {
                "xx-small" => return Some(Value::Length(Length::Px(9.0))),
                "x-small" => return Some(Value::Length(Length::Px(10.0))),
                "small" => return Some(Value::Length(Length::Px(13.0))),
                "medium" => return Some(Value::Length(Length::Px(16.0))),
                "large" => return Some(Value::Length(Length::Px(18.0))),
                "x-large" => return Some(Value::Length(Length::Px(24.0))),
                "xx-large" => return Some(Value::Length(Length::Px(32.0))),
                "smaller" => return Some(Value::Length(Length::Em(0.8))),
                "larger" => return Some(Value::Length(Length::Em(1.2))),
                _ => {}
            }

            // Colors
            if let Some(color) = Value::parse_color(&lower) {
                return Some(Value::Color(color));
            }

            // Length auto and intrinsic keywords
            match lower.as_str() {
                "auto" => return Some(Value::Length(Length::Auto)),
                "content" => return Some(Value::Length(Length::Content)),
                "min-content" => return Some(Value::Length(Length::MinContent)),
                "max-content" => return Some(Value::Length(Length::MaxContent)),
                "fit-content" => return Some(Value::Length(Length::FitContent)),
                _ => {}
            }

            // Display
            match lower.as_str() {
                "block" => return Some(Value::Display(Display::Block)),
                "inline" => return Some(Value::Display(Display::Inline)),
                "inline-block" => return Some(Value::Display(Display::InlineBlock)),
                "flex" => return Some(Value::Display(Display::Flex)),
                "inline-flex" => return Some(Value::Display(Display::InlineFlex)),
                "grid" => return Some(Value::Display(Display::Grid)),
                "inline-grid" => return Some(Value::Display(Display::InlineGrid)),
                "flow-root" => return Some(Value::Display(Display::FlowRoot)),
                "contents" => return Some(Value::Display(Display::Contents)),
                "table" => return Some(Value::Display(Display::Table)),
                "table-row" => return Some(Value::Display(Display::TableRow)),
                "table-cell" => return Some(Value::Display(Display::TableCell)),
                "table-caption" => return Some(Value::Display(Display::TableCaption)),
                "table-row-group" => return Some(Value::Display(Display::TableRowGroup)),
                "table-header-group" => return Some(Value::Display(Display::TableHeaderGroup)),
                "table-footer-group" => return Some(Value::Display(Display::TableFooterGroup)),
                "table-column" => return Some(Value::Display(Display::TableColumn)),
                "table-column-group" => return Some(Value::Display(Display::TableColumnGroup)),
                "list-item" => return Some(Value::Display(Display::ListItem)),
                "ruby" => return Some(Value::Display(Display::Ruby)),
                "ruby-base" => return Some(Value::Display(Display::RubyBase)),
                "ruby-text" => return Some(Value::Display(Display::RubyText)),
                _ => {}
            }

            // Position
            match lower.as_str() {
                "static" => return Some(Value::Position(Position::Static)),
                "relative" => return Some(Value::Position(Position::Relative)),
                "absolute" => return Some(Value::Position(Position::Absolute)),
                "fixed" => return Some(Value::Position(Position::Fixed)),
                "sticky" => return Some(Value::Position(Position::Sticky)),
                _ => {}
            }

            // Direction
            match lower.as_str() {
                "ltr" => return Some(Value::Direction(Direction::Ltr)),
                "rtl" => return Some(Value::Direction(Direction::Rtl)),
                _ => {}
            }

            // Unicode Bidi
            match lower.as_str() {
                "embed" => return Some(Value::UnicodeBidi(UnicodeBidi::Embed)),
                "isolate" => return Some(Value::UnicodeBidi(UnicodeBidi::Isolate)),
                "bidi-override" => return Some(Value::UnicodeBidi(UnicodeBidi::BidiOverride)),
                "isolate-override" => {
                    return Some(Value::UnicodeBidi(UnicodeBidi::IsolateOverride));
                }
                "plaintext" => return Some(Value::UnicodeBidi(UnicodeBidi::Plaintext)),
                _ => {}
            }

            // Flex Direction
            match lower.as_str() {
                "row" => return Some(Value::FlexDirection(FlexDirection::Row)),
                "row-reverse" => return Some(Value::FlexDirection(FlexDirection::RowReverse)),
                "column" => return Some(Value::FlexDirection(FlexDirection::Column)),
                "column-reverse" => {
                    return Some(Value::FlexDirection(FlexDirection::ColumnReverse));
                }
                _ => {}
            }

            // Flex Wrap
            match lower.as_str() {
                "nowrap" => return Some(Value::FlexWrap(FlexWrap::NoWrap)),
                "wrap" => return Some(Value::FlexWrap(FlexWrap::Wrap)),
                "wrap-reverse" => return Some(Value::FlexWrap(FlexWrap::WrapReverse)),
                _ => {}
            }

            // Justify Content
            match lower.as_str() {
                "flex-start" => return Some(Value::JustifyContent(JustifyContent::FlexStart)),
                "flex-end" => return Some(Value::JustifyContent(JustifyContent::FlexEnd)),
                "space-between" => {
                    return Some(Value::JustifyContent(JustifyContent::SpaceBetween));
                }
                "space-around" => {
                    return Some(Value::JustifyContent(JustifyContent::SpaceAround));
                }
                "space-evenly" => {
                    return Some(Value::JustifyContent(JustifyContent::SpaceEvenly));
                }
                _ => {}
            }

            // Align Items & Align Self
            match lower.as_str() {
                "stretch" => return Some(Value::AlignItems(AlignItems::Stretch)),
                "baseline" => return Some(Value::AlignItems(AlignItems::Baseline)),
                _ => {}
            }

            // Float
            match lower.as_str() {
                "left" => return Some(Value::Float(Float::Left)),
                "right" => return Some(Value::Float(Float::Right)),
                _ => {}
            }

            // Clear
            if lower.as_str() == "both" {
                return Some(Value::Clear(Clear::Both));
            }

            // Text align
            match lower.as_str() {
                "center" => return Some(Value::TextAlign(TextAlign::Center)),
                "justify" => return Some(Value::TextAlign(TextAlign::Justify)),
                _ => {}
            }

            // Border style
            match lower.as_str() {
                "solid" => return Some(Value::BorderStyle(BorderStyle::Solid)),
                "dashed" => return Some(Value::BorderStyle(BorderStyle::Dashed)),
                "dotted" => return Some(Value::BorderStyle(BorderStyle::Dotted)),
                "double" => return Some(Value::BorderStyle(BorderStyle::Double)),
                _ => {}
            }

            // Border collapse
            match lower.as_str() {
                "collapse" => return Some(Value::BorderCollapse(BorderCollapse::Collapse)),
                "separate" => return Some(Value::BorderCollapse(BorderCollapse::Separate)),
                _ => {}
            }

            // Font weight
            match lower.as_str() {
                "bold" => return Some(Value::FontWeight(FontWeight::Bold)),
                "normal" => return Some(Value::FontWeight(FontWeight::Normal)),
                "bolder" => return Some(Value::FontWeight(FontWeight::Bolder)),
                "lighter" => return Some(Value::FontWeight(FontWeight::Lighter)),
                _ => {}
            }

            // Font style
            match lower.as_str() {
                "italic" => return Some(Value::FontStyle(FontStyle::Italic)),
                "oblique" => return Some(Value::FontStyle(FontStyle::Oblique)),
                _ => {}
            }

            // Box sizing
            match lower.as_str() {
                "border-box" => return Some(Value::BoxSizing(BoxSizing::BorderBox)),
                "content-box" => return Some(Value::BoxSizing(BoxSizing::ContentBox)),
                _ => {}
            }

            // Text decoration
            match lower.as_str() {
                "underline" => return Some(Value::TextDecoration(TextDecoration::Underline)),
                "line-through" => {
                    return Some(Value::TextDecoration(TextDecoration::LineThrough));
                }
                _ => {}
            }

            // Overflow
            match lower.as_str() {
                "visible" => return Some(Value::Overflow(Overflow::Visible)),
                "hidden" => return Some(Value::Overflow(Overflow::Hidden)),
                "scroll" => return Some(Value::Overflow(Overflow::Scroll)),
                "auto" => return Some(Value::Overflow(Overflow::Auto)),
                _ => {}
            }

            // ColumnSpan (PRD 8.1)
            if lower.as_str() == "all" {
                return Some(Value::ColumnSpan(ColumnSpan::All));
            }

            // BreakInside (PRD 8.1)
            match lower.as_str() {
                "avoid" => return Some(Value::BreakInside(BreakInside::Avoid)),
                "avoid-column" => return Some(Value::BreakInside(BreakInside::AvoidColumn)),
                "avoid-page" => return Some(Value::BreakInside(BreakInside::AvoidPage)),
                _ => {}
            }

            // Visibility
            if lower.as_str() == "collapse" {
                return Some(Value::Visibility(Visibility::Collapse));
            }

            // WhiteSpace
            match lower.as_str() {
                "nowrap" => return Some(Value::WhiteSpace(WhiteSpace::Nowrap)),
                "pre" => return Some(Value::WhiteSpace(WhiteSpace::Pre)),
                "pre-wrap" => return Some(Value::WhiteSpace(WhiteSpace::PreWrap)),
                "pre-line" => return Some(Value::WhiteSpace(WhiteSpace::PreLine)),
                "break-spaces" => return Some(Value::WhiteSpace(WhiteSpace::BreakSpaces)),
                _ => {}
            }

            // TextOverflow
            match lower.as_str() {
                "clip" => return Some(Value::TextOverflow(TextOverflow::Clip)),
                "ellipsis" => return Some(Value::TextOverflow(TextOverflow::Ellipsis)),
                _ => {}
            }

            // TextTransform
            match lower.as_str() {
                "uppercase" => return Some(Value::TextTransform(TextTransform::Uppercase)),
                "lowercase" => return Some(Value::TextTransform(TextTransform::Lowercase)),
                "capitalize" => return Some(Value::TextTransform(TextTransform::Capitalize)),
                _ => {}
            }

            // ListStyleType
            match lower.as_str() {
                "disc" => return Some(Value::ListStyleType(ListStyleType::Disc)),
                "circle" => return Some(Value::ListStyleType(ListStyleType::Circle)),
                "square" => return Some(Value::ListStyleType(ListStyleType::Square)),
                "decimal" => return Some(Value::ListStyleType(ListStyleType::Decimal)),
                "lower-alpha" => return Some(Value::ListStyleType(ListStyleType::LowerAlpha)),
                "upper-alpha" => return Some(Value::ListStyleType(ListStyleType::UpperAlpha)),
                "lower-roman" => return Some(Value::ListStyleType(ListStyleType::LowerRoman)),
                "upper-roman" => return Some(Value::ListStyleType(ListStyleType::UpperRoman)),
                _ => {}
            }

            // ListStylePosition
            match lower.as_str() {
                "outside" => return Some(Value::ListStylePosition(ListStylePosition::Outside)),
                "inside" => return Some(Value::ListStylePosition(ListStylePosition::Inside)),
                _ => {}
            }

            // Cursor
            match lower.as_str() {
                "pointer" => return Some(Value::Cursor(Cursor::Pointer)),
                "default" => return Some(Value::Cursor(Cursor::Default)),
                "text" => return Some(Value::Cursor(Cursor::Text)),
                "move" => return Some(Value::Cursor(Cursor::Move)),
                "not-allowed" => return Some(Value::Cursor(Cursor::NotAllowed)),
                "crosshair" => return Some(Value::Cursor(Cursor::Crosshair)),
                _ => {}
            }

            // VerticalAlign
            match lower.as_str() {
                "top" => return Some(Value::VerticalAlign(VerticalAlign::Top)),
                "middle" => return Some(Value::VerticalAlign(VerticalAlign::Middle)),
                "bottom" => return Some(Value::VerticalAlign(VerticalAlign::Bottom)),
                "text-top" => return Some(Value::VerticalAlign(VerticalAlign::TextTop)),
                "text-bottom" => return Some(Value::VerticalAlign(VerticalAlign::TextBottom)),
                "sub" => return Some(Value::VerticalAlign(VerticalAlign::Sub)),
                "super" => return Some(Value::VerticalAlign(VerticalAlign::Super)),
                _ => {}
            }

            // Background Repeat
            match lower.as_str() {
                "repeat" => return Some(Value::BackgroundRepeat(BackgroundRepeat::Repeat)),
                "repeat-x" => return Some(Value::BackgroundRepeat(BackgroundRepeat::RepeatX)),
                "repeat-y" => return Some(Value::BackgroundRepeat(BackgroundRepeat::RepeatY)),
                "no-repeat" => {
                    return Some(Value::BackgroundRepeat(BackgroundRepeat::NoRepeat));
                }
                _ => {}
            }

            // Background Size
            match lower.as_str() {
                "cover" => return Some(Value::BackgroundSize(BackgroundSize::Cover)),
                "contain" => return Some(Value::BackgroundSize(BackgroundSize::Contain)),
                _ => {}
            }

            // ObjectFit
            match lower.as_str() {
                "fill" => return Some(Value::ObjectFit(ObjectFit::Fill)),
                "scale-down" => {
                    return Some(Value::ObjectFit(ObjectFit::ScaleDown));
                }
                _ => {}
            }

            // Generic keyword
            Some(Value::Keyword(lower))
        }
        Token::Delim('/') => Some(Value::Keyword("/".to_string())),
        _ => None,
    }
}
