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
    try_parse_overflow_wrap, try_parse_quotes, try_parse_text_decoration,
    try_parse_text_decoration_color, try_parse_text_decoration_line,
    try_parse_text_decoration_style, try_parse_text_decoration_thickness,
    try_parse_text_emphasis_style, try_parse_text_underline_offset, try_parse_white_space,
    try_parse_word_break,
};
use crate::parser::declarations::value::parse_value_from_tokens;
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
use crate::values::{CaptionSide, Clear, Display, Float, Overflow, TableLayout, Value, Visibility};

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
                    } else if kw.eq_ignore_ascii_case("scope") {
                        self.skip_whitespace_and_comments();
                        let mut root = None;
                        if *self.peek() == Token::OpenParen {
                            self.advance();
                            let mut root_tokens = Vec::new();
                            let mut depth = 1;
                            while *self.peek() != Token::Eof && depth > 0 {
                                match self.peek() {
                                    Token::OpenParen => {
                                        depth += 1;
                                        root_tokens.push(self.advance());
                                    }
                                    Token::CloseParen => {
                                        depth -= 1;
                                        if depth > 0 {
                                            root_tokens.push(self.advance());
                                        } else {
                                            self.advance();
                                            break;
                                        }
                                    }
                                    _ => root_tokens.push(self.advance()),
                                }
                            }
                            let mut root_parser = CssParser::new(root_tokens);
                            if let Some(rsel) = root_parser.parse_selector_list() {
                                root = Some(crate::selectors::expand_nested_selectors(
                                    parent_sel, &rsel,
                                ));
                            }
                        } else {
                            root = Some(parent_sel.clone());
                        }

                        self.skip_whitespace_and_comments();
                        let mut limit = None;
                        if let Token::Ident(to_kw) = self.peek().clone() {
                            if to_kw.eq_ignore_ascii_case("to") {
                                self.advance();
                                self.skip_whitespace_and_comments();
                                if *self.peek() == Token::OpenParen {
                                    self.advance();
                                    let mut limit_tokens = Vec::new();
                                    let mut depth = 1;
                                    while *self.peek() != Token::Eof && depth > 0 {
                                        match self.peek() {
                                            Token::OpenParen => {
                                                depth += 1;
                                                limit_tokens.push(self.advance());
                                            }
                                            Token::CloseParen => {
                                                depth -= 1;
                                                if depth > 0 {
                                                    limit_tokens.push(self.advance());
                                                } else {
                                                    self.advance();
                                                    break;
                                                }
                                            }
                                            _ => limit_tokens.push(self.advance()),
                                        }
                                    }
                                    let mut limit_parser = CssParser::new(limit_tokens);
                                    limit = limit_parser.parse_selector_list();
                                }
                            }
                        }

                        self.skip_whitespace_and_comments();
                        if *self.peek() == Token::OpenCurly {
                            self.advance();
                            let scope_rules = self.parse_rule_list(true);
                            if *self.peek() == Token::CloseCurly {
                                self.advance();
                            }
                            if let Some(out) = out_rules.as_deref_mut() {
                                out.push(Rule::Scope(crate::parser::types::ScopeRule {
                                    root,
                                    limit,
                                    rules: scope_rules,
                                }));
                            }
                        } else {
                            self.skip_unknown_rule_or_block();
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
                let expanded_font = super::font::parse_font_shorthand(&value_tokens, important);
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
                let non_ws: Vec<&Token> = value_tokens
                    .iter()
                    .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                    .collect();
                if non_ws.len() == 1 || non_ws.len() == 2 {
                    let parse_o = |tok: &Token| match tok {
                        Token::Ident(s) => match s.to_ascii_lowercase().as_str() {
                            "visible" => Some(Value::Overflow(Overflow::Visible)),
                            "hidden" => Some(Value::Overflow(Overflow::Hidden)),
                            "scroll" => Some(Value::Overflow(Overflow::Scroll)),
                            "auto" => Some(Value::Overflow(Overflow::Auto)),
                            _ => None,
                        },
                        _ => None,
                    };
                    let ox = parse_o(non_ws[0]);
                    let oy = if non_ws.len() > 1 {
                        parse_o(non_ws[1])
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
                continue;
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

            let non_ws_val: Vec<&Token> = value_tokens
                .iter()
                .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                .collect();
            let parsed_val = if non_ws_val.len() == 1
                && let Token::Ident(ident) = non_ws_val[0]
                && matches!(
                    ident.to_ascii_lowercase().as_str(),
                    "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "revert-rule"
                ) {
                Some(Value::Keyword(ident.to_ascii_lowercase()))
            } else {
                match prop_lower.as_str() {
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
                    "text-decoration-line" => try_parse_text_decoration_line(&value_tokens),
                    "text-decoration-style" => try_parse_text_decoration_style(&value_tokens),
                    "text-decoration-color" => try_parse_text_decoration_color(&value_tokens),
                    "text-decoration-thickness" => {
                        try_parse_text_decoration_thickness(&value_tokens)
                    }
                    "text-underline-offset" => try_parse_text_underline_offset(&value_tokens),
                    "text-decoration" => try_parse_text_decoration(&value_tokens),
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
                    "display" => {
                        // `display` has a closed set of keywords. Parse it into a typed
                        // `Value::Display` so that validation and the cascade agree on
                        // the representation. Note that `none` is *not* handled by the
                        // generic keyword path in `parse_value_from_tokens`.
                        let non_ws: Vec<&Token> = value_tokens
                            .iter()
                            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                            .collect();
                        if non_ws.len() == 1 {
                            if let Token::Ident(s) = non_ws[0] {
                                let lower = s.to_ascii_lowercase();
                                let disp = match lower.as_str() {
                                    "none" => Some(Display::None),
                                    "block" => Some(Display::Block),
                                    "inline" => Some(Display::Inline),
                                    "inline-block" => Some(Display::InlineBlock),
                                    "flex" => Some(Display::Flex),
                                    "inline-flex" => Some(Display::InlineFlex),
                                    "grid" => Some(Display::Grid),
                                    "inline-grid" => Some(Display::InlineGrid),
                                    "flow-root" => Some(Display::FlowRoot),
                                    "contents" => Some(Display::Contents),
                                    "table" => Some(Display::Table),
                                    "table-row" => Some(Display::TableRow),
                                    "table-cell" => Some(Display::TableCell),
                                    "table-caption" => Some(Display::TableCaption),
                                    "table-row-group" => Some(Display::TableRowGroup),
                                    "table-header-group" => Some(Display::TableHeaderGroup),
                                    "table-footer-group" => Some(Display::TableFooterGroup),
                                    "table-column" => Some(Display::TableColumn),
                                    "table-column-group" => Some(Display::TableColumnGroup),
                                    "list-item" => Some(Display::ListItem),
                                    "ruby" => Some(Display::Ruby),
                                    "ruby-base" => Some(Display::RubyBase),
                                    "ruby-text" => Some(Display::RubyText),
                                    _ => None,
                                };
                                disp.map(Value::Display)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    "clear" => {
                        let non_ws: Vec<&Token> = value_tokens
                            .iter()
                            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                            .collect();
                        if non_ws.len() == 1 {
                            if let Token::Ident(s) = non_ws[0] {
                                match s.to_ascii_lowercase().as_str() {
                                    "none" => Some(Value::Clear(Clear::None)),
                                    "left" => Some(Value::Clear(Clear::Left)),
                                    "right" => Some(Value::Clear(Clear::Right)),
                                    "both" => Some(Value::Clear(Clear::Both)),
                                    "inline-start" => Some(Value::Clear(Clear::InlineStart)),
                                    "inline-end" => Some(Value::Clear(Clear::InlineEnd)),
                                    _ => parse_value_from_tokens(&value_tokens),
                                }
                            } else {
                                parse_value_from_tokens(&value_tokens)
                            }
                        } else {
                            None
                        }
                    }
                    "float" => {
                        let non_ws: Vec<&Token> = value_tokens
                            .iter()
                            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                            .collect();
                        if non_ws.len() == 1 {
                            if let Token::Ident(s) = non_ws[0] {
                                match s.to_ascii_lowercase().as_str() {
                                    "none" => Some(Value::Float(Float::None)),
                                    "left" => Some(Value::Float(Float::Left)),
                                    "right" => Some(Value::Float(Float::Right)),
                                    "inline-start" => Some(Value::Float(Float::InlineStart)),
                                    "inline-end" => Some(Value::Float(Float::InlineEnd)),
                                    _ => parse_value_from_tokens(&value_tokens),
                                }
                            } else {
                                parse_value_from_tokens(&value_tokens)
                            }
                        } else {
                            None
                        }
                    }
                    "visibility" => {
                        let non_ws: Vec<&Token> = value_tokens
                            .iter()
                            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                            .collect();
                        if non_ws.len() == 1 {
                            if let Token::Ident(s) = non_ws[0] {
                                match s.to_ascii_lowercase().as_str() {
                                    "visible" => Some(Value::Visibility(Visibility::Visible)),
                                    "hidden" => Some(Value::Visibility(Visibility::Hidden)),
                                    "collapse" => Some(Value::Visibility(Visibility::Collapse)),
                                    _ => parse_value_from_tokens(&value_tokens),
                                }
                            } else {
                                parse_value_from_tokens(&value_tokens)
                            }
                        } else {
                            None
                        }
                    }
                    "overflow" | "overflow-x" | "overflow-y" => {
                        let non_ws: Vec<&Token> = value_tokens
                            .iter()
                            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                            .collect();
                        let to_ov = |tok: &Token| match tok {
                            Token::Ident(s) => match s.to_ascii_lowercase().as_str() {
                                "visible" => Some(Overflow::Visible),
                                "hidden" => Some(Overflow::Hidden),
                                "scroll" => Some(Overflow::Scroll),
                                "auto" => Some(Overflow::Auto),
                                _ => None,
                            },
                            _ => None,
                        };
                        if non_ws.len() == 1 {
                            to_ov(non_ws[0])
                                .map(Value::Overflow)
                                .or_else(|| parse_value_from_tokens(&value_tokens))
                        } else if non_ws.len() == 2 && prop_lower == "overflow" {
                            let ox = to_ov(non_ws[0]);
                            let oy = to_ov(non_ws[1]);
                            if let (Some(x), Some(y)) = (ox, oy) {
                                Some(Value::List(vec![Value::Overflow(x), Value::Overflow(y)]))
                            } else {
                                parse_value_from_tokens(&value_tokens)
                            }
                        } else {
                            None
                        }
                    }
                    "margin-trim" => {
                        // CSS Box 4 grammar:
                        //   none | block | block-start | block-end | inline |
                        //   inline-start | inline-end | [block-start || block-end]
                        let mut idents: Vec<String> = Vec::new();
                        let mut only_idents = true;
                        for token in value_tokens {
                            match token {
                                Token::Whitespace | Token::Comment(_) => {}
                                Token::Ident(s) => idents.push(s.to_ascii_lowercase()),
                                _ => only_idents = false,
                            }
                        }
                        if !only_idents {
                            None
                        } else {
                            if idents.len() == 1 && idents[0] == "none" {
                                Some(Value::Keyword("none".to_string()))
                            } else if !idents.is_empty()
                                && idents.len() <= 4
                                && idents.iter().all(|ident| {
                                    matches!(
                                        ident.as_str(),
                                        "block"
                                            | "block-start"
                                            | "block-end"
                                            | "inline"
                                            | "inline-start"
                                            | "inline-end"
                                    )
                                })
                                && idents.windows(2).all(|pair| pair[0] != pair[1])
                            {
                                Some(Value::Keyword(idents.join(" ")))
                            } else {
                                None
                            }
                        }
                    }
                    _ => {
                        if prop_name.starts_with("--") {
                            let non_ws: Vec<&Token> = value_tokens
                                .iter()
                                .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
                                .collect();
                            if non_ws.len() == 1
                                && let Token::Ident(s) = non_ws[0]
                                && matches!(
                                    s.to_ascii_lowercase().as_str(),
                                    "initial"
                                        | "inherit"
                                        | "unset"
                                        | "revert"
                                        | "revert-layer"
                                        | "revert-rule"
                                )
                            {
                                Some(Value::Keyword(s.to_ascii_lowercase()))
                            } else if let Some((var_val, consumed)) =
                                crate::parser::declarations::functions::parse_var_call(
                                    &value_tokens,
                                )
                                && value_tokens[consumed..]
                                    .iter()
                                    .all(|t| matches!(t, Token::Whitespace | Token::Comment(_)))
                            {
                                Some(var_val)
                            } else {
                                let mut raw = String::new();
                                for tok in &value_tokens {
                                    raw.push_str(&self.token_to_string(tok));
                                }
                                let trimmed = raw.trim().to_string();
                                if !trimmed.is_empty() {
                                    if let Some(v_parsed) = parse_value_from_tokens(&value_tokens) {
                                        match v_parsed {
                                            Value::Color(_) => Some(Value::String(trimmed)),
                                            other => Some(other),
                                        }
                                    } else {
                                        Some(Value::String(trimmed))
                                    }
                                } else {
                                    None
                                }
                            }
                        } else {
                            parse_value_from_tokens(&value_tokens)
                        }
                    }
                }
            };

            if let Some(parsed_val) = parsed_val {
                if !crate::properties::validation::is_valid_property_value(&prop_lower, &parsed_val)
                {
                    continue;
                }
                let normalized_val =
                    super::font::normalize_unitless_zero_lengths(&prop_lower, parsed_val);
                let decl = Declaration::new(prop_name, normalized_val, important);
                // Expand shorthands (e.g. margin, padding, border) immediately
                for expanded in decl.expand_shorthand() {
                    declarations.push(expanded);
                }
            }
        }

        declarations
    }

    pub(crate) fn parse_font_shorthand(
        &self,
        tokens: &[Token],
        important: bool,
    ) -> Vec<Declaration> {
        super::font::parse_font_shorthand(tokens, important)
    }
}
