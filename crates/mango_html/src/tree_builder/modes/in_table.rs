//! Table-related insertion modes: InTable, InTableBody, InRow, InCell, InCaption, InColumnGroup.

use crate::tokenizer::Token;
use crate::tree_builder::types::{FormattingEntry, InsertionMode, TreeBuilder};

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_token_in_table(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "caption" {
                    self.clear_stack_back_to_table_context();
                    self.active_formatting_elements
                        .push(FormattingEntry::Marker);
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InCaption;
                    return;
                }
                if name == "colgroup" {
                    self.clear_stack_back_to_table_context();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InColumnGroup;
                    return;
                }
                if name == "col" {
                    self.clear_stack_back_to_table_context();
                    let elem = self.insert_element("colgroup", Vec::new());
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InColumnGroup;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing: false,
                    });
                    return;
                }
                if matches!(name.as_str(), "tbody" | "thead" | "tfoot") {
                    self.clear_stack_back_to_table_context();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InTableBody;
                    return;
                }
                if matches!(name.as_str(), "td" | "th" | "tr") {
                    self.clear_stack_back_to_table_context();
                    let elem = self.insert_element("tbody", Vec::new());
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InTableBody;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing: false,
                    });
                    return;
                }
                if name == "table" {
                    if self.is_in_table_scope("table") {
                        self.pop_until("table");
                        self.reset_insertion_mode();
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                if name == "style" || name == "script" {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if name == "input" {
                    let is_hidden = attributes.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("type") && v.eq_ignore_ascii_case("hidden")
                    });
                    if is_hidden {
                        self.insert_element(&name, attributes);
                        return;
                    }
                }
                if name == "template" {
                    self.process_template_start(attributes);
                    return;
                }

                // Foster parenting for misplaced elements
                self.foster_parenting = true;
                self.process_token_in_body(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
                self.foster_parenting = false;
            }
            Token::EndTag { name } => {
                if name == "table" {
                    if self.is_in_table_scope("table") {
                        self.pop_until("table");
                        self.reset_insertion_mode();
                    }
                    return;
                }
                if matches!(
                    name.as_str(),
                    "body"
                        | "caption"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) {
                    return;
                }
                if name == "template" {
                    self.process_token_in_head(Token::EndTag { name });
                    return;
                }

                // Foster parenting for end tags
                self.foster_parenting = true;
                self.process_token_in_body(Token::EndTag { name });
                self.foster_parenting = false;
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::EndOfFile => {
                self.process_token_in_body(Token::EndOfFile);
            }
            _ => {}
        }
    }

    pub(crate) fn process_token_in_table_body(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "tr" {
                    self.clear_stack_back_to_table_body_context();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InRow;
                    return;
                }
                if name == "th" || name == "td" {
                    self.clear_stack_back_to_table_body_context();
                    let elem = self.insert_element("tr", Vec::new());
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InRow;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing: false,
                    });
                    return;
                }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead"
                ) {
                    if self.is_in_table_scope("tbody")
                        || self.is_in_table_scope("thead")
                        || self.is_in_table_scope("tfoot")
                    {
                        self.clear_stack_back_to_table_body_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTable;
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                self.process_token_in_table(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            Token::EndTag { name } => {
                if matches!(name.as_str(), "tbody" | "tfoot" | "thead") {
                    if self.is_in_table_scope(&name) {
                        self.clear_stack_back_to_table_body_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTable;
                    }
                    return;
                }
                if name == "table" {
                    if self.is_in_table_scope("tbody")
                        || self.is_in_table_scope("thead")
                        || self.is_in_table_scope("tfoot")
                    {
                        self.clear_stack_back_to_table_body_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTable;
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" | "tr"
                ) {
                    return;
                }
                self.process_token_in_table(Token::EndTag { name });
            }
            _ => {
                self.process_token_in_table(token);
            }
        }
    }

    pub(crate) fn process_token_in_row(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "th" || name == "td" {
                    self.clear_stack_back_to_table_row_context();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.mode = InsertionMode::InCell;
                    self.active_formatting_elements
                        .push(FormattingEntry::Marker);
                    return;
                }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr"
                ) {
                    if self.is_in_table_scope("tr") {
                        self.clear_stack_back_to_table_row_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTableBody;
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                self.process_token_in_table(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            Token::EndTag { name } => {
                if name == "tr" {
                    if self.is_in_table_scope("tr") {
                        self.clear_stack_back_to_table_row_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTableBody;
                    }
                    return;
                }
                if name == "table" {
                    if self.is_in_table_scope("tr") {
                        self.clear_stack_back_to_table_row_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTableBody;
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                if matches!(name.as_str(), "tbody" | "tfoot" | "thead") {
                    if self.is_in_table_scope(&name) && self.is_in_table_scope("tr") {
                        self.clear_stack_back_to_table_row_context();
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTableBody;
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th"
                ) {
                    return;
                }
                self.process_token_in_table(Token::EndTag { name });
            }
            _ => {
                self.process_token_in_table(token);
            }
        }
    }

    pub(crate) fn process_token_in_cell(&mut self, token: Token) {
        match token {
            Token::EndTag { name } => {
                if name == "td" || name == "th" {
                    if self.is_in_table_scope(&name) {
                        self.generate_implied_end_tags(None);
                        self.pop_until(&name);
                        self.clear_active_formatting_to_marker();
                        self.mode = InsertionMode::InRow;
                    }
                    return;
                }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html"
                ) {
                    return;
                }
                if matches!(name.as_str(), "table" | "tbody" | "tfoot" | "thead" | "tr") {
                    if self.is_in_table_scope(&name) {
                        self.close_cell();
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                self.process_token_in_body(Token::EndTag { name });
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) {
                    if self.is_in_table_scope("td") || self.is_in_table_scope("th") {
                        self.close_cell();
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                self.process_token_in_body(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            _ => {
                self.process_token_in_body(token);
            }
        }
    }

    pub(crate) fn process_token_in_caption(&mut self, token: Token) {
        match token {
            Token::EndTag { name } => {
                if name == "caption" {
                    if self.is_in_table_scope("caption") {
                        self.generate_implied_end_tags(None);
                        self.pop_until("caption");
                        self.clear_active_formatting_to_marker();
                        self.mode = InsertionMode::InTable;
                    }
                    return;
                }
                if name == "table" {
                    if self.is_in_table_scope("caption") {
                        self.generate_implied_end_tags(None);
                        self.pop_until("caption");
                        self.clear_active_formatting_to_marker();
                        self.mode = InsertionMode::InTable;
                        self.process_token(Token::EndTag { name });
                    }
                    return;
                }
                if matches!(
                    name.as_str(),
                    "body"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) {
                    return;
                }
                self.process_token_in_body(Token::EndTag { name });
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) {
                    if self.is_in_table_scope("caption") {
                        self.generate_implied_end_tags(None);
                        self.pop_until("caption");
                        self.clear_active_formatting_to_marker();
                        self.mode = InsertionMode::InTable;
                        self.process_token(Token::StartTag {
                            name,
                            attributes,
                            self_closing,
                        });
                    }
                    return;
                }
                self.process_token_in_body(Token::StartTag {
                    name,
                    attributes,
                    self_closing,
                });
            }
            _ => {
                self.process_token_in_body(token);
            }
        }
    }

    pub(crate) fn process_token_in_column_group(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "col" {
                    self.insert_element(&name, attributes);
                    return;
                }
                if name == "template" {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if self.current_tag_name().as_deref() == Some("colgroup") {
                    self.open_elements.pop();
                    self.mode = InsertionMode::InTable;
                    self.process_token(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                }
            }
            Token::EndTag { name } => {
                if name == "colgroup" {
                    if self.current_tag_name().as_deref() == Some("colgroup") {
                        self.open_elements.pop();
                        self.mode = InsertionMode::InTable;
                    }
                    return;
                }
                if name == "col" {
                    return;
                }
                if name == "template" {
                    self.process_token_in_head(Token::EndTag { name });
                    return;
                }
                if self.current_tag_name().as_deref() == Some("colgroup") {
                    self.open_elements.pop();
                    self.mode = InsertionMode::InTable;
                    self.process_token(Token::EndTag { name });
                }
            }
            Token::Character(c) if c.is_ascii_whitespace() => {
                self.insert_text(&c.to_string());
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::EndOfFile => {
                self.process_token_in_body(Token::EndOfFile);
            }
            _ => {
                if self.current_tag_name().as_deref() == Some("colgroup") {
                    self.open_elements.pop();
                    self.mode = InsertionMode::InTable;
                    self.process_token(token);
                }
            }
        }
    }
}
