//! WHATWG HTML5 compliant tokenizer state machine.
//!
//! Emits a stream of [`Token`]s by processing characters one-by-one according
//! to the HTML parsing specification.

use crate::entities::decode_entities;

/// Tokens produced by the HTML tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    StartTag {
        name: String,
        attributes: Vec<(String, String)>,
        self_closing: bool,
    },
    EndTag {
        name: String,
    },
    Comment(String),
    Character(char),
    EndOfFile,
}

/// Tokenizer state machine states.
#[derive(Debug, Clone, PartialEq)]
enum State {
    Data,
    TagOpen,
    EndTagOpen,
    TagName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    MarkupDeclarationOpen,
    CommentStart,
    Comment,
    CommentEndDash,
    CommentEnd,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    BogusComment,
    RawText { tag: String },
}

/// HTML5 tokenizer.
pub struct Tokenizer<'a> {
    chars: std::str::Chars<'a>,
    current_char: Option<char>,
    state: State,
    // Buffer for building current token
    current_tag_name: String,
    current_attributes: Vec<(String, String)>,
    current_attr_name: String,
    current_attr_val: String,
    current_comment: String,
    current_doctype_name: String,
    current_doctype_public_id: Option<String>,
    current_doctype_system_id: Option<String>,
    current_doctype_force_quirks: bool,
    is_end_tag: bool,
    self_closing: bool,
    pending_chars: std::collections::VecDeque<char>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let current_char = chars.next();
        Self {
            chars,
            current_char,
            state: State::Data,
            current_tag_name: String::new(),
            current_attributes: Vec::new(),
            current_attr_name: String::new(),
            current_attr_val: String::new(),
            current_comment: String::new(),
            current_doctype_name: String::new(),
            current_doctype_public_id: None,
            current_doctype_system_id: None,
            current_doctype_force_quirks: false,
            is_end_tag: false,
            self_closing: false,
            pending_chars: std::collections::VecDeque::new(),
        }
    }

    /// Sets raw text mode for elements like `<script>` or `<style>`.
    pub fn set_raw_text_mode(&mut self, tag_name: &str) {
        self.state = State::RawText {
            tag: tag_name.to_ascii_lowercase(),
        };
    }

    fn advance(&mut self) -> Option<char> {
        let prev = self.current_char;
        self.current_char = self.chars.next();
        prev
    }

    fn peek(&self) -> Option<char> {
        self.current_char
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn consume_quoted_string(&mut self, quote: char) -> String {
        let mut s = String::new();
        while let Some(ch) = self.advance() {
            if ch == quote {
                break;
            }
            s.push(ch);
        }
        s
    }

    /// Pulls the next token from the stream.
    pub fn next_token(&mut self) -> Token {
        if let Some(ch) = self.pending_chars.pop_front() {
            return Token::Character(ch);
        }
        loop {
            match &self.state {
                State::Data => match self.advance() {
                    Some('<') => {
                        self.state = State::TagOpen;
                    }
                    Some('&') => {
                        // Gather entity
                        let mut entity = String::from('&');
                        while let Some(ch) = self.peek() {
                            if ch == ';' {
                                entity.push(self.advance().unwrap());
                                break;
                            }
                            if ch.is_alphanumeric() || ch == '#' {
                                entity.push(self.advance().unwrap());
                                if entity.len() > 32 {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        let decoded = decode_entities(&entity);
                        let mut chars = decoded.chars();
                        if let Some(first) = chars.next() {
                            for remaining in chars {
                                self.pending_chars.push_back(remaining);
                            }
                            return Token::Character(first);
                        }
                    }
                    Some(ch) => {
                        return Token::Character(ch);
                    }
                    None => {
                        return Token::EndOfFile;
                    }
                },

                State::TagOpen => match self.peek() {
                    Some('!') => {
                        self.advance();
                        self.state = State::MarkupDeclarationOpen;
                    }
                    Some('/') => {
                        self.advance();
                        self.state = State::EndTagOpen;
                    }
                    Some('?') => {
                        self.advance();
                        self.current_comment.clear();
                        self.state = State::BogusComment;
                    }
                    Some(ch) if ch.is_ascii_alphabetic() => {
                        self.current_tag_name.clear();
                        self.current_attributes.clear();
                        self.is_end_tag = false;
                        self.self_closing = false;
                        self.state = State::TagName;
                    }
                    Some('>') => {
                        self.advance();
                        self.state = State::Data;
                        return Token::Character('<');
                    }
                    Some(_) => {
                        self.state = State::Data;
                        return Token::Character('<');
                    }
                    None => {
                        return Token::Character('<');
                    }
                },

                State::EndTagOpen => match self.peek() {
                    Some(ch) if ch.is_ascii_alphabetic() => {
                        self.current_tag_name.clear();
                        self.current_attributes.clear();
                        self.is_end_tag = true;
                        self.self_closing = false;
                        self.state = State::TagName;
                    }
                    Some('>') => {
                        self.advance();
                        self.state = State::Data;
                    }
                    _ => {
                        self.current_comment.clear();
                        self.state = State::BogusComment;
                    }
                },

                State::TagName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {
                        self.state = State::BeforeAttributeName;
                    }
                    Some('/') => {
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.current_tag_name.push(ch.to_ascii_lowercase());
                    }
                    None => {
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::BeforeAttributeName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {}
                    Some('/') => {
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.current_attr_name.clear();
                        self.current_attr_val.clear();
                        self.current_attr_name.push(ch.to_ascii_lowercase());
                        self.state = State::AttributeName;
                    }
                    None => {
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AttributeName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {
                        self.state = State::AfterAttributeName;
                    }
                    Some('=') => {
                        self.state = State::BeforeAttributeValue;
                    }
                    Some('/') => {
                        self.finish_attribute();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.current_attr_name.push(ch.to_ascii_lowercase());
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AfterAttributeName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {}
                    Some('=') => {
                        self.state = State::BeforeAttributeValue;
                    }
                    Some('/') => {
                        self.finish_attribute();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.finish_attribute();
                        self.current_attr_name.clear();
                        self.current_attr_val.clear();
                        self.current_attr_name.push(ch.to_ascii_lowercase());
                        self.state = State::AttributeName;
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::BeforeAttributeValue => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {}
                    Some('"') => {
                        self.state = State::AttributeValueDoubleQuoted;
                    }
                    Some('\'') => {
                        self.state = State::AttributeValueSingleQuoted;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.current_attr_val.push(ch);
                        self.state = State::AttributeValueUnquoted;
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AttributeValueDoubleQuoted => match self.advance() {
                    Some('"') => {
                        self.state = State::AfterAttributeValueQuoted;
                    }
                    Some(ch) => {
                        self.current_attr_val.push(ch);
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AttributeValueSingleQuoted => match self.advance() {
                    Some('\'') => {
                        self.state = State::AfterAttributeValueQuoted;
                    }
                    Some(ch) => {
                        self.current_attr_val.push(ch);
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AttributeValueUnquoted => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {
                        self.finish_attribute();
                        self.state = State::BeforeAttributeName;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(ch) => {
                        self.current_attr_val.push(ch);
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::AfterAttributeValueQuoted => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {
                        self.finish_attribute();
                        self.state = State::BeforeAttributeName;
                    }
                    Some('/') => {
                        self.finish_attribute();
                        self.state = State::SelfClosingStartTag;
                    }
                    Some('>') => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(_) => {
                        self.finish_attribute();
                        self.state = State::BeforeAttributeName;
                    }
                    None => {
                        self.finish_attribute();
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::SelfClosingStartTag => match self.advance() {
                    Some('>') => {
                        self.self_closing = true;
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                    Some(_) => {
                        self.state = State::BeforeAttributeName;
                    }
                    None => {
                        self.state = State::Data;
                        return self.emit_current_tag();
                    }
                },

                State::MarkupDeclarationOpen => {
                    if self.peek() == Some('-') {
                        // Check if next two chars are "--"
                        self.advance(); // consume first '-'
                        if self.peek() == Some('-') {
                            self.advance(); // consume second '-'
                            self.current_comment.clear();
                            self.state = State::CommentStart;
                            continue;
                        }
                        self.current_comment.clear();
                        self.current_comment.push('-');
                        self.state = State::BogusComment;
                        continue;
                    }

                    // Check for DOCTYPE (case-insensitive)
                    let mut lookahead = String::new();
                    if let Some(ch) = self.peek() {
                        lookahead.push(ch);
                    }
                    let mut clone = self.chars.clone();
                    for _ in 0..6 {
                        if let Some(c) = clone.next() {
                            lookahead.push(c);
                        } else {
                            break;
                        }
                    }

                    if lookahead.eq_ignore_ascii_case("doctype") {
                        // Consume all 7 characters
                        for _ in 0..7 {
                            self.advance();
                        }
                        self.current_doctype_name.clear();
                        self.state = State::BeforeDoctypeName;
                        continue;
                    }

                    self.current_comment.clear();
                    self.state = State::BogusComment;
                }

                State::CommentStart => match self.advance() {
                    Some('-') => {
                        self.state = State::CommentEndDash;
                    }
                    Some('>') => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                    Some(ch) => {
                        self.current_comment.push(ch);
                        self.state = State::Comment;
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                },

                State::Comment => match self.advance() {
                    Some('-') => {
                        self.state = State::CommentEndDash;
                    }
                    Some(ch) => {
                        self.current_comment.push(ch);
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                },

                State::CommentEndDash => match self.advance() {
                    Some('-') => {
                        self.state = State::CommentEnd;
                    }
                    Some(ch) => {
                        self.current_comment.push('-');
                        self.current_comment.push(ch);
                        self.state = State::Comment;
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                },

                State::CommentEnd => match self.advance() {
                    Some('>') => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                    Some('-') => {
                        self.current_comment.push('-');
                    }
                    Some(ch) => {
                        self.current_comment.push_str("--");
                        self.current_comment.push(ch);
                        self.state = State::Comment;
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                },

                State::BogusComment => match self.advance() {
                    Some('>') | None => {
                        self.state = State::Data;
                        return Token::Comment(self.current_comment.clone());
                    }
                    Some(ch) => {
                        self.current_comment.push(ch);
                    }
                },

                State::BeforeDoctypeName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {}
                    Some('>') => {
                        self.state = State::Data;
                        return Token::Doctype {
                            name: None,
                            public_id: None,
                            system_id: None,
                            force_quirks: true,
                        };
                    }
                    Some(ch) => {
                        self.current_doctype_name.clear();
                        self.current_doctype_public_id = None;
                        self.current_doctype_system_id = None;
                        self.current_doctype_force_quirks = false;
                        self.current_doctype_name.push(ch.to_ascii_lowercase());
                        self.state = State::DoctypeName;
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Doctype {
                            name: None,
                            public_id: None,
                            system_id: None,
                            force_quirks: true,
                        };
                    }
                },

                State::DoctypeName => match self.advance() {
                    Some(ch) if ch.is_ascii_whitespace() => {
                        self.state = State::AfterDoctypeName;
                    }
                    Some('>') => {
                        self.state = State::Data;
                        return Token::Doctype {
                            name: Some(self.current_doctype_name.clone()),
                            public_id: self.current_doctype_public_id.take(),
                            system_id: self.current_doctype_system_id.take(),
                            force_quirks: false,
                        };
                    }
                    Some(ch) => {
                        self.current_doctype_name.push(ch.to_ascii_lowercase());
                    }
                    None => {
                        self.state = State::Data;
                        return Token::Doctype {
                            name: Some(self.current_doctype_name.clone()),
                            public_id: self.current_doctype_public_id.take(),
                            system_id: self.current_doctype_system_id.take(),
                            force_quirks: false,
                        };
                    }
                },

                State::AfterDoctypeName => {
                    self.skip_whitespace();
                    match self.peek() {
                        Some('>') => {
                            self.advance();
                            self.state = State::Data;
                            return Token::Doctype {
                                name: Some(self.current_doctype_name.clone()),
                                public_id: self.current_doctype_public_id.take(),
                                system_id: self.current_doctype_system_id.take(),
                                force_quirks: self.current_doctype_force_quirks,
                            };
                        }
                        None => {
                            self.state = State::Data;
                            return Token::Doctype {
                                name: Some(self.current_doctype_name.clone()),
                                public_id: self.current_doctype_public_id.take(),
                                system_id: self.current_doctype_system_id.take(),
                                force_quirks: true,
                            };
                        }
                        Some(_) => {
                            let mut keyword = String::new();
                            while let Some(ch) = self.peek() {
                                if ch.is_ascii_alphabetic() {
                                    keyword.push(self.advance().unwrap());
                                } else {
                                    break;
                                }
                            }
                            if keyword.eq_ignore_ascii_case("PUBLIC") {
                                self.skip_whitespace();
                                if let Some(q @ ('"' | '\'')) = self.peek() {
                                    self.advance();
                                    self.current_doctype_public_id = Some(self.consume_quoted_string(q));
                                    self.skip_whitespace();
                                    if let Some(q2 @ ('"' | '\'')) = self.peek() {
                                        self.advance();
                                        self.current_doctype_system_id = Some(self.consume_quoted_string(q2));
                                    }
                                } else {
                                    self.current_doctype_force_quirks = true;
                                }
                            } else if keyword.eq_ignore_ascii_case("SYSTEM") {
                                self.skip_whitespace();
                                if let Some(q @ ('"' | '\'')) = self.peek() {
                                    self.advance();
                                    self.current_doctype_system_id = Some(self.consume_quoted_string(q));
                                } else {
                                    self.current_doctype_force_quirks = true;
                                }
                            } else {
                                self.current_doctype_force_quirks = true;
                            }
                            // Advance past anything else until closing '>' or EOF
                            while let Some(ch) = self.peek() {
                                if ch == '>' {
                                    self.advance();
                                    break;
                                }
                                self.advance();
                            }
                            self.state = State::Data;
                            return Token::Doctype {
                                name: Some(self.current_doctype_name.clone()),
                                public_id: self.current_doctype_public_id.take(),
                                system_id: self.current_doctype_system_id.take(),
                                force_quirks: self.current_doctype_force_quirks,
                            };
                        }
                    }
                },

                State::RawText { tag } => {
                    // Check if the current position matches `</tag>`
                    if self.peek() == Some('<') {
                        let tag_clone = tag.clone();
                        // Lookahead to see if next chars are `/{tag}>`
                        let mut temp = String::new();
                        let mut match_found = false;
                        let mut chars_clone = self.chars.clone();

                        if chars_clone.next() == Some('/') {
                            temp.push('/');
                            let mut tag_buf = String::new();
                            for c in chars_clone {
                                if c.is_ascii_whitespace() || c == '>' || c == '/' {
                                    if c == '>' {
                                        temp.push('>');
                                    }
                                    break;
                                }
                                tag_buf.push(c.to_ascii_lowercase());
                                temp.push(c);
                            }
                            if tag_buf == tag_clone {
                                match_found = true;
                            }
                        }

                        if match_found {
                            self.advance(); // consume '<'
                            self.advance(); // consume '/'
                            for _ in 0..tag_clone.len() {
                                self.advance(); // consume tag name chars
                            }
                            self.current_tag_name = tag_clone;
                            self.is_end_tag = true;
                            self.state = State::BeforeAttributeName;
                            continue;
                        }
                    }

                    if crate::elements::is_escapable_raw_text_element(tag) && self.peek() == Some('&') {
                        self.advance(); // consume '&'
                        let mut entity = String::from('&');
                        while let Some(ch) = self.peek() {
                            if ch == ';' {
                                entity.push(self.advance().unwrap());
                                break;
                            }
                            if ch.is_alphanumeric() || ch == '#' {
                                entity.push(self.advance().unwrap());
                                if entity.len() > 32 {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        let decoded = decode_entities(&entity);
                        let mut chars = decoded.chars();
                        if let Some(first) = chars.next() {
                            for remaining in chars {
                                self.pending_chars.push_back(remaining);
                            }
                            return Token::Character(first);
                        }
                    }

                    match self.advance() {
                        Some(ch) => return Token::Character(ch),
                        None => {
                            self.state = State::Data;
                            return Token::EndOfFile;
                        }
                    }
                }
            }
        }
    }

    fn finish_attribute(&mut self) {
        if !self.current_attr_name.is_empty() {
            let decoded_val = decode_entities(&self.current_attr_val);
            self.current_attributes.push((
                std::mem::take(&mut self.current_attr_name),
                decoded_val,
            ));
            self.current_attr_val.clear();
        }
    }

    fn emit_current_tag(&mut self) -> Token {
        if self.is_end_tag {
            Token::EndTag {
                name: std::mem::take(&mut self.current_tag_name),
            }
        } else {
            Token::StartTag {
                name: std::mem::take(&mut self.current_tag_name),
                attributes: std::mem::take(&mut self.current_attributes),
                self_closing: self.self_closing,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic_tags() {
        let html = "<div class=\"container\" id='main'>Hello</div>";
        let mut tokenizer = Tokenizer::new(html);

        match tokenizer.next_token() {
            Token::StartTag { name, attributes, self_closing } => {
                assert_eq!(name, "div");
                assert_eq!(attributes, vec![
                    ("class".to_string(), "container".to_string()),
                    ("id".to_string(), "main".to_string())
                ]);
                assert!(!self_closing);
            }
            other => panic!("expected StartTag, got {:?}", other),
        }

        let mut text = String::new();
        loop {
            match tokenizer.next_token() {
                Token::Character(c) => text.push(c),
                Token::EndTag { name } => {
                    assert_eq!(name, "div");
                    break;
                }
                other => panic!("unexpected token {:?}", other),
            }
        }
        assert_eq!(text, "Hello");
    }

    #[test]
    fn test_tokenize_doctype() {
        let html = "<!DOCTYPE html><html>";
        let mut tokenizer = Tokenizer::new(html);

        match tokenizer.next_token() {
            Token::Doctype { name, .. } => {
                assert_eq!(name, Some("html".to_string()));
            }
            other => panic!("expected Doctype, got {:?}", other),
        }

        match tokenizer.next_token() {
            Token::StartTag { name, .. } => {
                assert_eq!(name, "html");
            }
            other => panic!("expected StartTag, got {:?}", other),
        }
    }

    #[test]
    fn test_tokenize_comment() {
        let html = "<!-- this is a comment -->";
        let mut tokenizer = Tokenizer::new(html);

        match tokenizer.next_token() {
            Token::Comment(text) => {
                assert_eq!(text, " this is a comment ");
            }
            other => panic!("expected Comment, got {:?}", other),
        }
    }

    #[test]
    fn test_self_closing_tag() {
        let html = "<img src=\"mango.png\" alt=\"Mango\" />";
        let mut tokenizer = Tokenizer::new(html);

        match tokenizer.next_token() {
            Token::StartTag { name, attributes, self_closing } => {
                assert_eq!(name, "img");
                assert!(self_closing);
                assert_eq!(attributes.len(), 2);
            }
            other => panic!("expected StartTag, got {:?}", other),
        }
    }

    #[test]
    fn test_multi_char_and_unrecognized_entity_decoding() {
        // Unknown entity should not drop characters
        let html = "a &unknown; b";
        let mut tokenizer = Tokenizer::new(html);
        let mut text = String::new();
        loop {
            match tokenizer.next_token() {
                Token::Character(c) => text.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        assert_eq!(text, "a &unknown; b");

        // Decoded entity
        let html2 = "one &amp; two";
        let mut tokenizer2 = Tokenizer::new(html2);
        let mut text2 = String::new();
        loop {
            match tokenizer2.next_token() {
                Token::Character(c) => text2.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        assert_eq!(text2, "one & two");
    }
}
