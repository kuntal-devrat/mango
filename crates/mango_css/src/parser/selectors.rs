use super::stream::CssParser;
use crate::selectors::{
    AttributeOperator, Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector,
};
use crate::tokenizer::Token;

impl CssParser {
    pub(crate) fn parse_selector_list(&mut self) -> Option<SelectorList> {
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

    pub(crate) fn parse_complex_selector(&mut self) -> Option<ComplexSelector> {
        let (head, initial_comb) = match self.peek() {
            Token::Delim('>') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Nesting]),
                    Some(Combinator::Child),
                )
            }
            Token::Delim('+') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Nesting]),
                    Some(Combinator::NextSibling),
                )
            }
            Token::Delim('~') => {
                self.advance();
                self.skip_whitespace_and_comments();
                (
                    CompoundSelector::new(vec![SimpleSelector::Nesting]),
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
                        Token::Ident(_)
                            | Token::Hash(_)
                            | Token::Delim('.')
                            | Token::Delim('*')
                            | Token::Delim('&')
                            | Token::OpenBracket
                            | Token::Colon
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

    pub(crate) fn parse_compound_selector(&mut self) -> Option<CompoundSelector> {
        let mut simples = Vec::new();

        while let Some(simple) = self.parse_simple_selector() {
            simples.push(simple);
            // Compound selectors must not be separated by whitespace
            if matches!(
                self.peek(),
                Token::Whitespace | Token::Comma | Token::OpenCurly | Token::Eof
            ) {
                break;
            }
        }

        if simples.is_empty() {
            None
        } else {
            Some(CompoundSelector::new(simples))
        }
    }

    pub(crate) fn parse_simple_selector(&mut self) -> Option<SimpleSelector> {
        match self.peek() {
            Token::Delim('&') => {
                self.advance();
                Some(SimpleSelector::Nesting)
            }
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
                    } else if delim_char == '~'
                        || delim_char == '^'
                        || delim_char == '$'
                        || delim_char == '*'
                        || delim_char == '|'
                    {
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
                        let mut lower = name.to_ascii_lowercase();
                        // Consume optional arguments e.g. ::slotted(...)
                        if *self.peek() == Token::OpenParen {
                            self.advance();
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
                        return Some(SimpleSelector::PseudoElement(lower));
                    } else {
                        return None;
                    }
                }

                if let Token::Ident(pseudo) = self.advance() {
                    let mut lower = pseudo.to_ascii_lowercase();
                    // Legacy single-colon pseudo-elements
                    if matches!(
                        lower.as_str(),
                        "before"
                            | "after"
                            | "first-line"
                            | "first-letter"
                            | "placeholder"
                            | "selection"
                            | "marker"
                    ) {
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
}
