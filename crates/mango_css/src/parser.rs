//! CSS parser for stylesheets, rules, selectors, and property declarations.

use mango_core::Color;
use crate::properties::Declaration;
use crate::selectors::{
    AttributeOperator, Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector,
};
use crate::tokenizer::{CssTokenizer, Token};
use crate::values::{
    AlignItems, BackgroundRepeat, BackgroundSize, BorderCollapse, BorderStyle, BoxSizing, BreakInside, CaptionSide, Clear, ColumnSpan, Cursor, Direction, Display,
    FlexDirection, FlexWrap, Float, FontDisplay, FontFeatureSettings, FontStyle, FontVariationSettings, FontWeight, GridAutoFlow, GridPlacement, GridTrackSize,
    Hyphens, JustifyContent, Length, LineClamp, ListStylePosition, ListStyleType, Overflow, OverflowWrap, Position, TableLayout, TextAlign,
    TextDecoration, TextDecorationThickness, TextEmphasisStyle, TextOverflow, TextTransform, UnicodeBidi, Value, VerticalAlign, Visibility, WhiteSpace, WordBreak,
};

/// A parsed CSS stylesheet containing style rules and at-rules.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

impl Stylesheet {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }
}

/// A top-level rule inside a stylesheet.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    Style(StyleRule),
    Media(MediaRule),
    Container(ContainerRule),
    Import(String),
    FontFace(FontFaceRule),
    /// `@keyframes name { ... }` animation rule.
    Keyframes(KeyframesRule),
}

/// A single keyframe rule inside `@keyframes`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeRule {
    /// Keyframe offsets as percentages (0.0 = from, 100.0 = to).
    pub offsets: Vec<f32>,
    pub declarations: Vec<Declaration>,
}

/// A complete `@keyframes` at-rule.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyframesRule {
    pub name: String,
    pub keyframes: Vec<KeyframeRule>,
}

/// A parsed `@font-face` at-rule describing a downloadable web font.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFaceRule {
    pub font_family: String,
    pub src_url: String,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_display: FontDisplay,
}

/// A CSS style rule consisting of a selector list and property declarations.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
    pub selectors: SelectorList,
    pub declarations: Vec<Declaration>,
}

/// An `@media` at-rule wrapping nested style rules.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaRule {
    pub query: String,
    pub rules: Vec<StyleRule>,
}

/// An `@container` at-rule wrapping nested style rules.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerRule {
    pub name: Option<String>,
    pub query: String,
    pub rules: Vec<StyleRule>,
}

/// The CSS parser.
pub struct CssParser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl CssParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    /// Parses an entire CSS stylesheet string.
    pub fn parse_stylesheet(css: &str) -> Stylesheet {
        let tokens = CssTokenizer::tokenize(css);
        let mut parser = Self::new(tokens);
        parser.parse_rules()
    }

    /// Parses an inline style string (e.g. from an HTML `style="..."` attribute).
    pub fn parse_declaration_list(css: &str) -> Vec<Declaration> {
        let tokens = CssTokenizer::tokenize(css);
        let mut parser = Self::new(tokens);
        parser.parse_declarations_until(Token::Eof)
    }

    /// Parses a CSS selector string (e.g. `div.main > p, #hero`).
    pub fn parse_selector_list_str(css: &str) -> Option<SelectorList> {
        let tokens = CssTokenizer::tokenize(css);
        let mut parser = Self::new(tokens);
        parser.parse_selector_list()
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.cursor).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        if self.cursor < self.tokens.len() {
            let tok = self.tokens[self.cursor].clone();
            self.cursor += 1;
            tok
        } else {
            Token::Eof
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Token::Whitespace | Token::Comment(_) = self.peek() {
            self.advance();
        }
    }

    fn parse_rules(&mut self) -> Stylesheet {
        let mut rules = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::Eof {
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
                                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
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
                        self.parse_nested_style_rules_into(&mut media_rules);
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
                        self.parse_nested_style_rules_into(&mut container_rules);
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

                if kw.eq_ignore_ascii_case("keyframes") || kw.eq_ignore_ascii_case("-webkit-keyframes") || kw.eq_ignore_ascii_case("-moz-keyframes") {
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
                        let mut keyframes: Vec<crate::parser::KeyframeRule> = Vec::new();
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
                                            _ => { valid_selectors = false; }
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
                                    Token::Comma => { self.advance(); continue; }
                                    _ => break,
                                }
                            }
                            self.skip_whitespace_and_comments();
                            if *self.peek() == Token::OpenCurly {
                                self.advance();
                                let decls = self.parse_declarations_until(Token::CloseCurly);
                                if valid_selectors && !selectors.is_empty() {
                                    keyframes.push(crate::parser::KeyframeRule {
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
                            rules.push(Rule::Keyframes(crate::parser::KeyframesRule {
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

                if kw.eq_ignore_ascii_case("supports") || kw.eq_ignore_ascii_case("layer") {
                    // @supports / @layer: parse condition then treat as block if curly brace opens
                    // Skip condition tokens up to the open brace or semicolon
                    let mut depth = 0i32;
                    loop {
                        match self.peek().clone() {
                            Token::OpenCurly => { depth += 1; self.advance(); if depth == 1 { break; } }
                            Token::Semicolon => { self.advance(); break; }
                            Token::Eof => break,
                            _ => { self.advance(); }
                        }
                    }
                    if depth > 0 {
                        // Parse rules inside @supports as author rules
                        let mut support_rules = Vec::new();
                        self.parse_nested_style_rules_into(&mut support_rules);
                        if *self.peek() == Token::CloseCurly { self.advance(); }
                        // Wrap as a media rule with "all" so cascade always applies it
                        rules.push(Rule::Media(MediaRule {
                            query: "all".to_string(),
                            rules: support_rules,
                        }));
                    }
                    continue;
                }

                // Unknown at-rule, skip until semicolon or block
                self.skip_unknown_rule_or_block();
                continue;
            }

            if let Some(rule) = self.parse_style_rule() {
                rules.push(Rule::Style(rule));
            } else {
                // Skip erroneous rule or block
                self.skip_unknown_rule_or_block();
            }
        }

        Stylesheet::new(rules)
    }

    fn parse_nested_style_rules_into(&mut self, out_rules: &mut Vec<StyleRule>) {
        while *self.peek() != Token::CloseCurly && *self.peek() != Token::Eof {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::CloseCurly || *self.peek() == Token::Eof {
                break;
            }
            if let Token::AtKeyword(inner_kw) = self.peek().clone() {
                self.advance();
                if inner_kw.eq_ignore_ascii_case("supports") || inner_kw.eq_ignore_ascii_case("layer") {
                    let mut depth = 0i32;
                    loop {
                        match self.peek().clone() {
                            Token::OpenCurly => {
                                depth += 1;
                                self.advance();
                                if depth == 1 {
                                    break;
                                }
                            }
                            Token::Semicolon => {
                                self.advance();
                                break;
                            }
                            Token::Eof => break,
                            _ => {
                                self.advance();
                            }
                        }
                    }
                    if depth > 0 {
                        self.parse_nested_style_rules_into(out_rules);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                    }
                } else {
                    self.skip_unknown_rule_or_block();
                }
                continue;
            }
            if let Some(rule) = self.parse_style_rule() {
                out_rules.push(rule);
            } else {
                self.skip_unknown_rule_or_block();
            }
        }
    }

    fn parse_style_rule(&mut self) -> Option<StyleRule> {
        let selectors = self.parse_selector_list()?;
        self.skip_whitespace_and_comments();

        if self.advance() != Token::OpenCurly {
            return None;
        }

        let declarations = self.parse_declarations_until(Token::CloseCurly);

        Some(StyleRule {
            selectors,
            declarations,
        })
    }

    fn parse_font_face_rule(&mut self) -> Option<FontFaceRule> {
        self.skip_whitespace_and_comments();
        if self.advance() != Token::OpenCurly {
            return None;
        }

        let mut font_family = String::new();
        let mut src_url = String::new();
        let mut font_weight = FontWeight::Normal;
        let mut font_style = FontStyle::Normal;
        let mut font_display = FontDisplay::Auto;

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::CloseCurly || *self.peek() == Token::Eof {
                if *self.peek() == Token::CloseCurly {
                    self.advance();
                }
                break;
            }

            let prop_name = match self.advance() {
                Token::Ident(name) => name,
                _ => {
                    self.skip_until(Token::Semicolon);
                    if *self.peek() == Token::Semicolon {
                        self.advance();
                    }
                    continue;
                }
            };

            self.skip_whitespace_and_comments();
            if self.advance() != Token::Colon {
                self.skip_until(Token::Semicolon);
                if *self.peek() == Token::Semicolon {
                    self.advance();
                }
                continue;
            }

            // Gather value tokens until semicolon, close curly, or EOF
            let mut val_tokens = Vec::new();
            while *self.peek() != Token::Semicolon
                && *self.peek() != Token::CloseCurly
                && *self.peek() != Token::Eof
            {
                val_tokens.push(self.advance());
            }

            if *self.peek() == Token::Semicolon {
                self.advance();
            }

            let prop_lower = prop_name.to_ascii_lowercase();
            match prop_lower.as_str() {
                "font-family" => {
                    let mut fam = String::new();
                    for tok in &val_tokens {
                        match tok {
                            Token::String(s) => {
                                fam = s.clone();
                                break;
                            }
                            Token::Ident(s) => {
                                if !fam.is_empty() {
                                    fam.push(' ');
                                }
                                fam.push_str(s);
                            }
                            _ => {}
                        }
                    }
                    font_family = fam.trim().to_string();
                }
                "src" => {
                    // Extract URL(s) and their optional format(...) from tokens
                    let mut candidates: Vec<(String, Option<String>)> = Vec::new();
                    let mut i = 0;
                    while i < val_tokens.len() {
                        if let Token::Ident(name) = &val_tokens[i]
                            && name.eq_ignore_ascii_case("url")
                            && i + 1 < val_tokens.len()
                            && val_tokens[i + 1] == Token::OpenParen
                        {
                            let mut u = String::new();
                            let mut j = i + 2;
                            while j < val_tokens.len() && val_tokens[j] != Token::CloseParen {
                                match &val_tokens[j] {
                                    Token::String(s) => u = s.clone(),
                                    other => u.push_str(&self.token_to_string(other)),
                                }
                                j += 1;
                            }
                            let trimmed_u = u.trim().to_string();
                            let mut format_hint = None;

                            // Check if followed by format(...)
                            let mut k = if j < val_tokens.len() && val_tokens[j] == Token::CloseParen { j + 1 } else { j };
                            while k < val_tokens.len() && matches!(val_tokens[k], Token::Whitespace) {
                                k += 1;
                            }
                            if k < val_tokens.len()
                                && matches!(&val_tokens[k], Token::Ident(fmt) if fmt.eq_ignore_ascii_case("format"))
                                && k + 1 < val_tokens.len()
                                && val_tokens[k + 1] == Token::OpenParen
                            {
                                let mut fmt_str = String::new();
                                let mut m = k + 2;
                                while m < val_tokens.len() && val_tokens[m] != Token::CloseParen {
                                    match &val_tokens[m] {
                                        Token::String(s) => fmt_str = s.clone(),
                                        Token::Ident(s) => fmt_str = s.clone(),
                                        other => fmt_str.push_str(&self.token_to_string(other)),
                                    }
                                    m += 1;
                                }
                                let cleaned = fmt_str.trim().trim_matches('\'').trim_matches('"').to_ascii_lowercase();
                                if !cleaned.is_empty() {
                                    format_hint = Some(cleaned);
                                }
                                j = m;
                            }

                            if !trimmed_u.is_empty() {
                                candidates.push((trimmed_u, format_hint));
                            }
                            i = j;
                        }
                        i += 1;
                    }

                    // Format priority order per Section 6.4: woff2 > woff > ttf > otf
                    fn format_rank(url: &str, hint: Option<&str>) -> u32 {
                        let hint_l = hint.unwrap_or("");
                        let url_l = url.to_ascii_lowercase();

                        if hint_l == "woff2"
                            || url_l.contains(".woff2")
                            || url_l.contains("font/woff2")
                        {
                            4
                        } else if hint_l == "woff"
                            || url_l.contains(".woff")
                            || url_l.contains("font/woff")
                        {
                            3
                        } else if hint_l == "truetype"
                            || hint_l == "ttf"
                            || url_l.contains(".ttf")
                            || url_l.contains("font/ttf")
                        {
                            2
                        } else if hint_l == "opentype"
                            || hint_l == "otf"
                            || url_l.contains(".otf")
                            || url_l.contains("font/otf")
                        {
                            1
                        } else {
                            0
                        }
                    }

                    if !candidates.is_empty() {
                        let mut best_url = &candidates[0].0;
                        let mut best_rank = format_rank(&candidates[0].0, candidates[0].1.as_deref());

                        for (cand_url, cand_hint) in &candidates[1..] {
                            let rank = format_rank(cand_url, cand_hint.as_deref());
                            if rank > best_rank {
                                best_rank = rank;
                                best_url = cand_url;
                            }
                        }
                        src_url = best_url.clone();
                    }
                }
                "font-weight" => {
                    for tok in &val_tokens {
                        match tok {
                            Token::Ident(s) if s.eq_ignore_ascii_case("bold") => {
                                font_weight = FontWeight::Bold;
                                break;
                            }
                            Token::Ident(s) if s.eq_ignore_ascii_case("normal") => {
                                font_weight = FontWeight::Normal;
                                break;
                            }
                            Token::Number(n) => {
                                let w = *n as u16;
                                font_weight = match w {
                                    400 => FontWeight::Normal,
                                    700 => FontWeight::Bold,
                                    100..=900 => FontWeight::Numeric(w),
                                    _ if *n >= 600.0 => FontWeight::Bold,
                                    _ => FontWeight::Normal,
                                };
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                "font-style" => {
                    for tok in &val_tokens {
                        if let Token::Ident(s) = tok {
                            if s.eq_ignore_ascii_case("italic") || s.eq_ignore_ascii_case("oblique") {
                                font_style = FontStyle::Italic;
                                break;
                            } else if s.eq_ignore_ascii_case("normal") {
                                font_style = FontStyle::Normal;
                                break;
                            }
                        }
                    }
                }
                "font-display" => {
                    for tok in &val_tokens {
                        if let Token::Ident(s) = tok {
                            if let Some(fd) = FontDisplay::parse(s) {
                                font_display = fd;
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !font_family.is_empty() && !src_url.is_empty() {
            Some(FontFaceRule {
                font_family,
                src_url,
                font_weight,
                font_style,
                font_display,
            })
        } else {
            None
        }
    }

    fn parse_selector_list(&mut self) -> Option<SelectorList> {
        let mut selectors = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::OpenCurly || *self.peek() == Token::Eof {
                break;
            }

            if let Some(complex) = self.parse_complex_selector() {
                selectors.push(complex);
            } else {
                // Recover from invalid selector: skip tokens until comma, open curly, or EOF
                while *self.peek() != Token::Comma
                    && *self.peek() != Token::OpenCurly
                    && *self.peek() != Token::Eof
                {
                    self.advance();
                }
            }

            self.skip_whitespace_and_comments();
            if *self.peek() == Token::Comma {
                self.advance(); // consume comma
                continue;
            }
            if *self.peek() == Token::OpenCurly || *self.peek() == Token::Eof {
                break;
            }
        }

        if selectors.is_empty() {
            None
        } else {
            Some(SelectorList::new(selectors))
        }
    }

    fn parse_complex_selector(&mut self) -> Option<ComplexSelector> {
        let (head, initial_comb) = match self.peek() {
            Token::Delim('>') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Universal]),
                    Some(Combinator::Child),
                )
            }
            Token::Delim('+') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Universal]),
                    Some(Combinator::NextSibling),
                )
            }
            Token::Delim('~') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Universal]),
                    Some(Combinator::SubsequentSibling),
                )
            }
            _ => (self.parse_compound_selector()?, None),
        };

        let mut tail = Vec::new();
        if let Some(comb) = initial_comb {
            let compound = self.parse_compound_selector()?;
            tail.push((comb, compound));
        }

        loop {
            // Check combinators: whitespace, `>`, `+`, `~`
            let had_whitespace = matches!(self.peek(), Token::Whitespace);
            self.skip_whitespace_and_comments();

            let combinator = match self.peek() {
                Token::Delim('>') => {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    Combinator::Child
                }
                Token::Delim('+') => {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    Combinator::NextSibling
                }
                Token::Delim('~') => {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    Combinator::SubsequentSibling
                }
                _ if had_whitespace => {
                    if matches!(
                        self.peek(),
                        Token::Ident(_) | Token::Hash(_) | Token::Delim('.') | Token::Delim('*') | Token::OpenBracket | Token::Colon
                    ) {
                        Combinator::Descendant
                    } else {
                        break;
                    }
                }
                _ => break,
            };

            if let Some(compound) = self.parse_compound_selector() {
                tail.push((combinator, compound));
            } else {
                break;
            }
        }

        Some(ComplexSelector::new(head, tail))
    }

    fn parse_compound_selector(&mut self) -> Option<CompoundSelector> {
        let mut simples = Vec::new();

        while let Some(simple) = self.parse_simple_selector() {
            simples.push(simple);
            // Compound selectors must not be separated by whitespace
            if matches!(self.peek(), Token::Whitespace | Token::Comma | Token::OpenCurly | Token::Eof) {
                break;
            }
        }

        if simples.is_empty() {
            None
        } else {
            Some(CompoundSelector::new(simples))
        }
    }

    fn parse_simple_selector(&mut self) -> Option<SimpleSelector> {
        match self.peek() {
            Token::Delim('*') => {
                self.advance();
                Some(SimpleSelector::Universal)
            }
            Token::Ident(tag) => {
                let tag_clone = tag.clone();
                self.advance();
                Some(SimpleSelector::Type(tag_clone))
            }
            Token::Delim('.') => {
                self.advance();
                if let Token::Ident(class_name) = self.advance() {
                    Some(SimpleSelector::Class(class_name))
                } else {
                    None
                }
            }
            Token::Hash(id) => {
                let id_clone = id.clone();
                self.advance();
                Some(SimpleSelector::Id(id_clone))
            }
            Token::OpenBracket => {
                self.advance(); // consume `[`
                self.skip_whitespace_and_comments();
                let attr_name = match self.advance() {
                    Token::Ident(name) => name,
                    _ => return None,
                };
                self.skip_whitespace_and_comments();

                let mut op = AttributeOperator::Exists;
                let mut attr_val = String::new();
                let mut case_insensitive = false;

                if let Token::Delim(c) = self.peek() {
                    let delim_char = *c;
                    if delim_char == '=' {
                        self.advance();
                        op = AttributeOperator::Exact;
                    } else if delim_char == '~' || delim_char == '^' || delim_char == '$' || delim_char == '*' || delim_char == '|' {
                        self.advance();
                        if let Token::Delim('=') = self.peek() {
                            self.advance();
                            op = match delim_char {
                                '~' => AttributeOperator::Includes,
                                '^' => AttributeOperator::Prefix,
                                '$' => AttributeOperator::Suffix,
                                '*' => AttributeOperator::Substring,
                                '|' => AttributeOperator::DashMatch,
                                _ => unreachable!(),
                            };
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }

                    self.skip_whitespace_and_comments();
                    match self.peek() {
                        Token::Ident(val) | Token::String(val) => {
                            attr_val = val.clone();
                            self.advance();
                        }
                        _ => {}
                    }
                    self.skip_whitespace_and_comments();
                    // Consume optional case-insensitive flag e.g. [type="submit" i]
                    if let Token::Ident(flag) = self.peek() {
                        if flag.eq_ignore_ascii_case("i") {
                            case_insensitive = true;
                            self.advance();
                        } else if flag.eq_ignore_ascii_case("s") {
                            case_insensitive = false;
                            self.advance();
                        }
                    }
                }

                self.skip_whitespace_and_comments();
                if self.advance() == Token::CloseBracket {
                    Some(SimpleSelector::Attribute {
                        name: attr_name,
                        op,
                        value: attr_val,
                        case_insensitive,
                    })
                } else {
                    None
                }
            }
            Token::Colon => {
                self.advance(); // consume first `:`
                // Check if pseudo-element :: (double colon)
                if *self.peek() == Token::Colon {
                    self.advance(); // consume second `:`
                    if let Token::Ident(name) = self.advance() {
                        let lower = name.to_ascii_lowercase();
                        // Consume optional arguments e.g. ::slotted(...)
                        if *self.peek() == Token::OpenParen {
                            self.advance();
                            let mut depth = 1;
                            while depth > 0 && *self.peek() != Token::Eof {
                                match self.advance() {
                                    Token::OpenParen => depth += 1,
                                    Token::CloseParen => depth -= 1,
                                    _ => {}
                                }
                            }
                        }
                        return Some(SimpleSelector::PseudoElement(lower));
                    } else {
                        return None;
                    }
                }

                if let Token::Ident(pseudo) = self.advance() {
                    let mut lower = pseudo.to_ascii_lowercase();
                    // Legacy single-colon pseudo-elements
                    if matches!(lower.as_str(), "before" | "after" | "first-line" | "first-letter" | "placeholder" | "selection" | "marker") {
                        return Some(SimpleSelector::PseudoElement(lower));
                    }
                    // Handle functional pseudo-classes like :nth-child(2), :not(.foo), :where(:not(...))
                    if *self.peek() == Token::OpenParen {
                        self.advance(); // consume `(`
                        let mut depth = 1;
                        let mut arg_str = String::new();
                        while depth > 0 && *self.peek() != Token::Eof {
                            let tok = self.advance();
                            match &tok {
                                Token::OpenParen => depth += 1,
                                Token::CloseParen => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            arg_str.push_str(&self.token_to_string(&tok));
                        }
                        lower.push('(');
                        lower.push_str(arg_str.trim());
                        lower.push(')');
                    }
                    Some(SimpleSelector::PseudoClass(lower))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_declarations_until(&mut self, stop_token: Token) -> Vec<Declaration> {
        let mut declarations = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            if *self.peek() == stop_token || *self.peek() == Token::Eof {
                if *self.peek() == stop_token {
                    self.advance();
                }
                break;
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

            while *self.peek() != Token::Semicolon && *self.peek() != stop_token && *self.peek() != Token::Eof {
                if *self.peek() == Token::Delim('!') {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    if let Token::Ident(word) = self.peek()
                        && word.eq_ignore_ascii_case("important") {
                            important = true;
                            self.advance();
                            self.skip_whitespace_and_comments();
                            if *self.peek() != Token::Semicolon && *self.peek() != stop_token && *self.peek() != Token::Eof {
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
                let decls = self.parse_transition_animation_property(&prop_lower, &value_tokens, important);
                declarations.extend(decls);
                continue;
            }

            // CSS overflow shorthand (PRD 8.1)
            if prop_lower == "overflow" {
                let idents: Vec<&str> = value_tokens.iter().filter_map(|t| match t {
                    Token::Ident(s) => Some(s.as_str()),
                    _ => None,
                }).collect();
                if !idents.is_empty() {
                    let parse_o = |s: &str| match s.to_ascii_lowercase().as_str() {
                        "visible" => Some(Value::Overflow(Overflow::Visible)),
                        "hidden" => Some(Value::Overflow(Overflow::Hidden)),
                        "scroll" => Some(Value::Overflow(Overflow::Scroll)),
                        "auto" => Some(Value::Overflow(Overflow::Auto)),
                        _ => None,
                    };
                    let ox = parse_o(idents[0]);
                    let oy = if idents.len() > 1 { parse_o(idents[1]) } else { ox.clone() };
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

            let parsed_val = match prop_lower.as_str() {
                "transform" | "-webkit-transform" | "-moz-transform" | "-ms-transform" | "-o-transform" => {
                    self.try_parse_transform_list(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "text-shadow" => {
                    self.try_parse_text_shadow(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "word-break" => {
                    self.try_parse_word_break(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "overflow-wrap" | "word-wrap" => {
                    self.try_parse_overflow_wrap(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "hyphens" => {
                    self.try_parse_hyphens(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "line-clamp" | "-webkit-line-clamp" => {
                    self.try_parse_line_clamp(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "font-variation-settings" => {
                    self.try_parse_font_variation_settings(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "font-feature-settings" => {
                    self.try_parse_font_feature_settings(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "text-emphasis-style" => {
                    self.try_parse_text_emphasis_style(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "text-decoration-thickness" => {
                    self.try_parse_text_decoration_thickness(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "filter" | "-webkit-filter" | "backdrop-filter" | "-webkit-backdrop-filter" => {
                    self.try_parse_filter_list(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "clip-path" | "-webkit-clip-path" => {
                    self.try_parse_clip_path(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "mix-blend-mode" => {
                    self.try_parse_blend_mode(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "background-blend-mode" => {
                    self.try_parse_background_blend_mode(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "background-attachment" => {
                    self.try_parse_background_attachment(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "background-clip" | "-webkit-background-clip" => {
                    self.try_parse_background_clip(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "border-image" | "border-image-source" => {
                    self.try_parse_border_image(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "mask-composite" | "-webkit-mask-composite" => {
                    self.try_parse_mask_composite(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "isolation" => {
                    self.try_parse_isolation(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "background-image" => {
                    self.try_parse_layered_background_images(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "mask-mode" | "-webkit-mask-mode" => {
                    self.try_parse_mask_mode(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "will-change" => {
                    self.try_parse_will_change(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "white-space" => {
                    self.try_parse_white_space(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "box-shadow" => {
                    self.parse_box_shadow(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "grid-template-columns" | "grid-template-rows" => {
                    self.parse_grid_track_list(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "grid-template" => {
                    self.parse_grid_template_shorthand(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "grid-template-areas" => {
                    self.parse_grid_template_areas(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "grid-column" | "grid-row" | "grid-area" | "grid-column-start" | "grid-column-end"
                | "grid-row-start" | "grid-row-end" => {
                    self.parse_grid_placement_or_shorthand(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "grid-auto-flow" => {
                    self.parse_grid_auto_flow(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "content" => {
                    self.try_parse_content(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "counter-reset" => {
                    self.try_parse_counter_actions(&value_tokens, 0)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "counter-increment" => {
                    self.try_parse_counter_actions(&value_tokens, 1)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "quotes" => {
                    self.try_parse_quotes(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "container-type" => {
                    self.try_parse_container_type(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "container-name" => {
                    self.try_parse_container_name(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "container" => {
                    self.try_parse_container_shorthand(&value_tokens)
                        .or_else(|| self.parse_value_from_tokens(&value_tokens))
                }
                "table-layout" => {
                    let first_ident = value_tokens.iter().find_map(|t| match t {
                        Token::Ident(s) => Some(s.to_ascii_lowercase()),
                        _ => None,
                    });
                    match first_ident.as_deref() {
                        Some("fixed") => Some(Value::TableLayout(TableLayout::Fixed)),
                        Some("auto") => Some(Value::TableLayout(TableLayout::Auto)),
                        _ => self.parse_value_from_tokens(&value_tokens),
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
                        _ => self.parse_value_from_tokens(&value_tokens),
                    }
                }
                _ => {
                    let v = self.parse_value_from_tokens(&value_tokens);
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
    fn parse_font_shorthand(&self, tokens: &[Token], important: bool) -> Vec<Declaration> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return Vec::new();
        }

        // Avoid breaking global keywords like inherit/initial/unset
        if filtered.len() == 1 {
            if let Token::Ident(kw) = filtered[0] {
                let lower = kw.to_ascii_lowercase();
                if matches!(lower.as_str(), "inherit" | "initial" | "unset" | "revert") {
                    return Vec::new();
                }
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
                            font_size = self.parse_single_value(filtered[idx]);
                            idx += 1;
                            break;
                        }
                        _ => {
                            break;
                        }
                    }
                }
                Token::Number(n) => {
                    if *n >= 100.0 && *n <= 900.0 && (*n as u16) % 100 == 0 {
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
                    font_size = self.parse_single_value(filtered[idx]);
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
                        line_height = self.parse_single_value(filtered[idx]);
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

        let mut decls = Vec::new();
        if let Some(style) = font_style {
            decls.push(Declaration::new("font-style", Value::FontStyle(style), important));
        }
        if let Some(weight) = font_weight {
            decls.push(Declaration::new("font-weight", Value::FontWeight(weight), important));
        }
        decls.push(Declaration::new("font-size", fs, important));
        if let Some(lh) = line_height {
            decls.push(Declaration::new("line-height", lh, important));
        }
        decls.push(Declaration::new("font-family", Value::String(family_str), important));

        decls
    }

    /// Parses a CSS Grid track list (e.g. `200px 1fr`, `repeat(2, 1fr)`, `minmax(100px, 1fr) auto`).
    fn parse_grid_track_list(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return None;
        }

        if filtered.len() == 1
            && let Token::Ident(w) = filtered[0]
            && w.eq_ignore_ascii_case("none")
        {
            return Some(Value::Keyword("none".to_string()));
        }

        let (tracks, lines) = self.parse_raw_tracks_with_lines(&filtered);
        if tracks.is_empty() {
            None
        } else if !lines.is_empty() {
            Some(Value::GridTrackListWithLines { tracks, lines })
        } else {
            Some(Value::GridTrackList(tracks))
        }
    }

    /// Parses a CSS `box-shadow` declaration value.
    fn parse_box_shadow(&self, tokens: &[Token]) -> Option<Value> {
        let val = self.parse_value_from_tokens(tokens)?;
        match val {
            Value::Keyword(ref k) if k.eq_ignore_ascii_case("none") => {
                Some(Value::Keyword("none".to_string()))
            }
            Value::BoxShadow(bs) => Some(Value::BoxShadow(bs)),
            Value::List(ref items) => {
                if items.iter().any(|it| matches!(it, Value::Var { .. })) {
                    return Some(Value::List(items.clone()));
                }
                let mut offset_x = None;
                let mut offset_y = None;
                let mut blur_radius = 0.0f32;
                let mut spread_radius = 0.0f32;
                let mut color = Color::rgba(0, 0, 0, 255);
                let mut inset = false;
                let mut len_count = 0;

                for item in items {
                    match item {
                        Value::Keyword(k) if k.eq_ignore_ascii_case("inset") => inset = true,
                        Value::Length(len) => {
                            let px = match len {
                                Length::Px(p) => *p,
                                Length::Em(e) => *e * 16.0,
                                Length::Rem(r) => *r * 16.0,
                                Length::Calc(c) => c.px,
                                _ => 0.0,
                            };
                            match len_count {
                                0 => offset_x = Some(px),
                                1 => offset_y = Some(px),
                                2 => blur_radius = px,
                                3 => spread_radius = px,
                                _ => {}
                            }
                            len_count += 1;
                        }
                        Value::Number(n) => {
                            let px = *n;
                            match len_count {
                                0 => offset_x = Some(px),
                                1 => offset_y = Some(px),
                                2 => blur_radius = px,
                                3 => spread_radius = px,
                                _ => {}
                            }
                            len_count += 1;
                        }
                        Value::Color(c) => color = *c,
                        Value::Var { fallback, .. } => {
                            if let Some(fb) = fallback {
                                if let Value::Color(c) = &**fb {
                                    color = *c;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
                    Some(Value::BoxShadow(crate::values::BoxShadow {
                        offset_x: ox,
                        offset_y: oy,
                        blur_radius,
                        spread_radius,
                        color,
                        inset,
                    }))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_raw_tracks_with_lines(&self, filtered: &[&Token]) -> (Vec<GridTrackSize>, Vec<(String, usize)>) {
        let mut tracks = Vec::new();
        let mut lines = Vec::new();
        let mut i = 0;

        while i < filtered.len() {
            match filtered[i] {
                Token::OpenBracket => {
                    let close_idx = filtered.iter().enumerate().skip(i + 1)
                        .find(|(_, tok)| ***tok == Token::CloseBracket)
                        .map(|(j, _)| j);
                    if let Some(end) = close_idx {
                        let line_tokens = &filtered[i + 1..end];
                        for tok in line_tokens {
                            if let Token::Ident(name) = tok {
                                lines.push((name.to_ascii_lowercase(), tracks.len()));
                            }
                        }
                        i = end + 1;
                        continue;
                    }
                }
                Token::Ident(name) if name.eq_ignore_ascii_case("repeat") => {
                    if i + 1 < filtered.len() && *filtered[i + 1] == Token::OpenParen {
                        let close_idx = filtered.iter().enumerate().skip(i + 2)
                            .find(|(_, tok)| ***tok == Token::CloseParen)
                            .map(|(j, _)| j);
                        if let Some(end) = close_idx {
                            let inner = &filtered[i + 2..end];
                            if let Some(comma_idx) = inner.iter().position(|t| **t == Token::Comma) {
                                let count_tokens = &inner[..comma_idx];
                                let track_tokens = &inner[comma_idx + 1..];
                                let is_auto_fill = count_tokens.iter().any(|tok| match tok {
                                    Token::Ident(name) => name.eq_ignore_ascii_case("auto-fill"),
                                    _ => false,
                                });
                                let is_auto_fit = count_tokens.iter().any(|tok| match tok {
                                    Token::Ident(name) => name.eq_ignore_ascii_case("auto-fit"),
                                    _ => false,
                                });
                                let (repeated_tracks, repeated_lines) = self.parse_raw_tracks_with_lines(track_tokens);
                                if is_auto_fill {
                                    for t in repeated_tracks {
                                        tracks.push(GridTrackSize::RepeatAutoFill(Box::new(t)));
                                    }
                                } else if is_auto_fit {
                                    for t in repeated_tracks {
                                        tracks.push(GridTrackSize::RepeatAutoFit(Box::new(t)));
                                    }
                                } else {
                                    let count = match count_tokens.first() {
                                        Some(Token::Number(n)) => (*n as usize).max(1),
                                        _ => 1,
                                    };
                                    for _ in 0..count {
                                        let base_idx = tracks.len();
                                        tracks.extend(repeated_tracks.clone());
                                        for (lname, lidx) in &repeated_lines {
                                            lines.push((lname.clone(), base_idx + *lidx));
                                        }
                                    }
                                }
                            }
                            i = end + 1;
                            continue;
                        }
                    }
                }
                Token::Ident(name) if name.eq_ignore_ascii_case("minmax") => {
                    if i + 1 < filtered.len() && *filtered[i + 1] == Token::OpenParen {
                        let close_idx = filtered.iter().enumerate().skip(i + 2)
                            .find(|(_, tok)| ***tok == Token::CloseParen)
                            .map(|(j, _)| j);
                        if let Some(end) = close_idx {
                            let inner = &filtered[i + 2..end];
                            if let Some(comma_idx) = inner.iter().position(|t| **t == Token::Comma) {
                                let min_tokens = &inner[..comma_idx];
                                let max_tokens = &inner[comma_idx + 1..];
                                if let (Some(min_tok), Some(max_tok)) = (min_tokens.first(), max_tokens.first())
                                    && let (Some(min_t), Some(max_t)) = (self.parse_single_track(min_tok), self.parse_single_track(max_tok))
                                {
                                    tracks.push(GridTrackSize::MinMax(Box::new(min_t), Box::new(max_t)));
                                }
                            }
                            i = end + 1;
                            continue;
                        }
                    }
                }
                token => {
                    if let Some(track) = self.parse_single_track(token) {
                        tracks.push(track);
                    }
                }
            }
            i += 1;
        }

        (tracks, lines)
    }

    #[allow(dead_code)]
    fn parse_raw_tracks(&self, filtered: &[&Token]) -> Vec<GridTrackSize> {
        self.parse_raw_tracks_with_lines(filtered).0
    }

    fn parse_grid_auto_flow(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }

        let mut is_column = false;
        let mut is_dense = false;

        for tok in filtered {
            if let Token::Ident(ident) = tok {
                match ident.to_ascii_lowercase().as_str() {
                    "column" => is_column = true,
                    "row" => is_column = false,
                    "dense" => is_dense = true,
                    _ => return None,
                }
            } else {
                return None;
            }
        }

        let flow = match (is_column, is_dense) {
            (true, true) => GridAutoFlow::ColumnDense,
            (true, false) => GridAutoFlow::Column,
            (false, true) => GridAutoFlow::RowDense,
            (false, false) => GridAutoFlow::Row,
        };

        Some(Value::GridAutoFlow(flow))
    }

    fn parse_single_track(&self, token: &Token) -> Option<GridTrackSize> {
        match token {
            Token::Dimension { value, unit } => {
                if unit.eq_ignore_ascii_case("fr") {
                    Some(GridTrackSize::Fr(*value))
                } else if let Some(Value::Length(len)) = self.parse_single_value(token) {
                    Some(GridTrackSize::Length(len))
                } else {
                    None
                }
            }
            Token::Percentage(pct) => Some(GridTrackSize::Length(Length::Percent(*pct))),
            Token::Number(n) if *n == 0.0 => Some(GridTrackSize::Length(Length::Px(0.0))),
            Token::Ident(word) => match word.to_ascii_lowercase().as_str() {
                "auto" => Some(GridTrackSize::Auto),
                "min-content" => Some(GridTrackSize::MinContent),
                "max-content" => Some(GridTrackSize::MaxContent),
                "subgrid" => Some(GridTrackSize::Subgrid),
                _ => None,
            },
            _ => None,
        }
    }

    /// Parses `grid-template: <rows> / <columns>` shorthand or with template areas.
    fn parse_grid_template_shorthand(&self, tokens: &[Token]) -> Option<Value> {
        let slash_pos = tokens.iter().position(|t| *t == Token::Delim('/'))?;
        let rows_tokens = &tokens[..slash_pos];
        let cols_tokens = &tokens[slash_pos + 1..];

        let rows = self.parse_grid_track_list(rows_tokens)
            .unwrap_or(Value::GridTrackList(Vec::new()));
        let cols = self.parse_grid_track_list(cols_tokens)
            .unwrap_or(Value::GridTrackList(Vec::new()));

        Some(Value::List(vec![rows, cols]))
    }

    /// Parses `grid-template-areas: "..." "..."`.
    fn parse_grid_template_areas(&self, tokens: &[Token]) -> Option<Value> {
        let strings: Vec<Value> = tokens
            .iter()
            .filter_map(|t| match t {
                Token::String(s) => Some(Value::String(s.clone())),
                Token::Ident(w) if w.eq_ignore_ascii_case("none") => Some(Value::Keyword("none".to_string())),
                _ => None,
            })
            .collect();
        if strings.is_empty() {
            None
        } else if strings.len() == 1 {
            Some(strings[0].clone())
        } else {
            Some(Value::List(strings))
        }
    }

    /// Parses grid placement shorthand (`1 / 3`, `1 / span 2`, `titlebar / sidebar`) or individual placement (`span 2`, `1`, `auto`, `content`).
    fn parse_grid_placement_or_shorthand(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let parts: Vec<&[&Token]> = filtered.split(|t| **t == Token::Delim('/')).collect();
        if parts.len() > 1 {
            let placements: Vec<Value> = parts
                .iter()
                .map(|p| Value::GridPlacement(self.parse_placement_component(p)))
                .collect();
            Some(Value::List(placements))
        } else {
            let placement = self.parse_placement_component(&filtered);
            Some(Value::GridPlacement(placement))
        }
    }

    fn parse_placement_component(&self, tokens: &[&Token]) -> GridPlacement {
        if tokens.is_empty() {
            return GridPlacement::Auto;
        }

        if tokens.len() == 1 {
            match tokens[0] {
                Token::Number(n) => return GridPlacement::Line(*n as i32),
                Token::Ident(s) if s.eq_ignore_ascii_case("auto") => return GridPlacement::Auto,
                Token::Ident(s) => return GridPlacement::Area(s.to_ascii_lowercase()),
                Token::String(s) => return GridPlacement::Area(s.to_ascii_lowercase()),
                _ => {}
            }
        }

        if tokens.len() == 2 {
            if let (Token::Ident(span_word), Token::Number(n)) = (tokens[0], tokens[1])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Span((*n as u16).max(1));
            }
            if let (Token::Ident(span_word), Token::Ident(name)) = (tokens[0], tokens[1])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Area(name.to_ascii_lowercase());
            }
            if let (Token::Delim('-'), Token::Number(n)) = (tokens[0], tokens[1]) {
                return GridPlacement::Line(-(*n as i32));
            }
        }

        if tokens.len() == 3 {
            if let (Token::Ident(span_word), Token::Number(n), Token::Ident(_)) = (tokens[0], tokens[1], tokens[2])
                && span_word.eq_ignore_ascii_case("span")
            {
                return GridPlacement::Span((*n as u16).max(1));
            }
            if let (Token::OpenBracket, Token::Ident(name), Token::CloseBracket) = (tokens[0], tokens[1], tokens[2]) {
                return GridPlacement::Area(name.to_ascii_lowercase());
            }
        }

        GridPlacement::Auto
    }

    fn parse_var_call(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        if tokens.is_empty() {
            return None;
        }
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() {
            return None;
        }
        match &tokens[idx] {
            Token::Ident(name) if name.eq_ignore_ascii_case("var") => {}
            _ => return None,
        }
        idx += 1;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
            return None;
        }
        idx += 1; // consume OpenParen

        // Find matching CloseParen, handling nested parens
        let mut depth = 1;
        let mut close_idx = None;
        for (j, tok) in tokens.iter().enumerate().skip(idx) {
            match tok {
                Token::OpenParen => depth += 1,
                Token::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close_idx = close_idx?;
        let inner_tokens = &tokens[idx..close_idx];
        let total_consumed = close_idx + 1;

        // Find first comma at depth 0
        let mut inner_depth = 0;
        let mut comma_idx = None;
        for (k, tok) in inner_tokens.iter().enumerate() {
            match tok {
                Token::OpenParen => inner_depth += 1,
                Token::CloseParen => {
                    if inner_depth > 0 {
                        inner_depth -= 1;
                    }
                }
                Token::Comma if inner_depth == 0 => {
                    comma_idx = Some(k);
                    break;
                }
                _ => {}
            }
        }

        let (var_name_tokens, fallback_tokens) = match comma_idx {
            Some(k) => (&inner_tokens[..k], Some(&inner_tokens[k + 1..])),
            None => (inner_tokens, None),
        };

        let mut var_name = String::new();
        for tok in var_name_tokens {
            match tok {
                Token::Whitespace | Token::Comment(_) => {}
                Token::Ident(name) => {
                    var_name = name.clone();
                    break;
                }
                _ => {}
            }
        }

        if var_name.is_empty() {
            return None;
        }

        let fallback = if let Some(fb_tokens) = fallback_tokens {
            self.parse_value_from_tokens(fb_tokens).map(Box::new)
        } else {
            None
        };

        Some((Value::Var { name: var_name, fallback }, total_consumed))
    }

    fn parse_color_fn(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        if tokens.is_empty() {
            return None;
        }
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() {
            return None;
        }
        let fn_name = match &tokens[idx] {
            Token::Ident(name)
                if name.eq_ignore_ascii_case("rgb")
                    || name.eq_ignore_ascii_case("rgba")
                    || name.eq_ignore_ascii_case("hsl")
                    || name.eq_ignore_ascii_case("hsla")
                    || name.eq_ignore_ascii_case("color")
                    || name.eq_ignore_ascii_case("oklab")
                    || name.eq_ignore_ascii_case("oklch")
                    || name.eq_ignore_ascii_case("color-mix") =>
            {
                name.clone()
            }
            _ => return None,
        };
        idx += 1;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
            return None;
        }
        idx += 1; // consume OpenParen

        let mut depth = 1;
        let mut close_idx = None;
        for (j, tok) in tokens.iter().enumerate().skip(idx) {
            match tok {
                Token::OpenParen => depth += 1,
                Token::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close_idx = close_idx?;
        let mut call_str = fn_name;
        for t in &tokens[idx - 1..=close_idx] {
            call_str.push_str(&self.token_to_string(t));
        }
        let col = Value::parse_color(&call_str)?;
        Some((Value::Color(col), close_idx + 1))
    }

    fn parse_url_fn(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() {
            return None;
        }
        match &tokens[idx] {
            Token::Ident(name) if name.eq_ignore_ascii_case("url") => {}
            _ => return None,
        }
        idx += 1;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
            return None;
        }
        idx += 1; // consume OpenParen

        let mut depth = 1;
        let mut close_idx = None;
        for (j, tok) in tokens.iter().enumerate().skip(idx) {
            match tok {
                Token::OpenParen => depth += 1,
                Token::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close_idx = close_idx?;
        let inner_tokens = &tokens[idx..close_idx];
        let mut url_str = String::new();
        for t in inner_tokens {
            match t {
                Token::Whitespace | Token::Comment(_) => {}
                Token::String(s) => url_str.push_str(s),
                _ => url_str.push_str(&self.token_to_string(t)),
            }
        }
        let url_str = url_str.trim().trim_matches('"').trim_matches('\'').to_string();
        Some((Value::Url(url_str), close_idx + 1))
    }

    fn parse_calc_call(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() {
            return None;
        }
        match &tokens[idx] {
            Token::Ident(name) if name.eq_ignore_ascii_case("calc") => {}
            _ => return None,
        }
        idx += 1;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen {
            return None;
        }
        idx += 1; // consume OpenParen

        let mut depth = 1;
        let mut close_idx = None;
        for (j, tok) in tokens.iter().enumerate().skip(idx) {
            match tok {
                Token::OpenParen => depth += 1,
                Token::CloseParen => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close_idx = close_idx?;
        let inner_tokens = &tokens[idx..close_idx];

        let mut calc = crate::values::CalcLength::default();
        let mut current_sign = 1.0f32;

        let filtered: Vec<&Token> = inner_tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        for tok in filtered {
            match tok {
                Token::Delim('+') => current_sign = 1.0,
                Token::Delim('-') => current_sign = -1.0,
                Token::Ident(s) if s == "+" => current_sign = 1.0,
                Token::Ident(s) if s == "-" => current_sign = -1.0,
                Token::Dimension { value, unit } => {
                    let val = *value * current_sign;
                    match unit.to_ascii_lowercase().as_str() {
                        "px" => calc.px += val,
                        "em" => calc.em += val,
                        "rem" => calc.rem += val,
                        "vw" => calc.vw += val,
                        "vh" => calc.vh += val,
                        "pt" => calc.px += val * 4.0 / 3.0,
                        "in" => calc.px += val * 96.0,
                        "cm" => calc.px += val * 96.0 / 2.54,
                        "mm" => calc.px += val * 9.6 / 2.54,
                        _ => calc.px += val,
                    }
                    current_sign = 1.0;
                }
                Token::Percentage(pct) => {
                    calc.percent += *pct * current_sign;
                    current_sign = 1.0;
                }
                Token::Number(n) => {
                    calc.px += *n * current_sign;
                    current_sign = 1.0;
                }
                _ => {}
            }
        }

        Some((Value::Length(Length::Calc(calc)), close_idx + 1))
    }

    fn parse_value_from_tokens(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();

        if filtered.is_empty() {
            return None;
        }

        // Check if a single var(...) covers all non-whitespace tokens
        if let Some((var_val, consumed)) = self.parse_var_call(tokens) {
            let remaining = &tokens[consumed..];
            if remaining.iter().all(|t| matches!(t, Token::Whitespace | Token::Comment(_))) {
                return Some(var_val);
            }
        }

        // Check if a single url(...) covers all non-whitespace tokens
        if let Some((url_val, consumed)) = self.parse_url_fn(tokens) {
            let remaining = &tokens[consumed..];
            if remaining.iter().all(|t| matches!(t, Token::Whitespace | Token::Comment(_))) {
                return Some(url_val);
            }
        }

        // Check if a single calc(...) covers all non-whitespace tokens
        if let Some((calc_val, consumed)) = self.parse_calc_call(tokens) {
            let remaining = &tokens[consumed..];
            if remaining.iter().all(|t| matches!(t, Token::Whitespace | Token::Comment(_))) {
                return Some(calc_val);
            }
        }

        // Check for gradient functions: linear-gradient / radial-gradient
        if let Some((grad_val, consumed)) = self.parse_gradient_fn(tokens) {
            let remaining = &tokens[consumed..];
            if remaining.iter().all(|t| matches!(t, Token::Whitespace | Token::Comment(_))) {
                return Some(grad_val);
            }
        }

        // NOTE: `transform`, `text-shadow`, and `filter` values are parsed by
        // property-specific branches in `parse_declarations_until`. Parsing them
        // generically here would let e.g. `display: none` or `content: none` be
        // mis-read as `transform: none`.

        if filtered.len() == 1 {
            return self.parse_single_value(filtered[0]);
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
                call_str.push_str(&self.token_to_string(t));
            }
            if let Some(col) = Value::parse_color(&call_str) {
                return Some(Value::Color(col));
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

            if let Some((var_val, consumed)) = self.parse_var_call(&tokens[i..]) {
                list.push(var_val);
                i += consumed;
                continue;
            }

            if let Some((color_val, consumed)) = self.parse_color_fn(&tokens[i..]) {
                list.push(color_val);
                i += consumed;
                continue;
            }

            if let Some((url_val, consumed)) = self.parse_url_fn(&tokens[i..]) {
                list.push(url_val);
                i += consumed;
                continue;
            }

            if let Some((grad_val, consumed)) = self.parse_gradient_fn(&tokens[i..]) {
                list.push(grad_val);
                i += consumed;
                continue;
            }

            if let Some((calc_val, consumed)) = self.parse_calc_call(&tokens[i..]) {
                list.push(calc_val);
                i += consumed;
                continue;
            }

            if matches!(tokens[i], Token::Comma) {
                i += 1;
                continue;
            }

            if let Some(val) = self.parse_single_value(&tokens[i]) {
                list.push(val);
            } else {
                return None;
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

    fn parse_single_value(&self, token: &Token) -> Option<Value> {
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
                    "isolate-override" => return Some(Value::UnicodeBidi(UnicodeBidi::IsolateOverride)),
                    "plaintext" => return Some(Value::UnicodeBidi(UnicodeBidi::Plaintext)),
                    _ => {}
                }

                // Flex Direction
                match lower.as_str() {
                    "row" => return Some(Value::FlexDirection(FlexDirection::Row)),
                    "row-reverse" => return Some(Value::FlexDirection(FlexDirection::RowReverse)),
                    "column" => return Some(Value::FlexDirection(FlexDirection::Column)),
                    "column-reverse" => return Some(Value::FlexDirection(FlexDirection::ColumnReverse)),
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
                    "space-between" => return Some(Value::JustifyContent(JustifyContent::SpaceBetween)),
                    "space-around" => return Some(Value::JustifyContent(JustifyContent::SpaceAround)),
                    "space-evenly" => return Some(Value::JustifyContent(JustifyContent::SpaceEvenly)),
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
                if lower.as_str() == "both" { return Some(Value::Clear(Clear::Both)) }

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
                    "line-through" => return Some(Value::TextDecoration(TextDecoration::LineThrough)),
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
                    "no-repeat" => return Some(Value::BackgroundRepeat(BackgroundRepeat::NoRepeat)),
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
                    "fill" => return Some(Value::ObjectFit(crate::values::ObjectFit::Fill)),
                    "scale-down" => return Some(Value::ObjectFit(crate::values::ObjectFit::ScaleDown)),
                    _ => {}
                }

                // Generic keyword
                Some(Value::Keyword(lower))
            }
            Token::Delim('/') => Some(Value::Keyword("/".to_string())),
            _ => None,
        }
    }

    // ── Gradient parsing ───────────────────────────────────────────────────────

    /// Tries to parse `linear-gradient(...)` or `radial-gradient(...)` or their
    /// `-webkit-` prefixed variants starting at `tokens[0]`.
    /// Returns `(Value::Gradient(...), tokens_consumed)` on success.
    fn parse_gradient_fn(&self, tokens: &[Token]) -> Option<(Value, usize)> {
        let mut idx = 0;
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) {
            idx += 1;
        }
        let fn_name = if let Token::Ident(name) = &tokens.get(idx)? {
            name.to_ascii_lowercase()
        } else {
            return None;
        };
        let is_conic = fn_name.contains("conic-gradient");
        let is_linear = fn_name.contains("linear-gradient");
        let is_radial = fn_name.contains("radial-gradient");
        let repeating = fn_name.contains("repeating-");
        if !is_linear && !is_radial && !is_conic {
            return None;
        }
        idx += 1; // consume function name
        // Consume whitespace then OpenParen
        while idx < tokens.len() && matches!(tokens[idx], Token::Whitespace | Token::Comment(_)) { idx += 1; }
        if idx >= tokens.len() || tokens[idx] != Token::OpenParen { return None; }
        idx += 1;

        // Collect all tokens up to the matching CloseParen
        let mut depth = 1usize;
        let mut inner: Vec<Token> = Vec::new();
        while idx < tokens.len() {
            match &tokens[idx] {
                Token::OpenParen => { depth += 1; inner.push(tokens[idx].clone()); idx += 1; }
                Token::CloseParen => {
                    depth -= 1;
                    idx += 1;
                    if depth == 0 { break; }
                    inner.push(Token::CloseParen);
                }
                tok => { inner.push(tok.clone()); idx += 1; }
            }
        }

        // Parse angle and color stops from inner tokens
        let inner_str: String = inner.iter().map(|t| self.token_to_string(t)).collect();
        let gradient = if is_conic {
            parse_conic_gradient_inner(&inner_str, repeating)
        } else if is_linear {
            parse_linear_gradient_inner_repeating(&inner_str, repeating)
        } else {
            parse_radial_gradient_inner_repeating(&inner_str, repeating)
        };
        Some((Value::Gradient(Box::new(gradient)), idx))
    }

    // ── Transform parsing ─────────────────────────────────────────────────────

    /// Tries to parse `transform: func(...) func(...) ...` from the token slice.
    fn try_parse_transform_list(&self, tokens: &[Token]) -> Option<Value> {
        use crate::values::{Transform, TransformFunction};
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        let text_lower = text.trim().to_ascii_lowercase();
        if text_lower == "none" {
            return Some(Value::Transform(Transform::default()));
        }
        let has_transform = [
            "translate(", "translatex(", "translatey(", "translatez(", "translate3d(",
            "rotate(", "rotatex(", "rotatey(", "rotatez(", "rotate3d(",
            "scale(", "scalex(", "scaley(", "scalez(", "scale3d(",
            "skew(", "skewx(", "skewy(",
            "matrix(", "matrix3d(", "perspective(",
        ]
        .iter()
        .any(|f| text_lower.contains(f));
        if !has_transform {
            return None;
        }

        let mut funcs: Vec<TransformFunction> = Vec::new();
        let raw = text.trim();
        let mut remaining = raw;
        loop {
            remaining = remaining.trim_start();
            if remaining.is_empty() {
                break;
            }
            let Some(paren_pos) = remaining.find('(') else {
                break;
            };
            let fn_name = remaining[..paren_pos].trim().to_ascii_lowercase();
            let args_start = paren_pos + 1;
            let mut depth = 1;
            let mut end = args_start;
            for (i, ch) in remaining[args_start..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = args_start + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let args_str = remaining[args_start..end].trim();
            remaining = remaining[end + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());

            let parse_num = |s: &str| -> f32 {
                let trimmed = s.trim();
                let clean = trimmed.trim_end_matches("px").trim_end_matches('%');
                clean.parse::<f32>().unwrap_or(0.0)
            };

            let parse_len = |s: &str| -> Length {
                let trimmed = s.trim();
                if let Some(p) = trimmed.strip_suffix('%') {
                    Length::Percent(p.trim().parse::<f32>().unwrap_or(0.0))
                } else if let Some(px) = trimmed.strip_suffix("px") {
                    Length::Px(px.trim().parse::<f32>().unwrap_or(0.0))
                } else if let Some(em) = trimmed.strip_suffix("em") {
                    Length::Em(em.trim().parse::<f32>().unwrap_or(0.0))
                } else if let Some(rem) = trimmed.strip_suffix("rem") {
                    Length::Rem(rem.trim().parse::<f32>().unwrap_or(0.0))
                } else {
                    Length::Px(trimmed.parse::<f32>().unwrap_or(0.0))
                }
            };
            let is_len = |s: &str| -> bool {
                let trimmed = s.trim();
                trimmed.ends_with('%') || trimmed.ends_with("em") || trimmed.ends_with("rem")
            };

            let parse_angle = |s: &str| -> f32 {
                let trimmed = s.trim().to_ascii_lowercase();
                if let Some(deg_str) = trimmed.strip_suffix("deg") {
                    deg_str.trim().parse::<f32>().unwrap_or(0.0)
                } else if let Some(rad_str) = trimmed.strip_suffix("rad") {
                    rad_str.trim().parse::<f32>().unwrap_or(0.0) * (180.0 / std::f32::consts::PI)
                } else if let Some(grad_str) = trimmed.strip_suffix("grad") {
                    grad_str.trim().parse::<f32>().unwrap_or(0.0) * 0.9
                } else if let Some(turn_str) = trimmed.strip_suffix("turn") {
                    turn_str.trim().parse::<f32>().unwrap_or(0.0) * 360.0
                } else {
                    trimmed.parse::<f32>().unwrap_or(0.0)
                }
            };

            let parts: Vec<&str> = if args_str.contains(',') {
                args_str.split(',').collect()
            } else {
                args_str.split_whitespace().collect()
            };
            let get_num = |idx: usize| -> f32 {
                parts.get(idx).map(|s| parse_num(s)).unwrap_or(0.0)
            };
            let get_ang = |idx: usize| -> f32 {
                parts.get(idx).map(|s| parse_angle(s)).unwrap_or(0.0)
            };

            let f = match fn_name.as_str() {
                "translate" => {
                    let s0 = parts.get(0).copied().unwrap_or("");
                    let s1 = parts.get(1).copied().unwrap_or("");
                    if is_len(s0) || is_len(s1) {
                        TransformFunction::TranslateLen(parse_len(s0), parse_len(s1))
                    } else {
                        let tx = get_num(0);
                        let ty = if parts.len() > 1 { get_num(1) } else { 0.0 };
                        TransformFunction::Translate(tx, ty)
                    }
                }
                "translatex" => {
                    let s0 = parts.get(0).copied().unwrap_or("");
                    if is_len(s0) {
                        TransformFunction::TranslateXLen(parse_len(s0))
                    } else {
                        TransformFunction::TranslateX(get_num(0))
                    }
                }
                "translatey" => {
                    let s0 = parts.get(0).copied().unwrap_or("");
                    if is_len(s0) {
                        TransformFunction::TranslateYLen(parse_len(s0))
                    } else {
                        TransformFunction::TranslateY(get_num(0))
                    }
                }
                "translatez" => TransformFunction::TranslateZ(get_num(0)),
                "translate3d" => TransformFunction::Translate3d(get_num(0), get_num(1), get_num(2)),
                "rotate" => TransformFunction::Rotate(get_ang(0)),
                "rotatex" => TransformFunction::RotateX(get_ang(0)),
                "rotatey" => TransformFunction::RotateY(get_ang(0)),
                "rotatez" => TransformFunction::RotateZ(get_ang(0)),
                "rotate3d" => TransformFunction::Rotate3d(get_num(0), get_num(1), get_num(2), get_ang(3)),
                "scale" => {
                    let sx = get_num(0);
                    let sy = if parts.len() > 1 { get_num(1) } else { sx };
                    TransformFunction::Scale(sx, sy)
                }
                "scalex" => TransformFunction::ScaleX(get_num(0)),
                "scaley" => TransformFunction::ScaleY(get_num(0)),
                "scalez" => TransformFunction::ScaleZ(get_num(0)),
                "scale3d" => TransformFunction::Scale3d(get_num(0), get_num(1), get_num(2)),
                "skew" => {
                    let ax = get_ang(0);
                    let ay = if parts.len() > 1 { get_ang(1) } else { 0.0 };
                    TransformFunction::Skew(ax, ay)
                }
                "skewx" => TransformFunction::Skew(get_ang(0), 0.0),
                "skewy" => TransformFunction::Skew(0.0, get_ang(0)),
                "matrix" => TransformFunction::Matrix(
                    get_num(0), get_num(1), get_num(2), get_num(3), get_num(4), get_num(5),
                ),
                "matrix3d" => {
                    let mut m = [0.0f32; 16];
                    for (i, p) in parts.iter().enumerate().take(16) {
                        m[i] = parse_num(p);
                    }
                    TransformFunction::Matrix3d(m)
                }
                "perspective" => TransformFunction::Perspective(get_num(0)),
                _ => break,
            };
            funcs.push(f);
        }
        if funcs.is_empty() {
            return None;
        }
        Some(Value::Transform(Transform(funcs)))
    }

    // ── Text-shadow parsing ───────────────────────────────────────────────────

    /// Tries to parse `text-shadow: offset-x offset-y blur? color?` from token slice.
    fn try_parse_text_shadow(&self, tokens: &[Token]) -> Option<Value> {
        use crate::values::TextShadow;
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        let lower = text.trim().to_ascii_lowercase();
        if lower == "none" { return Some(Value::TextShadow(TextShadow { offset_x: 0.0, offset_y: 0.0, blur_radius: 0.0, color: mango_core::Color::TRANSPARENT })); }
        // Must have at least 2 length values; check for length tokens
        let len_count = tokens.iter().filter(|t| matches!(t, Token::Dimension { .. })).count();
        if len_count < 2 { return None; }
        let mut offset_x = None;
        let mut offset_y = None;
        let mut blur = 0.0f32;
        let mut color = mango_core::Color::rgba(0, 0, 0, 180);
        let mut len_idx = 0;
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                Token::Dimension { value, unit } => {
                    let px = match unit.to_ascii_lowercase().as_str() {
                        "px" => *value, "em" => *value * 16.0, "rem" => *value * 16.0, _ => *value,
                    };
                    match len_idx { 0 => offset_x = Some(px), 1 => offset_y = Some(px), 2 => blur = px, _ => {} }
                    len_idx += 1;
                }
                Token::Number(n) if *n == 0.0 => {
                    match len_idx { 0 => offset_x = Some(0.0), 1 => offset_y = Some(0.0), _ => {} }
                    len_idx += 1;
                }
                Token::Hash(hex) => {
                    if let Some(c) = Value::parse_color(&format!("#{hex}")) { color = c; }
                }
                Token::Ident(name) => {
                    let lower = name.to_ascii_lowercase();
                    if lower != "none" && lower != "inherit" && lower != "initial" {
                        if let Some(c) = Value::parse_color(&lower) { color = c; }
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if let (Some(ox), Some(oy)) = (offset_x, offset_y) {
            Some(Value::TextShadow(TextShadow { offset_x: ox, offset_y: oy, blur_radius: blur, color }))
        } else {
            None
        }
    }

    fn try_parse_word_break(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        WordBreak::parse(&text).map(Value::WordBreak)
    }

    fn try_parse_overflow_wrap(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        OverflowWrap::parse(&text).map(Value::OverflowWrap)
    }

    fn try_parse_hyphens(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        Hyphens::parse(&text).map(Value::Hyphens)
    }

    fn try_parse_line_clamp(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        LineClamp::parse(&text).map(Value::LineClamp)
    }

    fn try_parse_font_variation_settings(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        FontVariationSettings::parse(&text).map(Value::FontVariationSettings)
    }

    fn try_parse_font_feature_settings(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        FontFeatureSettings::parse(&text).map(Value::FontFeatureSettings)
    }

    fn try_parse_text_emphasis_style(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        TextEmphasisStyle::parse(&text).map(Value::TextEmphasisStyle)
    }

    fn try_parse_text_decoration_thickness(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        TextDecorationThickness::parse(&text).map(Value::TextDecorationThickness)
    }

    // ── Filter parsing ────────────────────────────────────────────────────────

    /// Tries to parse `filter: blur(4px) brightness(0.8) ...` from token slice.
    fn try_parse_filter_list(&self, tokens: &[Token]) -> Option<Value> {
        use crate::values::FilterFunction;
        let raw: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case("none") { return Some(Value::Filter(vec![])); }
        // Must have function-call syntax
        if !raw.contains('(') { return None; }
        let mut funcs: Vec<FilterFunction> = Vec::new();
        let mut remaining = raw;
        loop {
            remaining = remaining.trim_start();
            if remaining.is_empty() { break; }
            let paren = match remaining.find('(') {
                Some(p) => p,
                None => break,
            };
            let fn_name = remaining[..paren].trim().to_ascii_lowercase();
            let args_start = paren + 1;
            let mut depth = 1;
            let mut end = args_start;
            for (i, ch) in remaining[args_start..].char_indices() {
                match ch { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { end = args_start + i; break; } } _ => {} }
            }
            let arg = remaining[args_start..end].trim().trim_end_matches("px").trim_end_matches('%').trim_end_matches("deg");
            let val: f32 = arg.parse().unwrap_or(0.0);
            // Normalise percentage values for functions that expect 0..1
            let norm = |v: f32, is_pct: bool| -> f32 {
                if is_pct || remaining[args_start..end].contains('%') { v / 100.0 } else { v }
            };
            if let Some(f) = match fn_name.as_str() {
                "blur" => Some(FilterFunction::Blur(val)),
                "brightness" => Some(FilterFunction::Brightness(norm(val, false))),
                "contrast" => Some(FilterFunction::Contrast(norm(val, false))),
                "grayscale" => Some(FilterFunction::Grayscale(norm(val, false))),
                "opacity" => Some(FilterFunction::Opacity(norm(val, false))),
                "saturate" => Some(FilterFunction::Saturate(norm(val, false))),
                "sepia" => Some(FilterFunction::Sepia(norm(val, false))),
                "hue-rotate" => Some(FilterFunction::HueRotate(val)),
                "invert" => Some(FilterFunction::Invert(norm(val, false))),
                "drop-shadow" => {
                    let parts: Vec<&str> = remaining[args_start..end].split_whitespace().collect();
                    let px = |s: &str| -> f32 { s.trim_end_matches("px").parse().unwrap_or(0.0) };
                    let ox = parts.first().map(|s| px(s)).unwrap_or(0.0);
                    let oy = parts.get(1).map(|s| px(s)).unwrap_or(0.0);
                    let blur = parts.get(2).map(|s| px(s)).unwrap_or(0.0);
                    let color = parts.get(3).and_then(|s| Value::parse_color(s)).unwrap_or(mango_core::Color::rgba(0,0,0,128));
                    Some(FilterFunction::DropShadow { offset_x: ox, offset_y: oy, blur, color })
                }
                _ => None,
            } { funcs.push(f); }
            remaining = remaining[end + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        }
        if funcs.is_empty() { return None; }
        Some(Value::Filter(funcs))
    }

    /// Tries to parse `clip-path` (e.g. `none`, `url(...)`, `circle(...)`, `ellipse(...)`, `inset(...)`, `polygon(...)`).
    fn try_parse_clip_path(&self, tokens: &[Token]) -> Option<Value> {
        use crate::values::{ClipPath, Length};
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        let trimmed = text.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "none" {
            return Some(Value::ClipPath(ClipPath::None));
        }
        if lower.starts_with("url(") && lower.ends_with(')') {
            let inner = trimmed[4..trimmed.len() - 1].trim().trim_matches('"').trim_matches('\'');
            return Some(Value::ClipPath(ClipPath::Url(inner.to_string())));
        }

        let parse_len = |s: &str| -> Length {
            let t = s.trim();
            if let Some(pct) = t.strip_suffix('%') {
                Length::Percent(pct.parse().unwrap_or(0.0))
            } else if let Some(px) = t.strip_suffix("px") {
                Length::Px(px.parse().unwrap_or(0.0))
            } else if let Some(em) = t.strip_suffix("em") {
                Length::Em(em.parse().unwrap_or(0.0))
            } else if let Some(rem) = t.strip_suffix("rem") {
                Length::Rem(rem.parse().unwrap_or(0.0))
            } else if let Some(vw) = t.strip_suffix("vw") {
                Length::Vw(vw.parse().unwrap_or(0.0))
            } else if let Some(vh) = t.strip_suffix("vh") {
                Length::Vh(vh.parse().unwrap_or(0.0))
            } else if t.eq_ignore_ascii_case("center") {
                Length::Percent(50.0)
            } else if t.eq_ignore_ascii_case("left") || t.eq_ignore_ascii_case("top") {
                Length::Percent(0.0)
            } else if t.eq_ignore_ascii_case("right") || t.eq_ignore_ascii_case("bottom") {
                Length::Percent(100.0)
            } else {
                Length::Px(t.parse().unwrap_or(0.0))
            }
        };

        if lower.starts_with("circle(") && lower.ends_with(')') {
            let inner = trimmed[7..trimmed.len() - 1].trim();
            let (radius_str, pos_str) = if let Some(at_idx) = inner.to_ascii_lowercase().find(" at ") {
                (&inner[..at_idx], Some(&inner[at_idx + 4..]))
            } else {
                (inner, None)
            };
            let radius = if radius_str.is_empty() {
                Length::Percent(50.0)
            } else {
                parse_len(radius_str)
            };
            let (cx, cy) = if let Some(p) = pos_str {
                let parts: Vec<&str> = p.split_whitespace().collect();
                let x = parts.first().map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
                let y = parts.get(1).map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
                (x, y)
            } else {
                (Length::Percent(50.0), Length::Percent(50.0))
            };
            return Some(Value::ClipPath(ClipPath::Circle { radius, center_x: cx, center_y: cy }));
        }

        if lower.starts_with("ellipse(") && lower.ends_with(')') {
            let inner = trimmed[8..trimmed.len() - 1].trim();
            let (radii_str, pos_str) = if let Some(at_idx) = inner.to_ascii_lowercase().find(" at ") {
                (&inner[..at_idx], Some(&inner[at_idx + 4..]))
            } else {
                (inner, None)
            };
            let radii_parts: Vec<&str> = radii_str.split_whitespace().collect();
            let rx = radii_parts.first().map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
            let ry = radii_parts.get(1).map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
            let (cx, cy) = if let Some(p) = pos_str {
                let parts: Vec<&str> = p.split_whitespace().collect();
                let x = parts.first().map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
                let y = parts.get(1).map(|s| parse_len(s)).unwrap_or(Length::Percent(50.0));
                (x, y)
            } else {
                (Length::Percent(50.0), Length::Percent(50.0))
            };
            return Some(Value::ClipPath(ClipPath::Ellipse { radius_x: rx, radius_y: ry, center_x: cx, center_y: cy }));
        }

        if lower.starts_with("inset(") && lower.ends_with(')') {
            let inner = trimmed[6..trimmed.len() - 1].trim();
            let (insets_str, round_str) = if let Some(r_idx) = inner.to_ascii_lowercase().find(" round ") {
                (&inner[..r_idx], Some(&inner[r_idx + 7..]))
            } else {
                (inner, None)
            };
            let parts: Vec<&str> = insets_str.split_whitespace().collect();
            let (t, r, b, l) = match parts.len() {
                1 => {
                    let v = parse_len(parts[0]);
                    (v, v, v, v)
                }
                2 => {
                    let tb = parse_len(parts[0]);
                    let rl = parse_len(parts[1]);
                    (tb, rl, tb, rl)
                }
                3 => {
                    let top = parse_len(parts[0]);
                    let rl = parse_len(parts[1]);
                    let bot = parse_len(parts[2]);
                    (top, rl, bot, rl)
                }
                4.. => {
                    (parse_len(parts[0]), parse_len(parts[1]), parse_len(parts[2]), parse_len(parts[3]))
                }
                _ => (Length::Px(0.0), Length::Px(0.0), Length::Px(0.0), Length::Px(0.0)),
            };
            let round = round_str.map(|r_text| {
                let r_parts: Vec<&str> = r_text.split_whitespace().collect();
                match r_parts.len() {
                    1 => {
                        let rad = parse_len(r_parts[0]);
                        [rad, rad, rad, rad]
                    }
                    2 => {
                        let r1 = parse_len(r_parts[0]);
                        let r2 = parse_len(r_parts[1]);
                        [r1, r2, r1, r2]
                    }
                    3 => {
                        let r1 = parse_len(r_parts[0]);
                        let r2 = parse_len(r_parts[1]);
                        let r3 = parse_len(r_parts[2]);
                        [r1, r2, r3, r2]
                    }
                    4.. => [
                        parse_len(r_parts[0]),
                        parse_len(r_parts[1]),
                        parse_len(r_parts[2]),
                        parse_len(r_parts[3]),
                    ],
                    _ => [Length::Px(0.0); 4],
                }
            });
            return Some(Value::ClipPath(ClipPath::Inset { top: t, right: r, bottom: b, left: l, round }));
        }

        if lower.starts_with("polygon(") && lower.ends_with(')') {
            let inner = trimmed[8..trimmed.len() - 1].trim();
            let mut pairs = Vec::new();
            for item in inner.split(',') {
                let item = item.trim();
                if item.eq_ignore_ascii_case("evenodd") || item.eq_ignore_ascii_case("nonzero") {
                    continue;
                }
                let coords: Vec<&str> = item.split_whitespace().collect();
                if coords.len() >= 2 {
                    pairs.push((parse_len(coords[0]), parse_len(coords[1])));
                }
            }
            if !pairs.is_empty() {
                return Some(Value::ClipPath(ClipPath::Polygon(pairs)));
            }
        }

        None
    }

    /// Tries to parse `mix-blend-mode` (e.g. `normal`, `multiply`, `screen`, etc.).
    fn try_parse_blend_mode(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        crate::values::BlendMode::parse(&text).map(Value::BlendMode)
    }

    /// Tries to parse `background-blend-mode` (single blend mode or comma-separated list).
    fn try_parse_background_blend_mode(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        let mut modes = Vec::new();
        for g in groups {
            let text: String = g.iter().map(|t| self.token_to_string(t)).collect();
            if let Some(m) = crate::values::BlendMode::parse(&text) {
                modes.push(Value::BlendMode(m));
            }
        }
        if modes.is_empty() {
            None
        } else if modes.len() == 1 {
            Some(modes.remove(0))
        } else {
            Some(Value::List(modes))
        }
    }

    /// Tries to parse `background-attachment` (single or comma-separated list).
    fn try_parse_background_attachment(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        let mut list = Vec::new();
        for g in groups {
            let text: String = g.iter().map(|t| self.token_to_string(t)).collect();
            if let Some(a) = crate::values::BackgroundAttachment::parse(&text) {
                list.push(Value::BackgroundAttachment(a));
            }
        }
        if list.is_empty() {
            None
        } else if list.len() == 1 {
            Some(list.remove(0))
        } else {
            Some(Value::List(list))
        }
    }

    /// Tries to parse `background-clip` (single or comma-separated list).
    fn try_parse_background_clip(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        let mut list = Vec::new();
        for g in groups {
            let text: String = g.iter().map(|t| self.token_to_string(t)).collect();
            if let Some(c) = crate::values::BackgroundClip::parse(&text) {
                list.push(Value::BackgroundClip(c));
            }
        }
        if list.is_empty() {
            None
        } else if list.len() == 1 {
            Some(list.remove(0))
        } else {
            Some(Value::List(list))
        }
    }

    /// Tries to parse `mask-composite` (single or comma-separated list).
    fn try_parse_mask_composite(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        let mut list = Vec::new();
        for g in groups {
            let text: String = g.iter().map(|t| self.token_to_string(t)).collect();
            if let Some(m) = crate::values::MaskComposite::parse(&text) {
                list.push(Value::MaskComposite(m));
            }
        }
        if list.is_empty() {
            None
        } else if list.len() == 1 {
            Some(list.remove(0))
        } else {
            Some(Value::List(list))
        }
    }

    /// Tries to parse `isolation` (auto | isolate).
    fn try_parse_isolation(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        crate::values::Isolation::parse(&text).map(Value::Isolation)
    }

    /// Tries to parse comma-separated `background-image` list (url, gradient, none).
    fn try_parse_layered_background_images(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        if groups.len() <= 1 {
            return None;
        }
        let mut images = Vec::new();
        for g in groups {
            if let Some(val) = self.parse_value_from_tokens(&g) {
                images.push(val);
            }
        }
        if images.is_empty() {
            None
        } else {
            Some(Value::List(images))
        }
    }

    /// Tries to parse `border-image` shorthand or longhand.
    fn try_parse_border_image(&self, tokens: &[Token]) -> Option<Value> {
        let mut bi = crate::values::BorderImage::default();
        let mut i = 0;
        let mut found_any = false;

        while i < tokens.len() {
            if matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
                i += 1;
                continue;
            }
            if let Some((Value::Url(url), consumed)) = self.parse_url_fn(&tokens[i..]) {
                bi.source = Some(url);
                found_any = true;
                i += consumed;
                continue;
            }
            if let Some((Value::Gradient(g), consumed)) = self.parse_gradient_fn(&tokens[i..]) {
                bi.gradient = Some(g);
                found_any = true;
                i += consumed;
                continue;
            }
            match &tokens[i] {
                Token::String(s) => {
                    bi.source = Some(s.clone());
                    found_any = true;
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("none") => {
                    bi.source = None;
                    found_any = true;
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("fill") => {
                    bi.fill = true;
                    found_any = true;
                    i += 1;
                }
                Token::Ident(id) if matches!(id.to_ascii_lowercase().as_str(), "stretch" | "repeat" | "round" | "space") => {
                    if let Some(r) = crate::values::BorderImageRepeat::parse(id) {
                        bi.repeat_h = r;
                        bi.repeat_v = r;
                        found_any = true;
                    }
                    i += 1;
                }
                Token::Number(n) => {
                    let val = crate::values::Length::Px(*n);
                    bi.slice = [val, val, val, val];
                    found_any = true;
                    i += 1;
                }
                Token::Percentage(p) => {
                    let val = crate::values::Length::Percent(*p);
                    bi.slice = [val, val, val, val];
                    found_any = true;
                    i += 1;
                }
                Token::Delim('/') => {
                    i += 1;
                    while i < tokens.len() && matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
                        i += 1;
                    }
                    if i < tokens.len() {
                        if let Token::Number(w) = &tokens[i] {
                            let len = crate::values::Length::Px(*w);
                            bi.width = [len, len, len, len];
                            i += 1;
                        } else if let Token::Dimension { value, unit } = &tokens[i] {
                            if unit.eq_ignore_ascii_case("px") {
                                let len = crate::values::Length::Px(*value);
                                bi.width = [len, len, len, len];
                                i += 1;
                            }
                        }
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        if found_any {
            Some(Value::BorderImage(bi))
        } else {
            None
        }
    }

    /// Tries to parse `mask-mode` (e.g. `alpha`, `luminance`, `match-source`).
    fn try_parse_mask_mode(&self, tokens: &[Token]) -> Option<Value> {
        let text: String = tokens.iter().map(|t| self.token_to_string(t)).collect();
        crate::values::MaskMode::parse(&text).map(Value::MaskMode)
    }

    /// Tries to parse `will-change` (e.g. `transform, opacity`, `scroll-position`, `auto`).
    fn try_parse_will_change(&self, tokens: &[Token]) -> Option<Value> {
        let groups = split_commas(tokens);
        let mut props = Vec::new();
        for g in groups {
            let text: String = g.iter().map(|t| self.token_to_string(t)).collect();
            let clean = text.trim();
            if !clean.is_empty() {
                props.push(Value::Keyword(clean.to_string()));
            }
        }
        if props.is_empty() {
            None
        } else if props.len() == 1 {
            Some(props.remove(0))
        } else {
            Some(Value::List(props))
        }
    }

    /// Tries to parse `white-space` keywords (normal, nowrap, pre, pre-wrap, pre-line).
    fn try_parse_white_space(&self, tokens: &[Token]) -> Option<Value> {
        let first = tokens.iter().find(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))?;
        if let Token::Ident(ident) = first {
            match ident.to_ascii_lowercase().as_str() {
                "normal" => Some(Value::WhiteSpace(WhiteSpace::Normal)),
                "nowrap" => Some(Value::WhiteSpace(WhiteSpace::Nowrap)),
                "pre" => Some(Value::WhiteSpace(WhiteSpace::Pre)),
                "pre-wrap" => Some(Value::WhiteSpace(WhiteSpace::PreWrap)),
                "pre-line" => Some(Value::WhiteSpace(WhiteSpace::PreLine)),
                "break-spaces" => Some(Value::WhiteSpace(WhiteSpace::BreakSpaces)),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Tries to parse `content` property for generated content (`::before` / `::after`).
    fn try_parse_content(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }

        if filtered.len() == 1 {
            if let Token::Ident(kw) = filtered[0] {
                if kw.eq_ignore_ascii_case("none") || kw.eq_ignore_ascii_case("normal") {
                    return Some(Value::Content(Vec::new()));
                }
            }
        }

        use crate::values::ContentItem;
        let mut items = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if matches!(tokens[i], Token::Whitespace | Token::Comment(_)) {
                i += 1;
                continue;
            }

            match &tokens[i] {
                Token::String(s) => {
                    items.push(ContentItem::String(s.clone()));
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("open-quote") => {
                    items.push(ContentItem::OpenQuote);
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("close-quote") => {
                    items.push(ContentItem::CloseQuote);
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("no-open-quote") => {
                    items.push(ContentItem::NoOpenQuote);
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("no-close-quote") => {
                    items.push(ContentItem::NoCloseQuote);
                    i += 1;
                }
                Token::Ident(id) if id.eq_ignore_ascii_case("none") || id.eq_ignore_ascii_case("normal") => {
                    i += 1;
                }
                Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("attr") => {
                    let mut j = i + 1;
                    while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_)) {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::OpenParen {
                        j += 1;
                        let mut attr_name = String::new();
                        while j < tokens.len() && tokens[j] != Token::CloseParen {
                            match &tokens[j] {
                                Token::Ident(s) | Token::String(s) => attr_name.push_str(s),
                                Token::Delim(c) => attr_name.push(*c),
                                _ => {}
                            }
                            j += 1;
                        }
                        if j < tokens.len() && tokens[j] == Token::CloseParen {
                            j += 1;
                        }
                        items.push(ContentItem::Attr(attr_name.trim().to_string()));
                        i = j;
                    } else {
                        i += 1;
                    }
                }
                Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("counter") => {
                    let mut j = i + 1;
                    while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_)) {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::OpenParen {
                        j += 1;
                        let mut name = String::new();
                        let mut style = None;
                        let mut is_style = false;
                        while j < tokens.len() && tokens[j] != Token::CloseParen {
                            match &tokens[j] {
                                Token::Comma => is_style = true,
                                Token::Ident(s) => {
                                    if is_style {
                                        style = Some(s.to_ascii_lowercase());
                                    } else {
                                        name = s.clone();
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        if j < tokens.len() && tokens[j] == Token::CloseParen {
                            j += 1;
                        }
                        items.push(ContentItem::Counter {
                            name: name.trim().to_string(),
                            style,
                        });
                        i = j;
                    } else {
                        i += 1;
                    }
                }
                Token::Ident(fn_name) if fn_name.eq_ignore_ascii_case("counters") => {
                    let mut j = i + 1;
                    while j < tokens.len() && matches!(tokens[j], Token::Whitespace | Token::Comment(_)) {
                        j += 1;
                    }
                    if j < tokens.len() && tokens[j] == Token::OpenParen {
                        j += 1;
                        let mut name = String::new();
                        let mut separator = String::new();
                        let mut style = None;
                        let mut param_idx = 0;
                        while j < tokens.len() && tokens[j] != Token::CloseParen {
                            match &tokens[j] {
                                Token::Comma => param_idx += 1,
                                Token::Ident(s) => {
                                    if param_idx == 0 {
                                        name = s.clone();
                                    } else if param_idx >= 2 {
                                        style = Some(s.to_ascii_lowercase());
                                    }
                                }
                                Token::String(s) => {
                                    if param_idx == 1 {
                                        separator = s.clone();
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        if j < tokens.len() && tokens[j] == Token::CloseParen {
                            j += 1;
                        }
                        items.push(ContentItem::Counters {
                            name: name.trim().to_string(),
                            separator,
                            style,
                        });
                        i = j;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    if let Some((Value::Url(url), consumed)) = self.parse_url_fn(&tokens[i..]) {
                        items.push(ContentItem::Url(url));
                        i += consumed;
                    } else {
                        i += 1;
                    }
                }
            }
        }

        Some(Value::Content(items))
    }

    /// Tries to parse `counter-reset` or `counter-increment` property values.
    fn try_parse_counter_actions(&self, tokens: &[Token], default_val: i32) -> Option<Value> {
        use crate::values::CounterAction;
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }

        if filtered.len() == 1 {
            if let Token::Ident(kw) = filtered[0] {
                if kw.eq_ignore_ascii_case("none") {
                    return Some(Value::CounterActions(Vec::new()));
                }
            }
        }

        let mut actions = Vec::new();
        let mut i = 0;
        while i < filtered.len() {
            if let Token::Ident(name) = filtered[i] {
                i += 1;
                let mut val = default_val;
                if i < filtered.len() {
                    if let Token::Number(n) = filtered[i] {
                        val = *n as i32;
                        i += 1;
                    } else if i + 1 < filtered.len()
                        && matches!(filtered[i], Token::Delim('-'))
                        && matches!(filtered[i + 1], Token::Number(_))
                    {
                        if let Token::Number(n) = filtered[i + 1] {
                            val = -(*n as i32);
                            i += 2;
                        }
                    }
                }
                actions.push(CounterAction {
                    name: name.clone(),
                    value: val,
                });
            } else {
                i += 1;
            }
        }

        Some(Value::CounterActions(actions))
    }

    /// Tries to parse `quotes` property value.
    fn try_parse_quotes(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }

        if filtered.len() == 1 {
            if let Token::Ident(kw) = filtered[0] {
                if kw.eq_ignore_ascii_case("none") {
                    return Some(Value::Quotes(Vec::new()));
                }
                if kw.eq_ignore_ascii_case("auto") {
                    return Some(Value::Keyword("auto".to_string()));
                }
            }
        }

        let mut strings = Vec::new();
        for tok in filtered {
            if let Token::String(s) = tok {
                strings.push(s.clone());
            }
        }

        let mut pairs = Vec::new();
        for chunk in strings.chunks_exact(2) {
            pairs.push((chunk[0].clone(), chunk[1].clone()));
        }
        Some(Value::Quotes(pairs))
    }

    /// Tries to parse `container-type` property value (`normal`, `size`, `inline-size`).
    fn try_parse_container_type(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        let tok = filtered.first()?;
        if let Token::Ident(ident) = tok {
            crate::values::ContainerType::parse(ident).map(Value::ContainerType)
        } else {
            None
        }
    }

    /// Tries to parse `container-name` property value (`none`, custom ident).
    fn try_parse_container_name(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        let tok = filtered.first()?;
        if let Token::Ident(ident) = tok {
            if ident.eq_ignore_ascii_case("none") {
                Some(Value::Keyword("none".to_string()))
            } else {
                Some(Value::String(ident.clone()))
            }
        } else {
            None
        }
    }

    /// Tries to parse `container` shorthand property value (`<name>? / <type>`).
    fn try_parse_container_shorthand(&self, tokens: &[Token]) -> Option<Value> {
        let filtered: Vec<&Token> = tokens
            .iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
            .collect();
        if filtered.is_empty() {
            return None;
        }
        let slash_pos = filtered.iter().position(|t| **t == Token::Delim('/'));
        if let Some(pos) = slash_pos {
            let name_tokens: Vec<Token> = filtered[..pos].iter().map(|t| (*t).clone()).collect();
            let type_tokens: Vec<Token> = filtered[pos + 1..].iter().map(|t| (*t).clone()).collect();
            let name_val = self.try_parse_container_name(&name_tokens)?;
            let type_val = self.try_parse_container_type(&type_tokens)?;
            Some(Value::List(vec![name_val, type_val]))
        } else {
            if let Some(type_val) = self.try_parse_container_type(tokens) {
                Some(Value::List(vec![Value::Keyword("none".to_string()), type_val]))
            } else if let Some(name_val) = self.try_parse_container_name(tokens) {
                Some(Value::List(vec![name_val, Value::ContainerType(crate::values::ContainerType::Normal)]))
            } else {
                None
            }
        }
    }

    /// Parses a `transition*` or `animation*` property into normalized longhand declarations.
    ///
    /// Shorthands are expanded so that the computed-style layer only ever sees the
    /// six `transition-*` and eight `animation-*` longhands, each holding a comma
    /// separated [`Value::List`].
    fn parse_transition_animation_property(
        &self,
        prop: &str,
        tokens: &[Token],
        important: bool,
    ) -> Vec<Declaration> {
        use crate::values::TimingFunction;

        let groups = split_commas(tokens);
        let is_animation = prop.starts_with("animation");

        let mut names: Vec<Value> = Vec::new();
        let mut durations: Vec<Value> = Vec::new();
        let mut timings: Vec<Value> = Vec::new();
        let mut delays: Vec<Value> = Vec::new();
        let mut iterations: Vec<Value> = Vec::new();
        let mut directions: Vec<Value> = Vec::new();
        let mut fills: Vec<Value> = Vec::new();
        let mut states: Vec<Value> = Vec::new();

        let longhand_prop = if is_animation { "animation-name" } else { "transition-property" };

        for group in &groups {
            let mut name: Option<Value> = None;
            let mut duration: Option<Value> = None;
            let mut timing: Option<Value> = None;
            let mut delay: Option<Value> = None;
            let mut iteration: Option<Value> = None;
            let mut direction: Option<Value> = None;
            let mut fill: Option<Value> = None;
            let mut state: Option<Value> = None;
            let mut time_count = 0;

            for tok in group {
                match tok {
                    Token::Dimension { value, unit } if unit.eq_ignore_ascii_case("s") || unit.eq_ignore_ascii_case("ms") => {
                        let ms = if unit.eq_ignore_ascii_case("s") { value * 1000.0 } else { *value };
                        if time_count == 0 {
                            duration = Some(Value::Time(ms));
                        } else {
                            delay = Some(Value::Time(ms));
                        }
                        time_count += 1;
                    }
                    Token::Number(n) if *n == 0.0 && (time_count < 2) => {
                        if time_count == 0 {
                            duration = Some(Value::Time(0.0));
                        } else {
                            delay = Some(Value::Time(0.0));
                        }
                        time_count += 1;
                    }
                    Token::Number(n) if is_animation => {
                        iteration = Some(Value::Number(*n));
                    }
                    Token::Ident(word) => {
                        let lower = word.to_ascii_lowercase();
                        if let Some(tf) = crate::values::parse_timing_function(&lower) {
                            timing = Some(Value::TimingFunction(tf));
                        } else if is_animation {
                            match lower.as_str() {
                                "infinite" => iteration = Some(Value::Keyword("infinite".to_string())),
                                "normal" | "reverse" | "alternate" | "alternate-reverse" => {
                                    direction = Some(Value::Keyword(lower.clone()))
                                }
                                "forwards" | "backwards" | "both" | "none" => {
                                    fill = Some(Value::Keyword(lower.clone()))
                                }
                                "running" | "paused" => state = Some(Value::Keyword(lower.clone())),
                                _ => name = Some(Value::Keyword(lower.clone())),
                            }
                        } else if lower == "all" || lower == "none" {
                            name = Some(Value::Keyword(lower.clone()));
                        } else {
                            name = Some(Value::Keyword(lower.clone()));
                        }
                    }
                    _ => {}
                }
            }

            // Longhand-only properties carry a single value per group.
            match prop {
                "transition-property" | "animation-name" => {
                    names.push(name.unwrap_or(Value::Keyword("all".to_string())));
                    continue;
                }
                "transition-duration" | "transition-delay" | "animation-duration" | "animation-delay" => {
                    let v = duration.or(delay).unwrap_or(Value::Time(0.0));
                    if prop.ends_with("delay") {
                        delays.push(v);
                    } else {
                        durations.push(v);
                    }
                    continue;
                }
                "transition-timing-function" | "animation-timing-function" => {
                    timings.push(timing.unwrap_or(Value::TimingFunction(TimingFunction::Ease)));
                    continue;
                }
                "animation-iteration-count" => {
                    iterations.push(iteration.unwrap_or(Value::Number(1.0)));
                    continue;
                }
                "animation-direction" => {
                    directions.push(direction.unwrap_or(Value::Keyword("normal".to_string())));
                    continue;
                }
                "animation-fill-mode" => {
                    fills.push(fill.unwrap_or(Value::Keyword("none".to_string())));
                    continue;
                }
                "animation-play-state" => {
                    states.push(state.unwrap_or(Value::Keyword("running".to_string())));
                    continue;
                }
                _ => {}
            }

            // Shorthand: fill defaults for anything omitted.
            names.push(name.unwrap_or_else(|| {
                if is_animation {
                    Value::Keyword("none".to_string())
                } else {
                    Value::Keyword("all".to_string())
                }
            }));
            durations.push(duration.unwrap_or(Value::Time(0.0)));
            timings.push(timing.unwrap_or(Value::TimingFunction(TimingFunction::Ease)));
            delays.push(delay.unwrap_or(Value::Time(0.0)));
            if is_animation {
                iterations.push(iteration.unwrap_or(Value::Number(1.0)));
                directions.push(direction.unwrap_or(Value::Keyword("normal".to_string())));
                fills.push(fill.unwrap_or(Value::Keyword("none".to_string())));
                states.push(state.unwrap_or(Value::Keyword("running".to_string())));
            }
        }

        let mut decls = Vec::new();
        let single = |v: &[Value]| -> Value {
            if v.len() == 1 { v[0].clone() } else { Value::List(v.to_vec()) }
        };
        if !names.is_empty() {
            decls.push(Declaration::new(longhand_prop, single(&names), important));
            decls.push(Declaration::new(
                if is_animation { "animation-duration" } else { "transition-duration" },
                single(&durations),
                important,
            ));
            decls.push(Declaration::new(
                if is_animation { "animation-timing-function" } else { "transition-timing-function" },
                single(&timings),
                important,
            ));
            decls.push(Declaration::new(
                if is_animation { "animation-delay" } else { "transition-delay" },
                single(&delays),
                important,
            ));
            if is_animation {
                decls.push(Declaration::new("animation-iteration-count", single(&iterations), important));
                decls.push(Declaration::new("animation-direction", single(&directions), important));
                decls.push(Declaration::new("animation-fill-mode", single(&fills), important));
                decls.push(Declaration::new("animation-play-state", single(&states), important));
            }
        }
        decls
    }

    fn skip_until(&mut self, target: Token) {
        while *self.peek() != target && *self.peek() != Token::Eof {
            self.advance();
        }
        if *self.peek() == target {
            self.advance();
        }
    }

    fn skip_until_or_stop(&mut self, target: Token, stop: &Token) {
        while *self.peek() != target && self.peek() != stop && *self.peek() != Token::Eof {
            self.advance();
        }
        if *self.peek() == target {
            self.advance();
        }
    }

    fn skip_unknown_rule_or_block(&mut self) {
        let mut depth = 0;
        loop {
            match self.peek() {
                Token::OpenCurly => {
                    depth += 1;
                    self.advance();
                }
                Token::CloseCurly => {
                    if depth > 0 {
                        self.advance();
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        // Belongs to the outer block; do not consume
                        break;
                    }
                }
                Token::Semicolon if depth == 0 => {
                    self.advance();
                    break;
                }
                Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn token_to_string(&self, tok: &Token) -> String {
        match tok {
            Token::Ident(s) => s.clone(),
            Token::String(s) => format!("\"{s}\""),
            Token::Hash(h) => format!("#{h}"),
            Token::Number(n) => n.to_string(),
            Token::Percentage(p) => format!("{p}%"),
            Token::Dimension { value, unit } => format!("{value}{unit}"),
            Token::Colon => ":".to_string(),
            Token::Semicolon => ";".to_string(),
            Token::Comma => ",".to_string(),
            Token::OpenParen => "(".to_string(),
            Token::CloseParen => ")".to_string(),
            Token::Whitespace => " ".to_string(),
            Token::Delim(c) => c.to_string(),
            _ => String::new(),
        }
    }
}

/// Parses the inner content of `linear-gradient(...)`.
fn parse_linear_gradient_inner_repeating(inner: &str, repeating: bool) -> crate::values::Gradient {
    use crate::values::{ColorStop, Gradient};
    let mut angle_deg = 180.0f32; // default: to bottom
    let mut stops: Vec<ColorStop> = Vec::new();

    // Split on commas at depth 0 respecting nested parens
    let parts = split_gradient_parts(inner);

    let mut first = true;
    for part in &parts {
        let part = part.trim();
        if first {
            first = false;
            if part.to_ascii_lowercase().starts_with("to ") {
                let dir = part[3..].trim().to_ascii_lowercase();
                angle_deg = match dir.as_str() {
                    "top" => 0.0,
                    "right" => 90.0,
                    "bottom" => 180.0,
                    "left" => 270.0,
                    "top right" | "right top" => 45.0,
                    "bottom right" | "right bottom" => 135.0,
                    "bottom left" | "left bottom" => 225.0,
                    "top left" | "left top" => 315.0,
                    _ => 180.0,
                };
                continue;
            } else if let Some(deg) = parse_angle_deg(part) {
                angle_deg = deg;
                continue;
            }
            // Fall through: treat as a color stop
        }
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Linear { angle_deg, stops, repeating }
}

/// Parses the inner content of `radial-gradient(...)`.
fn parse_radial_gradient_inner_repeating(inner: &str, repeating: bool) -> crate::values::Gradient {
    use crate::values::{ColorStop, Gradient};
    let parts = split_gradient_parts(inner);
    let mut stops: Vec<ColorStop> = Vec::new();
    for part in &parts {
        let part = part.trim();
        // Skip shape/size/position args
        if part.starts_with("ellipse") || part.starts_with("circle") ||
           part.starts_with("closest") || part.starts_with("farthest") ||
           part.starts_with("at ") { continue; }
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Radial { stops, repeating }
}

/// Parses `conic-gradient([from <angle>] [at <position>], <stops>)`.
fn parse_conic_gradient_inner(inner: &str, repeating: bool) -> crate::values::Gradient {
    use crate::values::{ColorStop, Gradient};
    let parts = split_gradient_parts(inner);
    let mut stops: Vec<ColorStop> = Vec::new();
    let mut angle_deg = 0.0f32;
    let mut first = true;
    for part in &parts {
        let part = part.trim();
        if first && (part.to_ascii_lowercase().starts_with("from ")
            || part.to_ascii_lowercase().starts_with("at ")
            || part.contains(" from "))
        {
            first = false;
            let lower = part.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("from ") {
                let angle_tok = rest.split_whitespace().next().unwrap_or("0deg");
                angle_deg = parse_angle_deg(angle_tok).unwrap_or(0.0);
            }
            continue;
        }
        first = false;
        if let Some(stop) = parse_color_stop(part) {
            stops.push(stop);
        }
    }
    Gradient::Conic { angle_deg, stops, repeating }
}

fn split_gradient_parts(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => { depth += 1; current.push(ch); }
            ')' => { depth -= 1; current.push(ch); }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() { parts.push(current.trim().to_string()); }
    parts
}

fn parse_angle_deg(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "0" {
        return Some(0.0);
    }
    if let Some(deg_str) = s.strip_suffix("deg") {
        return deg_str.trim().parse().ok();
    }
    if let Some(rad_str) = s.strip_suffix("rad") {
        return rad_str.trim().parse::<f32>().ok().map(|r| r.to_degrees());
    }
    if let Some(turn_str) = s.strip_suffix("turn") {
        return turn_str.trim().parse::<f32>().ok().map(|t| t * 360.0);
    }
    if let Some(grad_str) = s.strip_suffix("grad") {
        return grad_str.trim().parse::<f32>().ok().map(|g| g * 0.9);
    }
    None
}

/// Returns true for the `transition*` / `animation*` property family (including vendor prefixes).
pub fn is_transition_animation_property(prop: &str) -> bool {
    let p = prop
        .trim_start_matches("-webkit-")
        .trim_start_matches("-moz-")
        .trim_start_matches("-ms-")
        .trim_start_matches("-o-");
    p == "transition"
        || p == "animation"
        || p.starts_with("transition-")
        || p.starts_with("animation-")
}

/// Splits a token slice on top-level commas, respecting nested parentheses.
fn split_commas(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for tok in tokens {
        match tok {
            Token::OpenParen => {
                depth += 1;
                current.push(tok.clone());
            }
            Token::CloseParen => {
                depth -= 1;
                current.push(tok.clone());
            }
            Token::Comma if depth == 0 => {
                groups.push(std::mem::take(&mut current));
            }
            Token::Whitespace | Token::Comment(_) => {}
            _ => current.push(tok.clone()),
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    if groups.is_empty() {
        groups.push(Vec::new());
    }
    groups
}

/// Parses a single gradient color stop such as `red`, `red 25%`, or `red 10% 40%`.
fn parse_color_stop(s: &str) -> Option<crate::values::ColorStop> {
    use crate::values::ColorStop;
    let s = s.trim();
    if s.is_empty() { return None; }

    // Find the trailing run of `<percentage>` / `<length>` position tokens.
    let mut position_parts: Vec<&str> = Vec::new();
    let mut color_end = s.len();
    let mut scan_end = s.len();
    let mut split_pos = 0usize;
    for (idx, ch) in s.char_indices().rev() {
        if ch.is_whitespace() {
            let token = s[idx..scan_end].trim();
            if !token.is_empty() && is_gradient_position(token) {
                position_parts.push(token);
                color_end = idx;
                split_pos = idx;
                scan_end = idx;
            } else {
                break;
            }
        }
    }
    let _ = split_pos;
    position_parts.reverse();

    let color_part = s[..color_end].trim();
    let is_current_color = color_part.eq_ignore_ascii_case("currentcolor");
    let color = if is_current_color {
        mango_core::Color::BLACK
    } else {
        Value::parse_color(color_part)?
    };
    let to_pos = |p: &str| -> Option<f32> {
        if let Some(pct_str) = p.strip_suffix('%') {
            pct_str.trim().parse::<f32>().ok().map(|v| v / 100.0)
        } else if let Some(px_str) = p.strip_suffix("px") {
            // Pixel stops are approximated against a nominal 1000px gradient box.
            px_str.trim().parse::<f32>().ok().map(|v| v / 1000.0)
        } else {
            None
        }
    };
    let position = position_parts.first().copied().and_then(to_pos);
    let end_position = position_parts.get(1).copied().and_then(to_pos);
    Some(ColorStop { color, position, end_position, is_current_color })
}

/// Returns true when a token is a gradient stop position (`50%`, `12px`, `0`).
fn is_gradient_position(token: &str) -> bool {
    let t = token.trim();
    if t == "0" {
        return true;
    }
    if let Some(pct) = t.strip_suffix('%') {
        return pct.trim().parse::<f32>().is_ok();
    }
    if let Some(px) = t.strip_suffix("px") {
        return px.trim().parse::<f32>().is_ok();
    }
    false
}

/// Convenience function: parses a CSS stylesheet string.
pub fn parse_stylesheet(css: &str) -> Stylesheet {
    CssParser::parse_stylesheet(css)
}

/// Convenience function: parses an inline CSS declaration list.
pub fn parse_declaration_list(css: &str) -> Vec<Declaration> {
    CssParser::parse_declaration_list(css)
}

/// Convenience function: parses a CSS selector list string.
pub fn parse_selectors(css: &str) -> Option<SelectorList> {
    CssParser::parse_selector_list_str(css)
}


#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Color;

    #[test]
    fn test_parse_simple_stylesheet() {
        let css = r#"
            body {
                margin: 0px;
                background-color: #ffffff;
                color: black;
            }
            h1, h2 {
                font-weight: bold;
                font-size: 24px;
            }
        "#;

        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);

        if let Rule::Style(r1) = &sheet.rules[0] {
            assert_eq!(r1.selectors.selectors.len(), 1);
            assert!(r1.declarations.iter().any(|d| d.name == "background-color"
                && d.value == Value::Color(Color::WHITE)));
        } else {
            panic!("expected StyleRule");
        }

        if let Rule::Style(r2) = &sheet.rules[1] {
            assert_eq!(r2.selectors.selectors.len(), 2); // h1 and h2
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_important_declaration() {
        let css = "p { color: red !important; }";
        let sheet = parse_stylesheet(css);
        if let Rule::Style(r) = &sheet.rules[0] {
            assert_eq!(r.declarations.len(), 1);
            assert!(r.declarations[0].important);
            assert_eq!(r.declarations[0].value, Value::Color(Color::RED));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_viewport_units() {
        let css = "body { width: 60vw; margin: 15vh auto; }";
        let sheet = parse_stylesheet(css);
        if let Rule::Style(r) = &sheet.rules[0] {
            let width_decl = r.declarations.iter().find(|d| d.name == "width").unwrap();
            assert_eq!(width_decl.value, Value::Length(Length::Vw(60.0)));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_import_rules() {
        let css = r#"
            @import "base.css";
            @import url("theme.css");
            @import url('layout.css');
            @import url(typography.css);
            body { color: black; }
        "#;
        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 5);
        assert_eq!(sheet.rules[0], Rule::Import("base.css".to_string()));
        assert_eq!(sheet.rules[1], Rule::Import("theme.css".to_string()));
        assert_eq!(sheet.rules[2], Rule::Import("layout.css".to_string()));
        assert_eq!(sheet.rules[3], Rule::Import("typography.css".to_string()));
    }

    #[test]
    fn test_parse_flex_declarations() {
        let css = r#"
            .nav {
                display: flex;
                flex-direction: row;
                justify-content: space-between;
                align-items: center;
                gap: 16px;
            }
            .item {
                flex: 1;
                order: 2;
                z-index: 10;
            }
        "#;
        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);
        if let Rule::Style(r1) = &sheet.rules[0] {
            assert!(r1.declarations.iter().any(|d| d.name == "display" && d.value == Value::Display(Display::Flex)));
            assert!(r1.declarations.iter().any(|d| d.name == "flex-direction" && d.value == Value::FlexDirection(FlexDirection::Row)));
            assert!(r1.declarations.iter().any(|d| d.name == "justify-content" && d.value == Value::JustifyContent(JustifyContent::SpaceBetween)));
            assert!(r1.declarations.iter().any(|d| d.name == "row-gap" && d.value == Value::Length(Length::Px(16.0))));
            assert!(r1.declarations.iter().any(|d| d.name == "column-gap" && d.value == Value::Length(Length::Px(16.0))));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_media_rule_with_unsupported_tokens_does_not_loop() {
        let css = r#"
            @media (min-width: 600px) {
                @supports (display: grid) {
                    .foo { display: grid; }
                }
                .bar { color: blue; }
                :::invalid_selector { margin: 10px; }
                .baz { color: red; }
            }
            body { background-color: white; }
        "#;
        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);
        if let Rule::Media(m) = &sheet.rules[0] {
            assert!(m.rules.iter().any(|r| r.declarations.iter().any(|d| d.name == "color" && d.value == Value::Color(Color::BLUE))));
            assert!(m.rules.iter().any(|r| r.declarations.iter().any(|d| d.name == "color" && d.value == Value::Color(Color::RED))));
        } else {
            panic!("expected MediaRule");
        }
    }

    #[test]
    fn test_parse_grid_properties() {
        let css = r#"
            .container {
                display: grid;
                grid-template-columns: 200px 1fr;
                grid-template-rows: repeat(2, 50px);
            }
            .sidebar {
                grid-column: 1 / 2;
                grid-row: span 2;
            }
        "#;
        let sheet = parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 2);

        if let Rule::Style(r1) = &sheet.rules[0] {
            let cols = r1.declarations.iter().find(|d| d.name == "grid-template-columns").unwrap();
            assert_eq!(
                cols.value,
                Value::GridTrackList(vec![
                    GridTrackSize::Length(Length::Px(200.0)),
                    GridTrackSize::Fr(1.0),
                ])
            );
            let rows = r1.declarations.iter().find(|d| d.name == "grid-template-rows").unwrap();
            assert_eq!(
                rows.value,
                Value::GridTrackList(vec![
                    GridTrackSize::Length(Length::Px(50.0)),
                    GridTrackSize::Length(Length::Px(50.0)),
                ])
            );
        } else {
            panic!("expected StyleRule");
        }

        if let Rule::Style(r2) = &sheet.rules[1] {
            let col_start = r2.declarations.iter().find(|d| d.name == "grid-column-start").unwrap();
            let col_end = r2.declarations.iter().find(|d| d.name == "grid-column-end").unwrap();
            let row_start = r2.declarations.iter().find(|d| d.name == "grid-row-start").unwrap();

            assert_eq!(col_start.value, Value::GridPlacement(GridPlacement::Line(1)));
            assert_eq!(col_end.value, Value::GridPlacement(GridPlacement::Line(2)));
            assert_eq!(row_start.value, Value::GridPlacement(GridPlacement::Span(2)));
        } else {
            panic!("expected StyleRule");
        }

        let css_area = r#"
            .layout {
                grid-template: 50px 1fr / 200px 1fr;
                grid-template-areas: "header header" "sidebar content";
            }
            .header {
                grid-area: header;
            }
        "#;
        let sheet_area = parse_stylesheet(css_area);
        assert_eq!(sheet_area.rules.len(), 2);
        if let Rule::Style(r) = &sheet_area.rules[0] {
            let cols = r.declarations.iter().find(|d| d.name == "grid-template-columns").unwrap();
            let rows = r.declarations.iter().find(|d| d.name == "grid-template-rows").unwrap();
            assert_eq!(
                cols.value,
                Value::GridTrackList(vec![
                    GridTrackSize::Length(Length::Px(200.0)),
                    GridTrackSize::Fr(1.0),
                ])
            );
            assert_eq!(
                rows.value,
                Value::GridTrackList(vec![
                    GridTrackSize::Length(Length::Px(50.0)),
                    GridTrackSize::Fr(1.0),
                ])
            );
            let areas = r.declarations.iter().find(|d| d.name == "grid-template-areas").unwrap();
            assert_eq!(
                areas.value,
                Value::List(vec![
                    Value::String("header header".to_string()),
                    Value::String("sidebar content".to_string()),
                ])
            );
        } else {
            panic!("expected StyleRule");
        }
        if let Rule::Style(r) = &sheet_area.rules[1] {
            let row_start = r.declarations.iter().find(|d| d.name == "grid-row-start").unwrap();
            let col_start = r.declarations.iter().find(|d| d.name == "grid-column-start").unwrap();
            assert_eq!(row_start.value, Value::GridPlacement(GridPlacement::Area("header".to_string())));
            assert_eq!(col_start.value, Value::GridPlacement(GridPlacement::Area("header".to_string())));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_font_face_rule() {
        let css = r#"
            @font-face {
                font-family: 'Open Sans';
                src: url('fonts/OpenSans-Bold.woff2') format('woff2');
                font-weight: bold;
                font-style: italic;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        match &sheet.rules[0] {
            Rule::FontFace(ff) => {
                assert_eq!(ff.font_family, "Open Sans");
                assert_eq!(ff.src_url, "fonts/OpenSans-Bold.woff2");
                assert_eq!(ff.font_weight, FontWeight::Bold);
                assert_eq!(ff.font_style, FontStyle::Italic);
            }
            _ => panic!("expected FontFace rule"),
        }
    }

    #[test]
    fn test_parse_font_face_rule_with_multiple_src_prefers_woff2() {
        let css = r#"
            @font-face {
                font-family: WikipediaLocal;
                src: url(wiki.ttf), url(wiki.woff2) format('woff2'), url(wiki.woff);
                font-weight: 700;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        match &sheet.rules[0] {
            Rule::FontFace(ff) => {
                assert_eq!(ff.font_family, "WikipediaLocal");
                assert_eq!(ff.src_url, "wiki.woff2");
                assert_eq!(ff.font_weight, FontWeight::Bold);
                assert_eq!(ff.font_style, FontStyle::Normal);
            }
            _ => panic!("expected FontFace rule"),
        }
    }

    #[test]
    fn test_parse_calc_function() {
        let css = r#"
            .container {
                width: calc(100% - 20px);
                max-width: calc(50vw + 10px);
                margin-left: calc(2rem - 4px);
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            let width = sr.declarations.iter().find(|d| d.name == "width").unwrap();
            match &width.value {
                Value::Length(Length::Calc(c)) => {
                    assert_eq!(c.percent, 100.0);
                    assert_eq!(c.px, -20.0);
                }
                other => panic!("expected calc length, got {:?}", other),
            }

            let max_w = sr.declarations.iter().find(|d| d.name == "max-width").unwrap();
            match &max_w.value {
                Value::Length(Length::Calc(c)) => {
                    assert_eq!(c.vw, 50.0);
                    assert_eq!(c.px, 10.0);
                }
                other => panic!("expected calc length, got {:?}", other),
            }

            let ml = sr.declarations.iter().find(|d| d.name == "margin-left").unwrap();
            match &ml.value {
                Value::Length(Length::Calc(c)) => {
                    assert_eq!(c.rem, 2.0);
                    assert_eq!(c.px, -4.0);
                }
                other => panic!("expected calc length, got {:?}", other),
            }
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_parse_box_shadow() {
        let css = r#"
            .card {
                box-shadow: 2px 4px 8px 1px rgba(0, 0, 0, 0.2);
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            let shadow = sr.declarations.iter().find(|d| d.name == "box-shadow").unwrap();
            match &shadow.value {
                Value::BoxShadow(bs) => {
                    assert_eq!(bs.offset_x, 2.0);
                    assert_eq!(bs.offset_y, 4.0);
                    assert_eq!(bs.blur_radius, 8.0);
                    assert_eq!(bs.spread_radius, 1.0);
                    assert!(!bs.inset);
                }
                other => panic!("expected BoxShadow, got {:?}", other),
            }
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_nested_supports_in_media() {
        let css = r#"
            @media screen {
                .vector-pinned-container { display: none; }
                @supports (display:grid) {
                    .vector-pinned-container { display: block; }
                }
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Media(mr) = &sheet.rules[0] {
            assert_eq!(mr.query, "screen");
            assert_eq!(mr.rules.len(), 2);
            let display_decls: Vec<&Value> = mr.rules.iter()
                .map(|r| &r.declarations.iter().find(|d| d.name == "display").unwrap().value)
                .collect();
            assert_eq!(display_decls, vec![&Value::Keyword("none".to_string()), &Value::Display(Display::Block)]);
        } else {
            panic!("expected MediaRule");
        }
    }

    #[test]
    fn test_font_face_formats_and_display() {
        let css = r#"
            @font-face {
                font-family: 'TestFont';
                src: url('font.otf') format('opentype'),
                     url('font.woff') format('woff'),
                     url('font.woff2') format('woff2'),
                     url('font.ttf') format('truetype');
                font-display: swap;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        match &sheet.rules[0] {
            Rule::FontFace(ff) => {
                assert_eq!(ff.font_family, "TestFont");
                assert_eq!(ff.src_url, "font.woff2", "Must prefer woff2 over woff, ttf, otf");
                assert_eq!(ff.font_display, crate::values::FontDisplay::Swap);
            }
            _ => panic!("expected FontFace rule"),
        }
    }

    #[test]
    fn test_font_variation_and_feature_settings() {
        let css = r#"
            .typography {
                font-variation-settings: "wght" 750, "wdth" 85.5;
                font-feature-settings: "liga" 1, "smcp" on, "swsh" 2, "dlig" off;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            let fvs = sr.declarations.iter().find(|d| d.name == "font-variation-settings").unwrap();
            match &fvs.value {
                Value::FontVariationSettings(crate::values::FontVariationSettings::Settings(s)) => {
                    assert_eq!(s.len(), 2);
                    assert_eq!(s[0], ("wght".to_string(), 750.0));
                    assert_eq!(s[1], ("wdth".to_string(), 85.5));
                }
                _ => panic!("expected FontVariationSettings::Settings"),
            }

            let ffs = sr.declarations.iter().find(|d| d.name == "font-feature-settings").unwrap();
            match &ffs.value {
                Value::FontFeatureSettings(crate::values::FontFeatureSettings::Features(f)) => {
                    assert_eq!(f.len(), 4);
                    assert_eq!(f[0], ("liga".to_string(), 1));
                    assert_eq!(f[1], ("smcp".to_string(), 1));
                    assert_eq!(f[2], ("swsh".to_string(), 2));
                    assert_eq!(f[3], ("dlig".to_string(), 0));
                }
                _ => panic!("expected FontFeatureSettings::Features"),
            }
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_wrapping_and_hyphenation_properties() {
        let css = r#"
            .text-rules {
                word-break: break-all;
                overflow-wrap: anywhere;
                hyphens: auto;
                line-clamp: 3;
                -webkit-line-clamp: 2;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            let wb = sr.declarations.iter().find(|d| d.name == "word-break").unwrap();
            assert_eq!(wb.value, Value::WordBreak(crate::values::WordBreak::BreakAll));

            let ow = sr.declarations.iter().find(|d| d.name == "overflow-wrap").unwrap();
            assert_eq!(ow.value, Value::OverflowWrap(crate::values::OverflowWrap::Anywhere));

            let hyp = sr.declarations.iter().find(|d| d.name == "hyphens").unwrap();
            assert_eq!(hyp.value, Value::Hyphens(crate::values::Hyphens::Auto));

            let lc = sr.declarations.iter().find(|d| d.name == "line-clamp").unwrap();
            assert_eq!(lc.value, Value::LineClamp(crate::values::LineClamp::Lines(3)));

            let wlc = sr.declarations.iter().find(|d| d.name == "-webkit-line-clamp").unwrap();
            assert_eq!(wlc.value, Value::LineClamp(crate::values::LineClamp::Lines(2)));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_text_decorations_and_emphasis() {
        let css = r#"
            .decorations {
                text-underline-offset: 4px;
                text-decoration-thickness: 2px;
                text-emphasis: filled circle #ff0000;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            let tuo = sr.declarations.iter().find(|d| d.name == "text-underline-offset").unwrap();
            assert_eq!(tuo.value, Value::Length(crate::values::Length::Px(4.0)));

            let tdt = sr.declarations.iter().find(|d| d.name == "text-decoration-thickness").unwrap();
            assert_eq!(tdt.value, Value::TextDecorationThickness(crate::values::TextDecorationThickness::Length(crate::values::Length::Px(2.0))));

            let tes = sr.declarations.iter().find(|d| d.name == "text-emphasis-style").unwrap();
            assert_eq!(tes.value, Value::TextEmphasisStyle(crate::values::TextEmphasisStyle::FilledCircle));

            let tec = sr.declarations.iter().find(|d| d.name == "text-emphasis-color").unwrap();
            assert_eq!(tec.value, Value::Color(mango_core::Color::rgb(255, 0, 0)));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_important_with_trailing_tokens_dropped() {
        let css = r#"
            div {
                color: red !important blue;
                background-color: green !important;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            assert!(sr.declarations.iter().all(|d| d.name != "color"), "color with tokens after !important must be dropped");
            assert!(sr.declarations.iter().any(|d| d.name == "background-color" && d.important));
        } else {
            panic!("expected StyleRule");
        }
    }

    #[test]
    fn test_keyframes_invalid_selector_dropped() {
        let css = r#"
            @keyframes bounce {
                from { top: 0px; }
                unknown { top: 50px; }
                to { top: 100px; }
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Keyframes(kf) = &sheet.rules[0] {
            assert_eq!(kf.keyframes.len(), 2, "Only from and to should remain, invalid selector must be dropped");
        } else {
            panic!("expected Keyframes");
        }
    }

    #[test]
    fn test_attribute_selector_invalid_op_dropped() {
        let css = r#"
            [a~b] { color: red; }
            [lang|="en"] { color: blue; }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        // [a~b] is invalid and must not produce a rule
        assert_eq!(sheet.rules.len(), 1);
    }

    #[test]
    fn test_unknown_unit_declaration_dropped() {
        let css = r#"
            div {
                width: 100foo;
                height: 200px;
                margin: 10px 20bar 30px 40px;
            }
        "#;
        let sheet = CssParser::parse_stylesheet(css);
        assert_eq!(sheet.rules.len(), 1);
        if let Rule::Style(sr) = &sheet.rules[0] {
            assert_eq!(sr.declarations.len(), 1, "Only height: 200px should be kept");
            assert_eq!(sr.declarations[0].name, "height");
        } else {
            panic!("expected StyleRule");
        }
    }
}

