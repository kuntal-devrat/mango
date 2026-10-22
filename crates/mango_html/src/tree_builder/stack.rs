//! Open element stack, insertion algorithms, and scope checking.

use super::types::{FormattingEntry, InsertionMode, TreeBuilder};
use crate::dom::{Document, NodeData, NodeId};
use crate::tokenizer::{Token, Tokenizer};

impl<'a> TreeBuilder<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            tokenizer: Tokenizer::new(input),
            doc: Document::new(),
            mode: InsertionMode::Initial,
            open_elements: Vec::new(),
            active_formatting_elements: Vec::new(),
            head_element: None,
            text_buffer: String::new(),
            template_insertion_modes: Vec::new(),
            original_insertion_mode: None,
            foster_parenting: false,
            frameset_ok: true,
        }
    }

    /// Parses the entire HTML string into a [`Document`].
    pub fn parse(input: &'a str) -> Document {
        let mut builder = Self::new(input);
        builder.run();
        builder.doc.clear_dirty();
        builder.doc
    }

    /// Runs the tree builder to completion.
    pub fn run(&mut self) {
        loop {
            let token = self.tokenizer.next_token();
            if token == Token::EndOfFile {
                self.flush_text_buffer();
                self.process_token(token);
                break;
            }

            // Buffer character tokens to collapse adjacent characters into single Text nodes
            if let Token::Character(ch) = token {
                self.text_buffer.push(ch);
                continue;
            }

            self.flush_text_buffer();
            self.process_token(token);
        }
    }

    pub(crate) fn flush_text_buffer(&mut self) {
        if self.text_buffer.is_empty() {
            return;
        }

        let text = std::mem::take(&mut self.text_buffer);

        let is_in_head_container = (self.mode == InsertionMode::InHead
            || self.mode == InsertionMode::InTemplate)
            && self
                .current_tag_name()
                .map(|t| t == "title" || t == "style" || t == "script")
                .unwrap_or(false);

        // In Initial, BeforeHtml, BeforeHead, AfterHead, and InHead/InTemplate (outside text container) modes, ignore whitespace text
        if (self.mode == InsertionMode::Initial
            || self.mode == InsertionMode::BeforeHtml
            || self.mode == InsertionMode::BeforeHead
            || self.mode == InsertionMode::AfterHead
            || ((self.mode == InsertionMode::InHead || self.mode == InsertionMode::InTemplate)
                && !is_in_head_container))
            && text.chars().all(|c| c.is_ascii_whitespace())
        {
            return;
        }

        // InHead with non-whitespace text outside text container: pop head and transition to body per WHATWG §13.2.6.4.4
        if self.mode == InsertionMode::InHead && !is_in_head_container {
            self.pop_until("head");
            self.mode = InsertionMode::AfterHead;
            self.ensure_body();
        } else if matches!(
            self.mode,
            InsertionMode::Initial
                | InsertionMode::BeforeHtml
                | InsertionMode::BeforeHead
                | InsertionMode::AfterHead
        ) {
            self.ensure_body();
        }

        // In table insertion modes, handle whitespace vs misplaced text (foster parenting)
        if matches!(
            self.mode,
            InsertionMode::InTable
                | InsertionMode::InTableText
                | InsertionMode::InTableBody
                | InsertionMode::InRow
        ) {
            if text.chars().all(|c| c.is_ascii_whitespace()) {
                // Per WHATWG, whitespace in table modes must be inserted, not dropped.
                self.insert_text(&text);
                return;
            }
            // Foster-parent misplaced text before the table
            self.foster_parenting = true;
            self.reconstruct_active_formatting_elements();
            self.insert_text(&text);
            self.foster_parenting = false;
            self.frameset_ok = false;
            return;
        }

        if self.mode == InsertionMode::InColumnGroup {
            if text.chars().all(|c| c.is_ascii_whitespace()) {
                return;
            }
            if self.current_tag_name().as_deref() == Some("colgroup") {
                self.open_elements.pop();
                self.mode = InsertionMode::InTable;
                self.foster_parenting = true;
                self.reconstruct_active_formatting_elements();
                self.insert_text(&text);
                self.foster_parenting = false;
                self.frameset_ok = false;
                return;
            }
        }

        if self.mode == InsertionMode::InFrameset || self.mode == InsertionMode::AfterFrameset {
            if text.chars().all(|c| c.is_ascii_whitespace()) {
                self.insert_text(&text);
            }
            return;
        }

        if self.mode == InsertionMode::InBody {
            self.reconstruct_active_formatting_elements();
        }

        if !text.chars().all(|c| c.is_ascii_whitespace()) {
            self.frameset_ok = false;
        }

        self.insert_text(&text);
    }

    pub(crate) fn find_enclosing_table(&self) -> Option<NodeId> {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
                && el.tag_name.eq_ignore_ascii_case("table")
            {
                return Some(node_id);
            }
        }
        None
    }

    pub(crate) fn current_node(&self) -> NodeId {
        *self.open_elements.last().unwrap_or(&self.doc.root())
    }

    #[allow(dead_code)]
    pub(crate) fn current_tag_name(&self) -> Option<String> {
        let node_id = *self.open_elements.last()?;
        let node = self.doc.get(node_id)?;
        if let NodeData::Element(el) = &node.data {
            Some(el.tag_name.clone())
        } else {
            None
        }
    }

    pub(crate) fn target_node_for_insertion(&self) -> NodeId {
        let curr = self.current_node();
        if let Some(node) = self.doc.get(curr)
            && let NodeData::Element(el) = &node.data
            && el.tag_name.eq_ignore_ascii_case("template")
            && let Some(frag_id) = el.template_contents
        {
            return frag_id;
        }
        curr
    }

    pub(crate) fn appropriate_insertion_location(&self) -> (NodeId, Option<NodeId>) {
        if self.foster_parenting
            && let Some(table_id) = self.find_enclosing_table()
        {
            if let Some(table_node) = self.doc.get(table_id)
                && let Some(parent) = table_node.parent
            {
                return (parent, Some(table_id));
            }
            if let Some(pos) = self.open_elements.iter().rposition(|&id| id == table_id)
                && pos > 0
            {
                return (self.open_elements[pos - 1], None);
            }
        }

        let target = self.target_node_for_insertion();
        (target, None)
    }

    pub(crate) fn insert_element(
        &mut self,
        name: &str,
        attributes: Vec<(String, String)>,
    ) -> NodeId {
        if name.eq_ignore_ascii_case("meta") {
            self.process_meta_attributes(&attributes);
        }
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_element(name, attributes);
        if let Some(before_id) = before {
            let _ = self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    pub(crate) fn insert_text(&mut self, text: &str) -> NodeId {
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_text(text);
        if let Some(before_id) = before {
            let _ = self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    pub(crate) fn insert_comment(&mut self, comment: &str) -> NodeId {
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_comment(comment);
        if let Some(before_id) = before {
            let _ = self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    pub(crate) fn pop_until(&mut self, tag_name: &str) -> Option<NodeId> {
        let mut matched = None;
        while let Some(&node_id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
                && el.tag_name.eq_ignore_ascii_case(tag_name)
            {
                matched = Some(node_id);
                self.open_elements.pop();
                break;
            }
            self.open_elements.pop();
        }
        matched
    }

    pub(crate) fn process_template_start(&mut self, attributes: Vec<(String, String)>) {
        self.flush_text_buffer();

        let shadow_mode = attributes.iter().find_map(|(k, v)| {
            if k.eq_ignore_ascii_case("shadowrootmode") || k.eq_ignore_ascii_case("shadowroot") {
                let lower = v.trim().to_ascii_lowercase();
                if lower == "open" || lower == "closed" {
                    Some(lower)
                } else {
                    None
                }
            } else {
                None
            }
        });

        let mut dsd_shadow_root = None;
        if let Some(mode) = shadow_mode {
            let parent_target = self.current_node();
            if let Some(parent_node) = self.doc.get(parent_target)
                && let NodeData::Element(pel) = &parent_node.data
            {
                // Disallow template hosting shadow root and ensure host doesn't already have one
                if !pel.tag_name.eq_ignore_ascii_case("template") && pel.shadow_root.is_none() {
                    let clonable = attributes
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("shadowrootclonable"));
                    let delegates_focus = attributes
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("shadowrootdelegatesfocus"));
                    dsd_shadow_root = self.doc.attach_shadow_with_options(
                        parent_target,
                        &mode,
                        clonable,
                        delegates_focus,
                    );
                }
            }
        }

        let mut attrs = attributes;
        let is_dsd = dsd_shadow_root.is_some();
        if is_dsd {
            attrs.push(("data-mango-dsd".to_string(), "true".to_string()));
        }

        let elem = self.insert_element("template", attrs);
        if let Some(sr_id) = dsd_shadow_root {
            // Repoint template_contents to the newly attached shadow root
            if let Some(t_node) = self.doc.get_mut(elem)
                && let NodeData::Element(el) = &mut t_node.data
            {
                el.template_contents = Some(sr_id);
            }
        }

        self.open_elements.push(elem);
        self.active_formatting_elements
            .push(FormattingEntry::Marker);
        self.template_insertion_modes
            .push(InsertionMode::InTemplate);
        self.mode = InsertionMode::InTemplate;
    }

    pub(crate) fn process_template_end(&mut self) {
        self.flush_text_buffer();
        if self.open_elements.iter().any(|&id| {
            self.doc
                .get(id)
                .map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template"))
                .unwrap_or(false)
        }) {
            self.generate_implied_end_tags(None);
            if let Some(popped_id) = self.pop_until("template") {
                if let Some(node) = self.doc.get(popped_id)
                    && let NodeData::Element(el) = &node.data
                    && el.get_attribute("data-mango-dsd") == Some("true")
                {
                    self.doc.detach(popped_id);
                }
            }
            self.clear_active_formatting_to_marker();
            self.template_insertion_modes.pop();
            self.reset_insertion_mode();
        }
    }

    pub(crate) fn is_in_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case(tag_name) {
                    return true;
                }
                if matches!(
                    el.tag_name.as_str(),
                    "applet"
                        | "caption"
                        | "html"
                        | "table"
                        | "td"
                        | "th"
                        | "marquee"
                        | "object"
                        | "template"
                ) {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn is_in_table_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case(tag_name) {
                    return true;
                }
                if matches!(el.tag_name.as_str(), "html" | "table" | "template") {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn is_in_select_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case(tag_name) {
                    return true;
                }
                if !matches!(el.tag_name.as_str(), "option" | "optgroup") {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn is_in_button_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case(tag_name) {
                    return true;
                }
                if matches!(
                    el.tag_name.as_str(),
                    "applet"
                        | "caption"
                        | "html"
                        | "table"
                        | "td"
                        | "th"
                        | "marquee"
                        | "object"
                        | "template"
                        | "body"
                        | "button"
                ) {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn is_in_list_item_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
            {
                if el.tag_name.eq_ignore_ascii_case(tag_name) {
                    return true;
                }
                if matches!(
                    el.tag_name.as_str(),
                    "applet"
                        | "caption"
                        | "html"
                        | "table"
                        | "td"
                        | "th"
                        | "marquee"
                        | "object"
                        | "template"
                        | "body"
                        | "ol"
                        | "ul"
                ) {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn clear_stack_back_to_table_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
                && matches!(el.tag_name.as_str(), "table" | "template" | "html")
            {
                break;
            }
            self.open_elements.pop();
        }
    }

    pub(crate) fn clear_stack_back_to_table_body_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
                && matches!(
                    el.tag_name.as_str(),
                    "tbody" | "thead" | "tfoot" | "template" | "html"
                )
            {
                break;
            }
            self.open_elements.pop();
        }
    }

    pub(crate) fn clear_stack_back_to_table_row_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
                && matches!(el.tag_name.as_str(), "tr" | "template" | "html")
            {
                break;
            }
            self.open_elements.pop();
        }
    }

    pub(crate) fn close_cell(&mut self) {
        self.generate_implied_end_tags(None);
        while let Some(top) = self.open_elements.pop() {
            if let Some(node) = self.doc.get(top)
                && let NodeData::Element(el) = &node.data
                && matches!(el.tag_name.as_str(), "td" | "th")
            {
                break;
            }
        }
        self.clear_active_formatting_to_marker();
        self.mode = InsertionMode::InRow;
    }

    pub(crate) fn generate_implied_end_tags(&mut self, exclude: Option<&str>) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
            {
                let tag = el.tag_name.as_str();
                if Some(tag) == exclude {
                    break;
                }
                if matches!(
                    tag,
                    "dd" | "dt" | "li" | "optgroup" | "option" | "p" | "rb" | "rt" | "rtc" | "rp"
                ) {
                    self.open_elements.pop();
                    continue;
                }
            }
            break;
        }
    }

    pub(crate) fn reset_insertion_mode(&mut self) {
        let mut last = false;
        let stack_len = self.open_elements.len();
        if stack_len == 0 {
            self.mode = InsertionMode::InBody;
            return;
        }

        for (i, &node_id) in self.open_elements.iter().enumerate().rev() {
            if i == 0 {
                last = true;
            }
            let Some(node) = self.doc.get(node_id) else {
                continue;
            };
            let NodeData::Element(el) = &node.data else {
                continue;
            };
            let tag = el.tag_name.as_str();

            match tag {
                "select" => {
                    let mut in_table = false;
                    for &ancestor_id in self.open_elements[..i].iter().rev() {
                        if let Some(a_node) = self.doc.get(ancestor_id)
                            && let NodeData::Element(a_el) = &a_node.data
                            && matches!(
                                a_el.tag_name.as_str(),
                                "table" | "tbody" | "thead" | "tfoot" | "tr" | "td" | "th"
                            )
                        {
                            in_table = true;
                            break;
                        }
                    }
                    self.mode = if in_table {
                        InsertionMode::InSelectInTable
                    } else {
                        InsertionMode::InSelect
                    };
                    return;
                }
                "td" | "th" if !last => {
                    self.mode = InsertionMode::InCell;
                    return;
                }
                "tr" => {
                    self.mode = InsertionMode::InRow;
                    return;
                }
                "tbody" | "thead" | "tfoot" => {
                    self.mode = InsertionMode::InTableBody;
                    return;
                }
                "caption" => {
                    self.mode = InsertionMode::InCaption;
                    return;
                }
                "colgroup" => {
                    self.mode = InsertionMode::InColumnGroup;
                    return;
                }
                "table" => {
                    self.mode = InsertionMode::InTable;
                    return;
                }
                "template" => {
                    if let Some(&top_mode) = self.template_insertion_modes.last() {
                        self.mode = top_mode;
                    } else {
                        self.mode = InsertionMode::InBody;
                    }
                    return;
                }
                "head" if !last => {
                    self.mode = InsertionMode::InHead;
                    return;
                }
                "body" => {
                    self.mode = InsertionMode::InBody;
                    return;
                }
                "frameset" => {
                    self.mode = InsertionMode::InFrameset;
                    return;
                }
                "html" => {
                    if self.head_element.is_none() {
                        self.mode = InsertionMode::BeforeHead;
                    } else {
                        self.mode = InsertionMode::AfterHead;
                    }
                    return;
                }
                _ => {}
            }
        }
        self.mode = InsertionMode::InBody;
    }

    pub(crate) fn has_heading_in_scope(&self) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
            {
                if matches!(
                    el.tag_name.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                ) {
                    return true;
                }
                if matches!(
                    el.tag_name.as_str(),
                    "html"
                        | "table"
                        | "template"
                        | "body"
                        | "applet"
                        | "caption"
                        | "td"
                        | "th"
                        | "marquee"
                        | "object"
                        | "button"
                        | "ol"
                        | "ul"
                ) {
                    return false;
                }
            }
        }
        false
    }

    pub(crate) fn pop_heading_in_scope(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
                && matches!(
                    el.tag_name.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                )
            {
                self.open_elements.pop();
                break;
            }
            self.open_elements.pop();
        }
    }

    /// Reconstructs the active formatting elements according to WHATWG HTML5 §13.2.6.4.
    pub(crate) fn ensure_html(&mut self) -> NodeId {
        if self.open_elements.is_empty() {
            let html = self.insert_element("html", Vec::new());
            self.open_elements.push(html);
            html
        } else {
            self.open_elements[0]
        }
    }

    pub(crate) fn ensure_head(&mut self) -> NodeId {
        self.ensure_html();
        if let Some(head) = self.head_element {
            head
        } else {
            let head = self.insert_element("head", Vec::new());
            self.head_element = Some(head);
            self.open_elements.push(head);
            self.mode = InsertionMode::InHead;
            head
        }
    }

    pub(crate) fn ensure_body(&mut self) -> NodeId {
        self.ensure_html();
        // If in head, pop head
        if self.mode == InsertionMode::InHead {
            self.pop_until("head");
            self.mode = InsertionMode::AfterHead;
        }

        // Check if body is already on open elements stack
        for &id in &self.open_elements {
            if let Some(node) = self.doc.get(id)
                && let NodeData::Element(el) = &node.data
                && el.tag_name == "body"
            {
                self.mode = InsertionMode::InBody;
                return id;
            }
        }

        let body = self.insert_element("body", Vec::new());
        self.open_elements.push(body);
        self.mode = InsertionMode::InBody;
        body
    }
}
