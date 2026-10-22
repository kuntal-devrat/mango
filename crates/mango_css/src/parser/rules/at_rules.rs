use super::super::stream::CssParser;
use crate::parser::types::*;
use crate::tokenizer::Token;
use crate::values::Value;

impl CssParser {
    pub(crate) fn parse_rule_list(&mut self, is_nested_block: bool) -> Vec<Rule> {
        let mut rules = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::Eof {
                break;
            }
            if is_nested_block && *self.peek() == Token::CloseCurly {
                break;
            }

            // Check for @media or @import
            if let Token::AtKeyword(kw) = self.peek().clone() {
                self.advance();
                self.skip_whitespace_and_comments();
                if kw.eq_ignore_ascii_case("import") {
                    let mut path = String::new();
                    self.skip_whitespace_and_comments();
                    match self.peek().clone() {
                        Token::String(s) => {
                            path = s;
                            self.advance();
                        }
                        Token::Ident(name) if name.eq_ignore_ascii_case("url") => {
                            self.advance(); // consume url
                            self.skip_whitespace_and_comments();
                            if *self.peek() == Token::OpenParen {
                                self.advance(); // consume (
                                self.skip_whitespace_and_comments();
                                if let Token::String(s) = self.peek().clone() {
                                    path = s;
                                    self.advance();
                                } else {
                                    // Unquoted url(...)
                                    while *self.peek() != Token::CloseParen
                                        && *self.peek() != Token::Eof
                                    {
                                        let tok = self.advance();
                                        path.push_str(&self.token_to_string(&tok));
                                    }
                                }
                                self.skip_whitespace_and_comments();
                                if *self.peek() == Token::CloseParen {
                                    self.advance();
                                }
                            }
                        }
                        _ => {}
                    }
                    self.skip_until(Token::Semicolon);
                    if *self.peek() == Token::Semicolon {
                        self.advance();
                    }
                    let trimmed = path.trim().to_string();
                    if !trimmed.is_empty() {
                        rules.push(Rule::Import(trimmed));
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("media") {
                    let mut query = String::new();
                    while *self.peek() != Token::OpenCurly && *self.peek() != Token::Eof {
                        let tok = self.advance();
                        query.push_str(&self.token_to_string(&tok));
                    }
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let mut media_rules = Vec::new();
                        self.parse_nested_style_rules_into(&mut media_rules, 0);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::Media(MediaRule {
                            query: query.trim().to_string(),
                            rules: media_rules,
                        }));
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("container") {
                    self.skip_whitespace_and_comments();
                    let mut container_name = None;
                    if let Token::Ident(name) = self.peek().clone() {
                        container_name = Some(name);
                        self.advance();
                        self.skip_whitespace_and_comments();
                    }
                    let mut query = String::new();
                    while *self.peek() != Token::OpenCurly && *self.peek() != Token::Eof {
                        let tok = self.advance();
                        query.push_str(&self.token_to_string(&tok));
                    }
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let mut container_rules = Vec::new();
                        self.parse_nested_style_rules_into(&mut container_rules, 0);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::Container(ContainerRule {
                            name: container_name,
                            query: query.trim().to_string(),
                            rules: container_rules,
                        }));
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("font-face") {
                    if let Some(font_face) = self.parse_font_face_rule() {
                        rules.push(Rule::FontFace(font_face));
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("keyframes")
                    || kw.eq_ignore_ascii_case("-webkit-keyframes")
                    || kw.eq_ignore_ascii_case("-moz-keyframes")
                {
                    // Parse @keyframes name { ... } — store as KeyframesRule for animation use.
                    self.skip_whitespace_and_comments();
                    let mut anim_name = String::new();
                    // Name may be an ident or a quoted string
                    match self.peek().clone() {
                        Token::Ident(name) | Token::String(name) => {
                            anim_name = name;
                            self.advance();
                        }
                        _ => {}
                    }
                    self.skip_whitespace_and_comments();
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let mut keyframes: Vec<KeyframeRule> = Vec::new();
                        loop {
                            self.skip_whitespace_and_comments();
                            if *self.peek() == Token::CloseCurly || *self.peek() == Token::Eof {
                                break;
                            }
                            // Parse selector (from/to/percentage list) + declarations block
                            let mut selectors: Vec<f32> = Vec::new();
                            let mut valid_selectors = true;
                            loop {
                                self.skip_whitespace_and_comments();
                                match self.peek().clone() {
                                    Token::Ident(kw2) => {
                                        self.advance();
                                        match kw2.to_ascii_lowercase().as_str() {
                                            "from" => selectors.push(0.0),
                                            "to" => selectors.push(100.0),
                                            _ => {
                                                valid_selectors = false;
                                            }
                                        };
                                    }
                                    Token::Percentage(p) => {
                                        selectors.push(p);
                                        self.advance();
                                    }
                                    Token::Number(n) => {
                                        selectors.push(n);
                                        self.advance();
                                    }
                                    Token::Comma => {
                                        self.advance();
                                        continue;
                                    }
                                    _ => break,
                                }
                            }
                            self.skip_whitespace_and_comments();
                            if *self.peek() == Token::OpenCurly {
                                self.advance();
                                let decls = self.parse_declarations_until(Token::CloseCurly);
                                if valid_selectors && !selectors.is_empty() {
                                    keyframes.push(KeyframeRule {
                                        offsets: selectors,
                                        declarations: decls,
                                    });
                                }
                            } else {
                                self.skip_unknown_rule_or_block();
                            }
                        }
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        if !anim_name.is_empty() {
                            rules.push(Rule::Keyframes(KeyframesRule {
                                name: anim_name,
                                keyframes,
                            }));
                        }
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("charset") || kw.eq_ignore_ascii_case("namespace") {
                    self.skip_until(Token::Semicolon);
                    if *self.peek() == Token::Semicolon {
                        self.advance();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("supports") {
                    let mut condition_tokens = Vec::new();
                    let mut has_block = false;
                    while *self.peek() != Token::Eof {
                        match self.peek() {
                            Token::OpenCurly => {
                                self.advance();
                                has_block = true;
                                break;
                            }
                            Token::Semicolon => {
                                self.advance();
                                break;
                            }
                            _ => {
                                condition_tokens.push(self.advance());
                            }
                        }
                    }
                    if has_block {
                        let mut support_rules = Vec::new();
                        self.parse_nested_style_rules_into(&mut support_rules, 0);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        if self.eval_supports_condition(&condition_tokens) {
                            rules.push(Rule::Media(MediaRule {
                                query: "all".to_string(),
                                rules: support_rules,
                            }));
                        }
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("layer") {
                    self.skip_whitespace_and_comments();
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let layer_rules = self.parse_rule_list(true);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::Layer(LayerRule {
                            name: None,
                            rules: layer_rules,
                        }));
                        continue;
                    }

                    let mut preamble_tokens = Vec::new();
                    let mut has_block = false;
                    while *self.peek() != Token::Eof {
                        match self.peek() {
                            Token::OpenCurly => {
                                self.advance();
                                has_block = true;
                                break;
                            }
                            Token::Semicolon => {
                                self.advance();
                                break;
                            }
                            _ => {
                                preamble_tokens.push(self.advance());
                            }
                        }
                    }

                    let mut preamble_str = String::new();
                    for t in &preamble_tokens {
                        preamble_str.push_str(&self.token_to_string(t));
                    }
                    let preamble_str = preamble_str.trim();

                    if has_block {
                        let layer_name = if preamble_str.is_empty() {
                            None
                        } else {
                            Some(preamble_str.to_string())
                        };
                        let layer_rules = self.parse_rule_list(true);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::Layer(LayerRule {
                            name: layer_name,
                            rules: layer_rules,
                        }));
                    } else {
                        // Statement syntax: @layer base, components, utilities;
                        for part in preamble_str.split(',') {
                            let name = part.trim();
                            if !name.is_empty() {
                                rules.push(Rule::Layer(LayerRule {
                                    name: Some(name.to_string()),
                                    rules: Vec::new(),
                                }));
                            }
                        }
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("scope") {
                    self.skip_whitespace_and_comments();
                    let mut root = None;
                    let mut limit = None;

                    // 1. Optional root selector: (root)
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
                                _ => {
                                    root_tokens.push(self.advance());
                                }
                            }
                        }
                        let mut root_parser = CssParser::new(root_tokens);
                        root = root_parser.parse_selector_list();
                    }

                    self.skip_whitespace_and_comments();

                    // 2. Optional limit selector: to (limit)
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
                                        _ => {
                                            limit_tokens.push(self.advance());
                                        }
                                    }
                                }
                                let mut limit_parser = CssParser::new(limit_tokens);
                                limit = limit_parser.parse_selector_list();
                            }
                        }
                    }

                    self.skip_whitespace_and_comments();

                    // 3. Nested style rules block: { ... }
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let scope_rules = self.parse_rule_list(true);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::Scope(ScopeRule {
                            root,
                            limit,
                            rules: scope_rules,
                        }));
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("position-try") {
                    self.skip_whitespace_and_comments();
                    let mut name = String::new();
                    while *self.peek() != Token::Eof
                        && *self.peek() != Token::OpenCurly
                        && *self.peek() != Token::Semicolon
                    {
                        match self.advance() {
                            Token::Ident(id) => name.push_str(&id),
                            Token::Delim(c) => name.push(c),
                            Token::Whitespace => {}
                            _ => {}
                        }
                    }
                    self.skip_whitespace_and_comments();
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let decls = self.parse_declarations_until(Token::CloseCurly);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        rules.push(Rule::PositionTry(PositionTryRule {
                            name: name.trim().to_string(),
                            declarations: decls,
                        }));
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("view-transition") {
                    self.skip_whitespace_and_comments();
                    if *self.peek() == Token::OpenCurly {
                        self.advance();
                        let decls = self.parse_declarations_until(Token::CloseCurly);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        let mut navigation = "none".to_string();
                        let mut types = Vec::new();
                        for d in decls {
                            if d.name.eq_ignore_ascii_case("navigation") {
                                if let Value::Keyword(k) | Value::String(k) = &d.value {
                                    navigation = k.clone();
                                }
                            } else if d.name.eq_ignore_ascii_case("types") {
                                if let Value::Keyword(k) | Value::String(k) = &d.value {
                                    types.push(k.clone());
                                } else if let Value::List(items) = &d.value {
                                    for item in items {
                                        if let Value::Keyword(k) | Value::String(k) = item {
                                            types.push(k.clone());
                                        }
                                    }
                                }
                            }
                        }
                        rules.push(Rule::ViewTransition(ViewTransitionRule {
                            navigation,
                            types,
                        }));
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                if kw.eq_ignore_ascii_case("property") {
                    self.skip_whitespace_and_comments();
                    let mut prop_name = String::new();
                    while *self.peek() != Token::Eof
                        && *self.peek() != Token::OpenCurly
                        && *self.peek() != Token::Semicolon
                    {
                        match self.advance() {
                            Token::Ident(id) => prop_name.push_str(&id),
                            Token::Delim(c) => prop_name.push(c),
                            Token::Whitespace => {}
                            _ => {}
                        }
                    }
                    self.skip_whitespace_and_comments();
                    let prop_name = prop_name.trim().to_string();
                    if prop_name.starts_with("--") && *self.peek() == Token::OpenCurly {
                        self.advance();
                        let decls = self.parse_declarations_until(Token::CloseCurly);
                        let mut syntax = None;
                        let mut inherits = false;
                        let mut initial_value = None;
                        for d in decls {
                            if d.name.eq_ignore_ascii_case("syntax") {
                                match &d.value {
                                    Value::String(s) | Value::Keyword(s) => {
                                        syntax = Some(s.clone());
                                    }
                                    _ => syntax = Some(d.value.to_css_string()),
                                }
                            } else if d.name.eq_ignore_ascii_case("inherits") {
                                if let Value::Keyword(k) = &d.value {
                                    inherits = k.eq_ignore_ascii_case("true");
                                }
                            } else if d.name.eq_ignore_ascii_case("initial-value") {
                                initial_value = Some(d.value);
                            }
                        }
                        rules.push(Rule::Property(Box::new(PropertyRule {
                            name: prop_name,
                            syntax,
                            inherits,
                            initial_value,
                        })));
                    } else {
                        self.skip_unknown_rule_or_block();
                    }
                    continue;
                }

                // Unknown at-rule, skip until semicolon or block
                self.skip_unknown_rule_or_block();
                continue;
            }

            if is_nested_block && !self.is_nested_style_rule_ahead() {
                let mut decl_parser_tokens = Vec::new();
                let mut depth = 0;
                while *self.peek() != Token::CloseCurly && *self.peek() != Token::Eof {
                    if *self.peek() == Token::OpenCurly {
                        depth += 1;
                    } else if *self.peek() == Token::CloseCurly {
                        if depth > 0 {
                            depth -= 1;
                        } else {
                            break;
                        }
                    }
                    if depth == 0 && self.is_nested_style_rule_ahead() {
                        break;
                    }
                    decl_parser_tokens.push(self.advance());
                }
                let mut sub_parser = CssParser::new(decl_parser_tokens);
                let decls = sub_parser.parse_declarations_until(Token::Eof);
                if !decls.is_empty() {
                    let amp_sel = crate::selectors::SelectorList::new(vec![
                        crate::selectors::ComplexSelector::new(
                            crate::selectors::CompoundSelector::new(vec![
                                crate::selectors::SimpleSelector::Nesting,
                            ]),
                            vec![],
                        ),
                    ]);
                    rules.push(Rule::Style(StyleRule {
                        selectors: amp_sel,
                        declarations: decls,
                    }));
                }
                continue;
            }

            let mut style_rules = Vec::new();
            if self.parse_style_rule_and_nested_into(&mut style_rules) {
                rules.extend(style_rules);
            } else {
                // Skip erroneous rule or block
                self.skip_unknown_rule_or_block();
            }
        }

        rules
    }
}
