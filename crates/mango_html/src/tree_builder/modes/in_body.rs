//! InBody insertion mode handling elements, text, and formatting tokens.

use super::foreign::adjust_svg_attributes;
use crate::dom::NodeData;
use crate::elements::{
    is_active_formatting_element, is_escapable_raw_text_element, is_raw_text_element,
    is_void_element,
};
use crate::tokenizer::Token;
use crate::tree_builder::types::{FormattingEntry, InsertionMode, TreeBuilder};

impl<'a> TreeBuilder<'a> {
    pub(crate) fn process_token_in_body(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "html" {
                    // Merge attributes onto existing <html> element per WHATWG InBody.
                    for &id in &self.open_elements {
                        if let Some(node) = self.doc.get_mut(id)
                            && let NodeData::Element(el) = &mut node.data
                            && el.tag_name == "html"
                        {
                            for (attr_name, attr_val) in attributes {
                                if !el
                                    .attributes
                                    .iter()
                                    .any(|(k, _)| k.eq_ignore_ascii_case(&attr_name))
                                {
                                    el.attributes.push((attr_name, attr_val));
                                }
                            }
                            break;
                        }
                    }
                    return;
                }
                if name == "body" {
                    for &id in &self.open_elements {
                        if let Some(node) = self.doc.get_mut(id)
                            && let NodeData::Element(el) = &mut node.data
                            && el.tag_name == "body"
                        {
                            for (attr_name, attr_val) in attributes {
                                if !el
                                    .attributes
                                    .iter()
                                    .any(|(k, _)| k.eq_ignore_ascii_case(&attr_name))
                                {
                                    el.attributes.push((attr_name, attr_val));
                                }
                            }
                            break;
                        }
                    }
                    return;
                }

                if name == "frameset" {
                    if !self.frameset_ok {
                        // Per spec, non-ok frameset token must be ignored, not inserted.
                        return;
                    }
                    while let Some(top) = self.open_elements.last() {
                        if let Some(node) = self.doc.get(*top)
                            && let NodeData::Element(el) = &node.data
                            && el.tag_name == "body"
                        {
                            self.open_elements.pop();
                            break;
                        }
                        self.open_elements.pop();
                    }
                    let frameset = self.insert_element(&name, attributes);
                    self.open_elements.push(frameset);
                    self.mode = InsertionMode::InFrameset;
                    return;
                }

                // Per WHATWG, frameset-ok set to "not ok" on content start tags
                // (div/p/h1-h6 etc), not just non-whitespace text.
                // Conservative: any start tag other than html/body/frameset clears it.
                if name != "html" && name != "body" && name != "frameset" {
                    self.frameset_ok = false;
                }

                if name == "svg" || name == "math" {
                    self.reconstruct_active_formatting_elements();
                    let mut attrs = attributes.clone();
                    adjust_svg_attributes(&mut attrs);
                    let elem = self.insert_element(&name, attrs);
                    if !self_closing {
                        self.open_elements.push(elem);
                    }
                    return;
                }

                if name == "table" {
                    if self.is_in_button_scope("p") {
                        self.pop_until("p");
                    }
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.active_formatting_elements
                        .push(FormattingEntry::Marker);
                    self.mode = InsertionMode::InTable;
                    return;
                }

                if name == "select" {
                    self.reconstruct_active_formatting_elements();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.active_formatting_elements
                        .push(FormattingEntry::Marker);
                    if matches!(
                        self.mode,
                        InsertionMode::InTable
                            | InsertionMode::InCaption
                            | InsertionMode::InTableBody
                            | InsertionMode::InRow
                            | InsertionMode::InCell
                    ) {
                        self.mode = InsertionMode::InSelectInTable;
                    } else {
                        self.mode = InsertionMode::InSelect;
                    }
                    return;
                }

                if name == "template" {
                    self.process_template_start(attributes);
                    return;
                }

                if is_active_formatting_element(&name) {
                    if name == "a" {
                        let has_a = self
                            .active_formatting_elements
                            .iter()
                            .rposition(|e| match e {
                                FormattingEntry::Marker => false,
                                FormattingEntry::Element { tag_name, .. } => {
                                    tag_name.eq_ignore_ascii_case("a")
                                }
                            });
                        if let Some(_cached_idx) = has_a {
                            let old_node_id_opt =
                                match &self.active_formatting_elements[_cached_idx] {
                                    FormattingEntry::Element { node_id, .. } => Some(*node_id),
                                    FormattingEntry::Marker => None,
                                };
                            self.handle_formatting_end_tag("a");
                            // Re-lookup by NodeId after AAA instead of cached idx
                            // (AAA may have shifted/removed entries; avoid double-remove).
                            if let Some(old_node_id) = old_node_id_opt
                                && let Some(new_idx) = self.active_formatting_elements.iter().position(
                                |e| matches!(e, FormattingEntry::Element { node_id, .. } if *node_id == old_node_id),
                            ) {
                                self.open_elements.retain(|&id| id != old_node_id);
                                self.active_formatting_elements.remove(new_idx);
                            }
                        }
                    }

                    self.reconstruct_active_formatting_elements();
                    let elem = self.insert_element(&name, attributes.clone());
                    if !self_closing {
                        self.open_elements.push(elem);
                        self.push_active_formatting_element(&name, &attributes, elem);
                    }
                    return;
                }

                if matches!(
                    name.as_str(),
                    "span"
                        | "sub"
                        | "sup"
                        | "mark"
                        | "img"
                        | "br"
                        | "input"
                        | "time"
                        | "abbr"
                        | "cite"
                ) {
                    self.reconstruct_active_formatting_elements();
                }

                if matches!(
                    name.as_str(),
                    "p" | "div"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "ul"
                        | "ol"
                        | "dl"
                        | "blockquote"
                        | "hr"
                        | "pre"
                        | "form"
                        | "fieldset"
                        | "section"
                        | "article"
                        | "header"
                        | "footer"
                        | "nav"
                        | "aside"
                        | "main"
                        | "figure"
                        | "figcaption"
                        | "dialog"
                ) && self.is_in_button_scope("p")
                {
                    self.pop_until("p");
                }

                if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
                    && self.has_heading_in_scope()
                {
                    self.pop_heading_in_scope();
                }

                if (name == "rt" || name == "rp") && self.is_in_scope("ruby") {
                    self.generate_implied_end_tags(Some("rtc"));
                }

                if name == "li" {
                    if self.is_in_list_item_scope("li") {
                        self.generate_implied_end_tags(Some("li"));
                        self.pop_until("li");
                    }
                    if self.is_in_button_scope("p") {
                        self.pop_until("p");
                    }
                }

                if name == "dd" || name == "dt" {
                    if self.is_in_scope("dd") {
                        self.generate_implied_end_tags(Some("dd"));
                        self.pop_until("dd");
                    }
                    if self.is_in_scope("dt") {
                        self.generate_implied_end_tags(Some("dt"));
                        self.pop_until("dt");
                    }
                    if self.is_in_button_scope("p") {
                        self.pop_until("p");
                    }
                }

                if name == "optgroup" {
                    if let Some(curr) = self.current_tag_name()
                        && curr == "option"
                    {
                        self.open_elements.pop();
                    }
                    if let Some(curr) = self.current_tag_name()
                        && curr == "optgroup"
                    {
                        self.open_elements.pop();
                    }
                }

                if name == "option"
                    && let Some(curr) = self.current_tag_name()
                    && curr == "option"
                {
                    self.open_elements.pop();
                }

                let is_foreign_self_closing = self_closing
                    && matches!(
                        name.to_ascii_lowercase().as_str(),
                        "path"
                            | "circle"
                            | "rect"
                            | "line"
                            | "polygon"
                            | "polyline"
                            | "ellipse"
                            | "use"
                            | "stop"
                    );
                let is_void = is_void_element(&name) || is_foreign_self_closing;
                let is_raw = is_raw_text_element(&name) || is_escapable_raw_text_element(&name);

                let elem = self.insert_element(&name, attributes);

                if !is_void {
                    self.open_elements.push(elem);
                    if is_raw {
                        self.tokenizer.set_raw_text_mode(&name);
                    }
                }
            }

            Token::EndTag { name } => {
                if name == "body" || name == "html" {
                    self.mode = InsertionMode::AfterBody;
                } else if name == "br" {
                    // Per WHATWG, </br> acts as <br>: create a br element.
                    self.reconstruct_active_formatting_elements();
                    let br = self.insert_element("br", Vec::new());
                    // br is void: do not push onto open elements.
                    let _ = br;
                } else if is_active_formatting_element(&name) {
                    self.handle_formatting_end_tag(&name);
                } else if name == "table" {
                    self.clear_active_formatting_to_marker();
                    if self.is_in_table_scope("table") {
                        self.pop_until("table");
                        self.reset_insertion_mode();
                    }
                } else if name == "select" {
                    self.clear_active_formatting_to_marker();
                    if self.is_in_select_scope("select") {
                        self.pop_until("select");
                        self.reset_insertion_mode();
                    }
                } else if name == "template" {
                    self.process_template_end();
                } else if name == "p" {
                    if !self.is_in_button_scope("p") {
                        let p = self.insert_element("p", Vec::new());
                        self.open_elements.push(p);
                    }
                    self.generate_implied_end_tags(Some("p"));
                    self.pop_until("p");
                } else if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                    if self.has_heading_in_scope() {
                        self.generate_implied_end_tags(None);
                        self.pop_heading_in_scope();
                    }
                } else if self.is_in_scope(&name) {
                    self.generate_implied_end_tags(Some(&name));
                    self.pop_until(&name);
                }
            }

            Token::Comment(c) => {
                self.insert_comment(&c);
            }

            Token::EndOfFile => {
                self.mode = InsertionMode::AfterAfterBody;
            }

            _ => {}
        }
    }
}
