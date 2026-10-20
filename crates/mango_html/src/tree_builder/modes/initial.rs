//! Document preamble insertion modes: Initial, BeforeHtml, BeforeHead, InHead, AfterHead.

use crate::dom::QuirksMode;
use crate::elements::{is_escapable_raw_text_element, is_raw_text_element, is_void_element};
use crate::tokenizer::Token;
use crate::tree_builder::types::{FormattingEntry, InsertionMode, TreeBuilder};

impl<'a> TreeBuilder<'a> {
    #[allow(clippy::if_same_then_else)]
    pub(crate) fn process_token_initial(&mut self, token: Token) {
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                let doc_name = name.as_deref().unwrap_or("");
                let pub_id = public_id.as_deref().unwrap_or("");
                let sys_id = system_id.as_deref().unwrap_or("");
                let doctype = self.doc.create_doctype(doc_name, pub_id, sys_id);
                let root = self.doc.root();
                self.doc.append_child(root, doctype);

                let pub_lower = pub_id.to_ascii_lowercase();
                if force_quirks || !doc_name.eq_ignore_ascii_case("html") {
                    self.doc.quirks_mode = QuirksMode::Quirks;
                } else if pub_lower.starts_with("+//silmaril//dtd html pro v0r11 19970101//")
                    || pub_lower.starts_with("-//as//dtd html 3.0 aswedit + extensions//")
                    || pub_lower.starts_with("-//advasoft ltd//dtd html 3.0 aswedit + extensions//")
                    || pub_lower.starts_with("-//ietf//dtd html 2.0//")
                    || pub_lower.starts_with("-//ietf//dtd html strict//")
                    || pub_lower.starts_with("-//w3c//dtd html 3 1995-03-24//")
                    || pub_lower.starts_with("-//w3c//dtd html 3.2//")
                    || pub_lower.starts_with("-//w3o//dtd w3 html 3.0//")
                    || (pub_lower.starts_with("-//w3c//dtd html 4.0 transitional//")
                        && sys_id.is_empty())
                    || (pub_lower.starts_with("-//w3c//dtd html 4.01 transitional//")
                        && sys_id.is_empty())
                {
                    self.doc.quirks_mode = QuirksMode::Quirks;
                } else if pub_lower.starts_with("-//w3c//dtd xhtml 1.0 frameset//")
                    || pub_lower.starts_with("-//w3c//dtd xhtml 1.0 transitional//")
                    || (pub_lower.starts_with("-//w3c//dtd html 4.0 transitional//")
                        && !sys_id.is_empty())
                    || (pub_lower.starts_with("-//w3c//dtd html 4.01 transitional//")
                        && !sys_id.is_empty())
                {
                    self.doc.quirks_mode = QuirksMode::LimitedQuirks;
                } else {
                    self.doc.quirks_mode = QuirksMode::NoQuirks;
                }

                self.mode = InsertionMode::BeforeHtml;
            }
            Token::Comment(c) => {
                let root = self.doc.root();
                let child = self.doc.create_comment(&c);
                self.doc.append_child(root, child);
            }
            Token::Character(c) if c.is_ascii_whitespace() => {}
            _ => {
                self.doc.quirks_mode = QuirksMode::Quirks;
                self.mode = InsertionMode::BeforeHtml;
                self.process_token(token);
            }
        }
    }

    pub(crate) fn process_token_before_html(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name, attributes, ..
            } if name == "html" => {
                let html = self.insert_element(&name, attributes);
                self.open_elements.push(html);
                self.mode = InsertionMode::BeforeHead;
            }
            Token::Comment(c) => {
                let root = self.doc.root();
                let child = self.doc.create_comment(&c);
                self.doc.append_child(root, child);
            }
            Token::Character(c) if c.is_ascii_whitespace() => {}
            _ => {
                self.ensure_html();
                self.mode = InsertionMode::BeforeHead;
                self.process_token(token);
            }
        }
    }

    pub(crate) fn process_token_before_head(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name, attributes, ..
            } if name == "head" => {
                let head = self.insert_element(&name, attributes);
                self.head_element = Some(head);
                self.open_elements.push(head);
                self.mode = InsertionMode::InHead;
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::Character(c) if c.is_ascii_whitespace() => {}
            _ => {
                self.ensure_head();
                self.process_token(token);
            }
        }
    }

    pub(crate) fn process_token_in_head(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name, attributes, ..
            } if name == "title" || name == "style" || name == "script" => {
                self.flush_text_buffer();
                let is_raw = is_raw_text_element(&name) || is_escapable_raw_text_element(&name);
                let elem = self.insert_element(&name, attributes);
                self.open_elements.push(elem);
                if is_raw {
                    self.tokenizer.set_raw_text_mode(&name);
                }
            }
            Token::StartTag {
                name, attributes, ..
            } if name == "template" => {
                self.process_template_start(attributes);
            }
            Token::StartTag {
                name, attributes, ..
            } if is_void_element(&name) => {
                self.insert_element(&name, attributes);
            }
            Token::StartTag { name, .. } if name == "head" => {}
            Token::Character(c) => {
                let is_inside_text_container = self
                    .current_tag_name()
                    .map(|t| t == "title" || t == "style" || t == "script")
                    .unwrap_or(false);
                if is_inside_text_container {
                    self.text_buffer.push(c);
                } else if c.is_ascii_whitespace() {
                } else {
                    self.flush_text_buffer();
                    self.pop_until("head");
                    self.mode = InsertionMode::AfterHead;
                    self.process_token(Token::Character(c));
                }
            }
            Token::EndTag { name } if name == "head" => {
                self.flush_text_buffer();
                self.pop_until("head");
                self.mode = InsertionMode::AfterHead;
            }
            Token::EndTag { name } if name == "template" => {
                self.process_template_end();
            }
            Token::EndTag { name } if name == "title" || name == "style" || name == "script" => {
                self.flush_text_buffer();
                self.pop_until(&name);
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            _ => {
                self.flush_text_buffer();
                self.pop_until("head");
                self.mode = InsertionMode::AfterHead;
                self.process_token(token);
            }
        }
    }

    pub(crate) fn process_token_after_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_ascii_whitespace() => {}
            Token::StartTag {
                name, attributes, ..
            } if name == "body" => {
                let body = self.insert_element(&name, attributes);
                self.open_elements.push(body);
                self.mode = InsertionMode::InBody;
            }
            Token::StartTag {
                name, attributes, ..
            } if name == "frameset" => {
                let frameset = self.insert_element(&name, attributes);
                self.open_elements.push(frameset);
                self.mode = InsertionMode::InFrameset;
            }
            Token::StartTag {
                name, attributes, ..
            } if name == "template" => {
                let elem = self.insert_element(&name, attributes);
                self.open_elements.push(elem);
                self.active_formatting_elements
                    .push(FormattingEntry::Marker);
                self.template_insertion_modes
                    .push(InsertionMode::InTemplate);
                self.mode = InsertionMode::InTemplate;
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            _ => {
                self.ensure_body();
                self.process_token(token);
            }
        }
    }
}
