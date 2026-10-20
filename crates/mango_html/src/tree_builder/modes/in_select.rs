//! Select-related insertion modes: InSelect, InSelectInTable.

use crate::tokenizer::Token;
use crate::tree_builder::types::TreeBuilder;

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_token_in_select(&mut self, token: Token) {
        match token {
            Token::Character(c) => {
                self.text_buffer.push(c);
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "option" {
                    if self.current_tag_name().as_deref() == Some("option") {
                        self.open_elements.pop();
                    }
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    return;
                }
                if name == "optgroup" {
                    if self.current_tag_name().as_deref() == Some("option") {
                        self.open_elements.pop();
                    }
                    if self.current_tag_name().as_deref() == Some("optgroup") {
                        self.open_elements.pop();
                    }
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    return;
                }
                if name == "select" {
                    if self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                    }
                    return;
                }
                if matches!(name.as_str(), "input" | "keygen" | "textarea") {
                    if self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                if name == "script" || name == "template" {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                }
            }
            Token::EndTag { name } => {
                if name == "optgroup" {
                    if self.current_tag_name().as_deref() == Some("option") {
                        self.open_elements.pop();
                    }
                    if self.current_tag_name().as_deref() == Some("optgroup") {
                        self.open_elements.pop();
                    }
                    return;
                }
                if name == "option" {
                    if self.current_tag_name().as_deref() == Some("option") {
                        self.open_elements.pop();
                    }
                    return;
                }
                if name == "select" {
                    if self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                    }
                    return;
                }
                if name == "template" {
                    self.process_token_in_head(Token::EndTag { name });
                }
            }
            Token::EndOfFile => {
                self.process_token_in_body(Token::EndOfFile);
            }
            _ => {}
        }
    }

    pub(crate) fn process_token_in_select_in_table(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if matches!(
                    name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) {
                    if self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                self.process_token_in_select(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            Token::EndTag { name } => {
                if matches!(
                    name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) {
                    if self.is_in_table_scope(&name) && self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                self.process_token_in_select(Token::EndTag { name });
            }
            _ => {
                self.process_token_in_select(token);
            }
        }
    }
}
