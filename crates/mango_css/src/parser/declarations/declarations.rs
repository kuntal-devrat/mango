use crate::parser::declarations::animation::{
    is_transition_animation_property, parse_transition_animation_property,
    try_parse_container_name, try_parse_container_shorthand, try_parse_container_type,
    try_parse_will_change,
};
use crate::parser::declarations::functions::try_parse_aspect_ratio;
use crate::parser::declarations::transforms::try_parse_transform_list;
use crate::parser::declarations::typography::{
    try_parse_content, try_parse_counter_actions, try_parse_font_feature_settings,
    try_parse_font_variation_settings, try_parse_hyphens, try_parse_line_clamp,
    try_parse_overflow_wrap, try_parse_quotes, try_parse_text_decoration_thickness,
    try_parse_text_emphasis_style, try_parse_white_space, try_parse_word_break,
};
use crate::parser::declarations::value::{parse_single_value, parse_value_from_tokens};
use crate::parser::declarations::visual::{
    split_commas, try_parse_background_attachment, try_parse_background_blend_mode,
    try_parse_background_clip, try_parse_blend_mode, try_parse_border_image, try_parse_clip_path,
    try_parse_filter_list, try_parse_isolation, try_parse_layered_background_images,
    try_parse_mask_composite, try_parse_mask_mode, try_parse_text_shadow,
};
use crate::parser::stream::CssParser;
use crate::parser::types::{MediaRule, Rule, StyleRule};
use crate::properties::Declaration;
use crate::selectors::SelectorList;
use crate::tokenizer::Token;
use crate::values::{CaptionSide, FontStyle, FontWeight, Length, Overflow, TableLayout, Value};

impl CssParser {
    pub(crate) fn parse_declarations_until(&mut self, stop_token: Token) -> Vec<Declaration> {
        self.parse_declarations_with_nesting_until(stop_token, None, None)
    }

    pub(crate) fn is_nested_style_rule_ahead(&self) -> bool {
        let mut idx = self.cursor;
        let mut paren_depth = 0;
        let mut bracket_depth = 0;

        while idx < self.tokens.len() {
            match &self.tokens[idx] {
                Token::OpenParen => paren_depth += 1,
                Token::CloseParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                Token::OpenBracket => bracket_depth += 1,
                Token::CloseBracket => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                Token::OpenCurly if paren_depth == 0 && bracket_depth == 0 => {
                    return true;
                }
                Token::Semicolon | Token::CloseCurly if paren_depth == 0 && bracket_depth == 0 => {
                    return false;
                }
                Token::Eof => return false,
                _ => {}
            }
            idx += 1;
        }
        false
    }

    pub(crate) fn parse_declarations_with_nesting_until(
        &mut self,
        stop_token: Token,
        current_selectors: Option<&SelectorList>,
        mut out_rules: Option<&mut Vec<Rule>>,
    ) -> Vec<Declaration> {
        let mut declarations = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == stop_token || *self.peek() == Token::Eof {
                if *self.peek() == stop_token {
                    self.advance();
                }
                break;
            }

            if let Some(parent_sel) = current_selectors {
                if let Token::AtKeyword(kw) = self.peek().clone() {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    if kw.eq_ignore_ascii_case("media") {
                        let mut query = String::new();
                        while *self.peek() != Token::OpenCurly && *self.peek() != Token::Eof {
                            let tok = self.advance();
                            query.push_str(&self.token_to_string(&tok));
                        }
                        if *self.peek() == Token::OpenCurly {
                            self.advance();
                            let mut inner_rules = Vec::new();
                            let nested_decls = self.parse_declarations_with_nesting_until(
                                Token::CloseCurly,
                                Some(parent_sel),
                                Some(&mut inner_rules),
                            );
                            let mut media_rules = Vec::new();
                            if !nested_decls.is_empty() {
                                media_rules.push(StyleRule {
                                    selectors: parent_sel.clone(),
                                    declarations: nested_decls,
                                });
                            }
                            for r in inner_rules {
                                if let Rule::Style(sr) = r {
                                    media_rules.push(sr);
                                }
                            }
                            if let Some(out) = out_rules.as_deref_mut() {
                                out.push(Rule::Media(MediaRule {
                                    query: query.trim().to_string(),
                                    rules: media_rules,
                                }));
                            }
                        }
                        continue;
                    } else if kw.eq_ignore_ascii_case("supports") {
                        let mut cond_tokens = Vec::new();
                        while *self.peek() != Token::OpenCurly && *self.peek() != Token::Eof {
                            cond_tokens.push(self.advance());
                        }
                        if *self.peek() == Token::OpenCurly {
                            self.advance();
                            let cond_ok = self.eval_supports_condition(&cond_tokens);
                            let mut inner_rules = Vec::new();
                            let nested_decls = self.parse_declarations_with_nesting_until(
                                Token::CloseCurly,
                                Some(parent_sel),
                                Some(&mut inner_rules),
                            );
                            if cond_ok {
                                if let Some(out) = out_rules.as_deref_mut() {
                                    if !nested_decls.is_empty() {
                                        out.push(Rule::Style(StyleRule {
                                            selectors: parent_sel.clone(),
                                            declarations: nested_decls,
                                        }));
                                    }
                                    out.extend(inner_rules);
                                }
                            }
                        }
                        continue;
                    } else {
                        self.skip_unknown_rule_or_block();
                        continue;
                    }
                }

                if self.is_nested_style_rule_ahead() {
                    if let Some(child_selectors) = self.parse_selector_list() {
                        let combined_selectors =
                            crate::selectors::expand_nested_selectors(parent_sel, &child_selectors);
                        self.skip_whitespace_and_comments();
                        if self.advance() == Token::OpenCurly {
                            let mut inner_rules = Vec::new();
                            let child_decls = self.parse_declarations_with_nesting_until(
                                Token::CloseCurly,
                                Some(&combined_selectors),
                                Some(&mut inner_rules),
                            );
                            if let Some(out) = out_rules.as_deref_mut() {
                                if !child_decls.is_empty() || inner_rules.is_empty() {
                                    out.push(Rule::Style(StyleRule {
                                        selectors: combined_selectors,
                                        declarations: child_decls,
                                    }));
                                }
                                out.extend(inner_rules);
                            }
                        }
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }
            }

            let prop_name = match self.advance() {
                Token::Ident(name) => name,
                _ => {
                    self.skip_until_or_stop(Token::Semicolon, &stop_token);
                    continue;
                }
            };

            self.skip_whitespace_and_comments();
            if self.advance() != Token::Colon {
                self.skip_until_or_stop(Token::Semicolon, &stop_token);
                continue;
            }

            // Gather value tokens until semicolon, stop token, or EOF
            let mut value_tokens = Vec::new();
            let mut important = false;
            let mut invalid_after_important = false;

            while *self.peek() != Token::Semicolon
                && *self.peek() != stop_token
                && *self.peek() != Token::Eof
            {
                if *self.peek() == Token::Delim('!') {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    if let Token::Ident(word) = self.peek()
                        && word.eq_ignore_ascii_case("important")
                    {
                        important = true;
                        self.advance();
                        self.skip_whitespace_and_comments();
                        if *self.peek() != Token::Semicolon
                            && *self.peek() != stop_token
                            && *self.peek() != Token::Eof
                        {
                            invalid_after_important = true;
                        }
                        continue;
                    }
                }
                if important {
                    invalid_after_important = true;
                }
                value_tokens.push(self.advance());
            }

            if *self.peek() == Token::Semicolon {
                self.advance();
            }

            if invalid_after_important {
                continue;
            }

            let prop_lower = prop_name.to_ascii_lowercase();
            if prop_lower == "font" {
                let expanded_font = self.parse_font_shorthand(&value_tokens, important);
                if !expanded_font.is_empty() {
                    declarations.extend(expanded_font);
                    continue;
                }
            }

            // transition-* / animation-* properties are parsed into longhand declaration lists.
            if is_transition_animation_property(&prop_lower) {
                let decls =
                    parse_transition_animation_property(&prop_lower, &value_tokens, important);
                declarations.extend(decls);
                continue;
            }

            // CSS overflow shorthand (PRD 8.1)
            if prop_lower == "overflow" {
                let idents: Vec<&str> = value_tokens
                    .iter()
                    .filter_map(|t| match t {
                        Token::Ident(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect();
                if !idents.is_empty() {
                    let parse_o = |s: &str| match s.to_ascii_lowercase().as_str() {
                        "visible" => Some(Value::Overflow(Overflow::Visible)),
                        "hidden" => Some(Value::Overflow(Overflow::Hidden)),
                        "scroll" => Some(Value::Overflow(Overflow::Scroll)),
                        "auto" => Some(Value::Overflow(Overflow::Auto)),
                        _ => None,
                    };
                    let ox = parse_o(idents[0]);
                    let oy = if idents.len() > 1 {
                        parse_o(idents[1])
                    } else {
                        ox.clone()
                    };
                    if let (Some(x_val), Some(y_val)) = (ox, oy) {
                        declarations.push(Declaration {
                            name: "overflow-x".to_string(),
                            value: x_val,
                            important,
                        });
                        declarations.push(Declaration {
                            name: "overflow-y".to_string(),
                            value: y_val,
                            important,
                        });
                        continue;
                    }
                }
            }

            if prop_lower == "background" {
                let groups = split_commas(&value_tokens);
                if groups.len() > 1 {
                    let mut all_vals = Vec::new();
                    for (idx, g) in groups.iter().enumerate() {
                        if idx > 0 {
                            all_vals.push(Value::Keyword(",".to_string()));
                        }
                        if let Some(val) = parse_value_from_tokens(g) {
                            match val {
                                Value::List(inner) => all_vals.extend(inner),
                                single => all_vals.push(single),
                            }
                        }
                    }
                    if !all_vals.is_empty() {
                        let decl = Declaration::new(prop_name, Value::List(all_vals), important);
                        for expanded in decl.expand_shorthand() {
                            declarations.push(expanded);
                        }
                        continue;
                    }
                }
            }

            let parsed_val = match prop_lower.as_str() {
                "aspect-ratio" => try_parse_aspect_ratio(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "transform" | "-webkit-transform" | "-moz-transform" | "-ms-transform"
                | "-o-transform" => try_parse_transform_list(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "text-shadow" => try_parse_text_shadow(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "word-break" => try_parse_word_break(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "overflow-wrap" | "word-wrap" => try_parse_overflow_wrap(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "hyphens" => try_parse_hyphens(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "line-clamp" | "-webkit-line-clamp" => try_parse_line_clamp(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "font-variation-settings" => try_parse_font_variation_settings(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "font-feature-settings" => try_parse_font_feature_settings(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "text-emphasis-style" => try_parse_text_emphasis_style(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "text-decoration-thickness" => try_parse_text_decoration_thickness(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "filter" | "-webkit-filter" | "backdrop-filter" | "-webkit-backdrop-filter" => {
                    try_parse_filter_list(&value_tokens)
                        .or_else(|| parse_value_from_tokens(&value_tokens))
                }
                "clip-path" | "-webkit-clip-path" => try_parse_clip_path(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "mix-blend-mode" => try_parse_blend_mode(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "background-blend-mode" => try_parse_background_blend_mode(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "background-attachment" => try_parse_background_attachment(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "background-clip" | "-webkit-background-clip" => {
                    try_parse_background_clip(&value_tokens)
                        .or_else(|| parse_value_from_tokens(&value_tokens))
                }
                "border-image" | "border-image-source" => try_parse_border_image(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "mask-composite" | "-webkit-mask-composite" => {
                    try_parse_mask_composite(&value_tokens)
                        .or_else(|| parse_value_from_tokens(&value_tokens))
                }
                "isolation" => try_parse_isolation(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "background-image" => try_parse_layered_background_images(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "mask-mode" | "-webkit-mask-mode" => try_parse_mask_mode(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "will-change" => try_parse_will_change(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "white-space" => try_parse_white_space(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "box-shadow" => self
                    .parse_box_shadow(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "grid-template-columns" | "grid-template-rows" => self
                    .parse_grid_track_list(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "grid-template" => self
                    .parse_grid_template_shorthand(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "grid-template-areas" => self
                    .parse_grid_template_areas(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "grid-column" | "grid-row" | "grid-area" | "grid-column-start"
                | "grid-column-end" | "grid-row-start" | "grid-row-end" => self
                    .parse_grid_placement_or_shorthand(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "grid-auto-flow" => self
                    .parse_grid_auto_flow(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "content" => try_parse_content(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "counter-reset" => try_parse_counter_actions(&value_tokens, 0)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "counter-increment" => try_parse_counter_actions(&value_tokens, 1)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "quotes" => try_parse_quotes(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "container-type" => try_parse_container_type(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "container-name" => try_parse_container_name(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "container" => try_parse_container_shorthand(&value_tokens)
                    .or_else(|| parse_value_from_tokens(&value_tokens)),
                "table-layout" => {
                    let first_ident = value_tokens.iter().find_map(|t| match t {
                        Token::Ident(s) => Some(s.to_ascii_lowercase()),
                        _ => None,
                    });
                    match first_ident.as_deref() {
                        Some("fixed") => Some(Value::TableLayout(TableLayout::Fixed)),
                        Some("auto") => Some(Value::TableLayout(TableLayout::Auto)),
                        _ => parse_value_from_tokens(&value_tokens),
                    }
                }
                "caption-side" => {
                    let first_ident = value_tokens.iter().find_map(|t| match t {
                        Token::Ident(s) => Some(s.to_ascii_lowercase()),
                        _ => None,
                    });
                    match first_ident.as_deref() {
                        Some("top") => Some(Value::CaptionSide(CaptionSide::Top)),
                        Some("bottom") => Some(Value::CaptionSide(CaptionSide::Bottom)),
                        _ => parse_value_from_tokens(&value_tokens),
                    }
                }
                _ => {
                    let v = parse_value_from_tokens(&value_tokens);
                    if v.is_none() && prop_name.starts_with("--") {
                        let mut raw = String::new();
                        for tok in &value_tokens {
                            raw.push_str(&self.token_to_string(tok));
                        }
                        let trimmed = raw.trim().to_string();
                        if !trimmed.is_empty() {
                            Some(Value::String(trimmed))
                        } else {
                            None
                        }
                    } else {
                        v
                    }
                }
            };

            if let Some(parsed_val) = parsed_val {
                let decl = Declaration::new(prop_name, parsed_val, important);
                // Expand shorthands (e.g. margin, padding, border) immediately
                for expanded in decl.expand_shorthand() {
                    declarations.push(expanded);
                }
            }
        }

        declarations
    }

    /// Parses a CSS `font` shorthand declaration into constituent longhands:
    /// `font-style`, `font-weight`, `font-size`, `line-height`, and `font-family`.
    pub(crate) fn parse_font_shorthand(
        &self,
        tokens: &[Token],
        important: bool,
    ) -> Vec<Declaration> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return Vec::new();
        }

        // Avoid breaking global keywords like inherit/initial/unset
        if filtered.len() == 1
            && let Token::Ident(kw) = filtered[0]
        {
            let lower = kw.to_ascii_lowercase();
            if matches!(lower.as_str(), "inherit" | "initial" | "unset" | "revert") {
                return Vec::new();
            }
        }

        let mut font_style: Option<FontStyle> = None;
        let mut font_weight: Option<FontWeight> = None;
        let mut font_size: Option<Value> = None;
        let mut line_height: Option<Value> = None;
        let mut idx = 0;

        // 1. Scan optional font-style, font-variant, font-weight tokens before font-size
        while idx < filtered.len() {
            match filtered[idx] {
                Token::Ident(w) => {
                    let lower = w.to_ascii_lowercase();
                    match lower.as_str() {
                        "normal" => {
                            idx += 1;
                        }
                        "italic" => {
                            font_style = Some(FontStyle::Italic);
                            idx += 1;
                        }
                        "oblique" => {
                            font_style = Some(FontStyle::Oblique);
                            idx += 1;
                        }
                        "bold" => {
                            font_weight = Some(FontWeight::Bold);
                            idx += 1;
                        }
                        "bolder" => {
                            font_weight = Some(FontWeight::Bolder);
                            idx += 1;
                        }
                        "lighter" => {
                            font_weight = Some(FontWeight::Lighter);
                            idx += 1;
                        }
                        "small-caps" => {
                            idx += 1;
                        }
                        "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large"
                        | "xx-large" | "smaller" | "larger" => {
                            font_size = parse_single_value(filtered[idx]);
                            idx += 1;
                            break;
                        }
                        _ => {
                            break;
                        }
                    }
                }
                Token::Number(n) => {
                    if *n >= 100.0 && *n <= 900.0 && (*n as u16).is_multiple_of(100) {
                        font_weight = Some(FontWeight::Numeric(*n as u16));
                        idx += 1;
                    } else if *n == 0.0 {
                        font_size = Some(Value::Length(Length::Px(0.0)));
                        idx += 1;
                        break;
                    } else {
                        break;
                    }
                }
                Token::Dimension { .. } | Token::Percentage(_) => {
                    font_size = parse_single_value(filtered[idx]);
                    idx += 1;
                    break;
                }
                _ => break,
            }
        }

        // Must have found a font-size
        let Some(fs) = font_size else {
            return Vec::new();
        };

        // 2. Check for optional `/ <line-height>`
        if idx < filtered.len() && matches!(filtered[idx], Token::Delim('/')) {
            idx += 1;
            if idx < filtered.len() {
                match filtered[idx] {
                    Token::Number(n) => {
                        line_height = Some(Value::Number(*n));
                        idx += 1;
                    }
                    Token::Dimension { .. } | Token::Percentage(_) => {
                        line_height = parse_single_value(filtered[idx]);
                        idx += 1;
                    }
                    Token::Ident(s) if s.eq_ignore_ascii_case("normal") => {
                        idx += 1;
                    }
                    _ => {}
                }
            }
        }

        // 3. All remaining tokens form the font-family
        if idx >= filtered.len() {
            return Vec::new();
        }

        let mut family_names = Vec::new();
        let mut cur_name = String::new();

        for &tok in &filtered[idx..] {
            match tok {
                Token::Comma => {
                    let trimmed = cur_name.trim().to_string();
                    if !trimmed.is_empty() {
                        family_names.push(trimmed);
                        cur_name.clear();
                    }
                }
                Token::String(s) => {
                    if !cur_name.is_empty() {
                        cur_name.push(' ');
                    }
                    cur_name.push_str(s);
                }
                Token::Ident(s) => {
                    if !cur_name.is_empty() {
                        cur_name.push(' ');
                    }
                    cur_name.push_str(s);
                }
                _ => {}
            }
        }
        let trimmed = cur_name.trim().to_string();
        if !trimmed.is_empty() {
            family_names.push(trimmed);
        }

        if family_names.is_empty() {
            return Vec::new();
        }

        let family_str = family_names.join(", ");

        vec![
            Declaration::new(
                "font-style",
                Value::FontStyle(font_style.unwrap_or(FontStyle::Normal)),
                important,
            ),
            Declaration::new(
                "font-weight",
                Value::FontWeight(font_weight.unwrap_or(FontWeight::Normal)),
                important,
            ),
            Declaration::new("font-size", fs, important),
            Declaration::new(
                "line-height",
                line_height.unwrap_or_else(|| Value::Keyword("normal".to_string())),
                important,
            ),
            Declaration::new("font-family", Value::String(family_str), important),
        ]
    }
}
