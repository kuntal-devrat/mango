use crate::parser::types::Stylesheet;
use crate::properties::Declaration;
use crate::selectors::SelectorList;
use crate::tokenizer::{CssTokenizer, Token};
use crate::values::{Length, Value};

/// CSS parser that walks a stream of tokens.
pub struct CssParser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) cursor: usize,
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

    pub(crate) fn peek(&self) -> &Token {
        self.tokens.get(self.cursor).unwrap_or(&Token::Eof)
    }

    pub(crate) fn advance(&mut self) -> Token {
        if self.cursor < self.tokens.len() {
            let tok = self.tokens[self.cursor].clone();
            self.cursor += 1;
            tok
        } else {
            Token::Eof
        }
    }

    pub(crate) fn skip_whitespace_and_comments(&mut self) {
        while let Token::Whitespace | Token::Comment(_) = self.peek() {
            self.advance();
        }
    }

    pub(crate) fn parse_rules(&mut self) -> Stylesheet {
        let rules = self.parse_rule_list(false);
        Stylesheet::new(rules)
    }

    pub(crate) fn skip_until(&mut self, target: Token) {
        while *self.peek() != target && *self.peek() != Token::Eof {
            self.advance();
        }
        if *self.peek() == target {
            self.advance();
        }
    }

    pub(crate) fn skip_until_or_stop(&mut self, target: Token, stop: &Token) {
        while *self.peek() != target && self.peek() != stop && *self.peek() != Token::Eof {
            self.advance();
        }
        if *self.peek() == target {
            self.advance();
        }
    }

    pub(crate) fn skip_unknown_rule_or_block(&mut self) {
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

    pub(crate) fn token_to_string(&self, tok: &Token) -> String {
        crate::parser::declarations::gradient::token_to_string(tok)
    }

    pub(crate) fn parse_value_from_tokens(&self, tokens: &[Token]) -> Option<Value> {
        crate::parser::declarations::value::parse_value_from_tokens(tokens)
    }

    pub(crate) fn parse_single_value(&self, token: &Token) -> Option<Value> {
        crate::parser::declarations::value::parse_single_value(token)
    }
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

/// Convenience function: parses a CSS math function string (`calc(...)`, `min(...)`, `max(...)`, `clamp(...)`) into a [`Length`].
pub fn parse_math_length_from_str(s: &str) -> Option<Length> {
    let tokens = CssTokenizer::tokenize(s);
    if let Some((Value::Length(len), _)) =
        crate::parser::declarations::math::parse_math_call(&tokens)
    {
        Some(len)
    } else {
        None
    }
}
