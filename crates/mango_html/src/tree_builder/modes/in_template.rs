//! Template, Frameset, and trailing insertion modes.

use crate::dom::NodeData;
use crate::tokenizer::Token;
use crate::tree_builder::types::{InsertionMode, TreeBuilder};

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_token_in_template(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if matches!(
                    name.as_str(),
                    "base"
                        | "basefont"
                        | "bgsound"
                        | "link"
                        | "meta"
                        | "noframes"
                        | "script"
                        | "style"
                        | "template"
                        | "title"
                ) {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if matches!(
                    name.as_str(),
                    "caption" | "colgroup" | "tbody" | "tfoot" | "thead"
                ) {
                    self.template_insertion_modes.pop();
                    self.template_insertion_modes.push(InsertionMode::InTable);
                    self.mode = InsertionMode::InTable;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if name == "col" {
                    self.template_insertion_modes.pop();
                    self.template_insertion_modes
                        .push(InsertionMode::InColumnGroup);
                    self.mode = InsertionMode::InColumnGroup;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if name == "tr" {
                    self.template_insertion_modes.pop();
                    self.template_insertion_modes
                        .push(InsertionMode::InTableBody);
                    self.mode = InsertionMode::InTableBody;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if name == "td" || name == "th" {
                    self.template_insertion_modes.pop();
                    self.template_insertion_modes.push(InsertionMode::InRow);
                    self.mode = InsertionMode::InRow;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                self.template_insertion_modes.pop();
                self.template_insertion_modes.push(InsertionMode::InBody);
                self.mode = InsertionMode::InBody;
                self.process_token(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            Token::EndTag { name } => {
                if name == "template" {
                    self.process_template_end();
                } else if matches!(name.as_str(), "title" | "style" | "script") {
                    self.process_token_in_head(Token::EndTag { name });
                }
            }
            Token::Character(c) => {
                if c.is_ascii_whitespace() {
                    return;
                }
                self.template_insertion_modes.pop();
                self.template_insertion_modes.push(InsertionMode::InBody);
                self.mode = InsertionMode::InBody;
                self.process_token(Token::Character(c));
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::EndOfFile
                if self.open_elements.iter().any(|&id| {
                    self.doc.get(id).map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template")).unwrap_or(false)
                }) => {
                    self.process_template_end();
                    self.process_token(Token::EndOfFile);
                }
            _ => {}
        }
    }

    pub(crate) fn process_token_in_frameset(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_ascii_whitespace() => {
                self.insert_text(&c.to_string());
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "frameset" {
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                } else if name == "frame" {
                    self.insert_element(&name, attributes);
                } else if name == "noframes" {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                }
            }
            Token::EndTag { name }
                if name == "frameset" && self.current_tag_name().as_deref() != Some("html") =>
            {
                self.open_elements.pop();
                if self.current_tag_name().as_deref() != Some("frameset") {
                    self.mode = InsertionMode::AfterFrameset;
                }
            }
            _ => {}
        }
    }

    pub(crate) fn process_token_after_frameset(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_ascii_whitespace() => {
                self.insert_text(&c.to_string());
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } if name == "noframes" => {
                self.process_token_in_head(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            Token::EndTag { name } if name == "html" => {
                self.mode = InsertionMode::AfterAfterBody;
            }
            _ => {}
        }
    }

    pub(crate) fn process_token_after_body(&mut self, token: Token) {
        match token {
            Token::EndTag { name } if name == "html" => {
                self.mode = InsertionMode::AfterAfterBody;
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::EndOfFile => {
                self.mode = InsertionMode::AfterAfterBody;
            }
            _ => {
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }

    pub(crate) fn process_token_after_after_body(&mut self, token: Token) {
        match token {
            Token::Comment(c) => {
                let root = self.doc.root();
                let child = self.doc.create_comment(&c);
                self.doc.append_child(root, child);
            }
            Token::Doctype { .. } => {}
            Token::Character(c) if c.is_ascii_whitespace() => {}
            Token::EndOfFile => {}
            _ => {
                self.mode = InsertionMode::InBody;
                self.process_token(token);
            }
        }
    }
}
