use super::super::stream::CssParser;
use crate::parser::types::{Rule, StyleRule};
use crate::tokenizer::Token;

impl CssParser {
    pub(crate) fn parse_nested_style_rules_into(
        &mut self,
        out_rules: &mut Vec<StyleRule>,
        depth: usize,
    ) {
        if depth >= 32 {
            self.skip_unknown_rule_or_block();
            return;
        }
        while *self.peek() != Token::CloseCurly && *self.peek() != Token::Eof {
            self.skip_whitespace_and_comments();
            if *self.peek() == Token::CloseCurly || *self.peek() == Token::Eof {
                break;
            }
            if let Token::AtKeyword(inner_kw) = self.peek().clone() {
                self.advance();
                if inner_kw.eq_ignore_ascii_case("supports") {
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
                        let mut nested_support_rules = Vec::new();
                        self.parse_nested_style_rules_into(&mut nested_support_rules, depth + 1);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                        if self.eval_supports_condition(&condition_tokens) {
                            out_rules.extend(nested_support_rules);
                        }
                    }
                } else if inner_kw.eq_ignore_ascii_case("layer")
                    || inner_kw.eq_ignore_ascii_case("scope")
                {
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
                                self.advance();
                            }
                        }
                    }
                    if has_block {
                        self.parse_nested_style_rules_into(out_rules, depth + 1);
                        if *self.peek() == Token::CloseCurly {
                            self.advance();
                        }
                    }
                } else {
                    self.skip_unknown_rule_or_block();
                }
                continue;
            }
            let mut style_rules = Vec::new();
            if self.parse_style_rule_and_nested_into(&mut style_rules) {
                for r in style_rules {
                    if let Rule::Style(sr) = r {
                        out_rules.push(sr);
                    }
                }
            } else {
                self.skip_unknown_rule_or_block();
            }
        }
    }

    pub(crate) fn parse_style_rule_and_nested_into(&mut self, out_rules: &mut Vec<Rule>) -> bool {
        let selectors = match self.parse_selector_list() {
            Some(s) => s,
            None => return false,
        };
        self.skip_whitespace_and_comments();

        if self.advance() != Token::OpenCurly {
            return false;
        }

        let mut nested_rules = Vec::new();
        let declarations = self.parse_declarations_with_nesting_until(
            Token::CloseCurly,
            Some(&selectors),
            Some(&mut nested_rules),
        );

        if !declarations.is_empty() || nested_rules.is_empty() {
            out_rules.push(Rule::Style(StyleRule {
                selectors,
                declarations,
            }));
        }
        out_rules.extend(nested_rules);
        true
    }

    #[allow(dead_code)]
    pub(crate) fn parse_style_rule(&mut self) -> Option<StyleRule> {
        let mut rules = Vec::new();
        if self.parse_style_rule_and_nested_into(&mut rules) {
            for r in rules {
                if let Rule::Style(sr) = r {
                    return Some(sr);
                }
            }
        }
        None
    }
}
