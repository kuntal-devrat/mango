use crate::parser::stream::CssParser;
use crate::parser::types::{MediaRule, Rule, StyleRule};
use crate::properties::Declaration;
use crate::selectors::SelectorList;
use crate::tokenizer::Token;

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

            self.parse_single_property_declaration(
                prop_name,
                value_tokens,
                important,
                &mut declarations,
            );
        }

        declarations
    }
}
