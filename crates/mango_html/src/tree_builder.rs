//! WHATWG HTML5 compliant tree builder.
//!
//! Consumes tokens from [`Tokenizer`] and builds an arena-allocated [`Document`].
//! Implements insertion modes, open element stack, implied tag closing, and
//! resilient error recovery.

use crate::dom::{Document, NodeData, NodeId, QuirksMode};
use crate::elements::{
    is_active_formatting_element, is_escapable_raw_text_element, is_raw_text_element,
    is_special_element, is_void_element,
};
use crate::tokenizer::{Token, Tokenizer};

/// An entry in the WHATWG list of active formatting elements.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattingEntry {
    Element {
        tag_name: String,
        attributes: Vec<(String, String)>,
        node_id: NodeId,
    },
    Marker,
}

/// HTML tree builder insertion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    InFrameset,
    AfterFrameset,
    AfterBody,
    AfterAfterBody,
}

/// The HTML tree builder state machine.
pub struct TreeBuilder<'a> {
    tokenizer: Tokenizer<'a>,
    doc: Document,
    mode: InsertionMode,
    /// Stack of open elements (NodeIds in the document).
    open_elements: Vec<NodeId>,
    /// List of active formatting elements (WHATWG HTML5 §13.2.4.3).
    active_formatting_elements: Vec<FormattingEntry>,
    /// Pointer to the `<head>` element node, if created.
    head_element: Option<NodeId>,
    /// Buffer for accumulating adjacent character tokens.
    text_buffer: String,
    /// Stack of template insertion modes (WHATWG HTML5 §13.2.4.2).
    template_insertion_modes: Vec<InsertionMode>,
    /// Original insertion mode (e.g. for text / select).
    #[allow(dead_code)]
    original_insertion_mode: Option<InsertionMode>,
    /// Flag indicating whether foster parenting is currently active.
    foster_parenting: bool,
    /// Frameset-ok flag (§13.2.4.1). Initialized to true, set to false on non-whitespace text / elements.
    frameset_ok: bool,
}

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

    fn flush_text_buffer(&mut self) {
        if self.text_buffer.is_empty() {
            return;
        }

        let text = std::mem::take(&mut self.text_buffer);

        let is_in_head_container = self.mode == InsertionMode::InHead
            && self
                .current_tag_name()
                .map(|t| t == "title" || t == "style" || t == "script")
                .unwrap_or(false);

        // In Initial, BeforeHtml, BeforeHead, AfterHead, and InHead (outside text container) modes, ignore whitespace text
        if (self.mode == InsertionMode::Initial
            || self.mode == InsertionMode::BeforeHtml
            || self.mode == InsertionMode::BeforeHead
            || self.mode == InsertionMode::AfterHead
            || (self.mode == InsertionMode::InHead && !is_in_head_container))
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

    fn find_enclosing_table(&self) -> Option<NodeId> {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id) {
                if let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case("table") {
                        return Some(node_id);
                    }
                }
            }
        }
        None
    }

    fn current_node(&self) -> NodeId {
        *self.open_elements.last().unwrap_or(&self.doc.root())
    }

    #[allow(dead_code)]
    fn current_tag_name(&self) -> Option<String> {
        let node_id = *self.open_elements.last()?;
        let node = self.doc.get(node_id)?;
        if let NodeData::Element(el) = &node.data {
            Some(el.tag_name.clone())
        } else {
            None
        }
    }

    fn target_node_for_insertion(&self) -> NodeId {
        let curr = self.current_node();
        if let Some(node) = self.doc.get(curr) {
            if let NodeData::Element(el) = &node.data {
                if el.tag_name.eq_ignore_ascii_case("template") {
                    if let Some(frag_id) = el.template_contents {
                        return frag_id;
                    }
                }
            }
        }
        curr
    }

    fn appropriate_insertion_location(&self) -> (NodeId, Option<NodeId>) {
        if self.foster_parenting {
            if let Some(table_id) = self.find_enclosing_table() {
                if let Some(table_node) = self.doc.get(table_id) {
                    if let Some(parent) = table_node.parent {
                        return (parent, Some(table_id));
                    }
                }
                if let Some(pos) = self.open_elements.iter().rposition(|&id| id == table_id) {
                    if pos > 0 {
                        return (self.open_elements[pos - 1], None);
                    }
                }
            }
        }

        let target = self.target_node_for_insertion();
        (target, None)
    }

    fn insert_element(&mut self, name: &str, attributes: Vec<(String, String)>) -> NodeId {
        if name.eq_ignore_ascii_case("meta") {
            self.process_meta_attributes(&attributes);
        }
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_element(name, attributes);
        if let Some(before_id) = before {
            self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    fn process_meta_attributes(&mut self, attributes: &[(String, String)]) {
        // 1. Check for charset="..."
        for (k, v) in attributes {
            if k.eq_ignore_ascii_case("charset") {
                if let Some(canon) = canonical_charset(v) {
                    self.doc.character_set = canon.to_string();
                    return;
                }
            }
        }

        // 2. Check for http-equiv="Content-Type" content="...charset=..."
        let is_http_equiv_ct = attributes.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("http-equiv") && v.trim().eq_ignore_ascii_case("content-type")
        });
        if is_http_equiv_ct {
            if let Some((_, content)) = attributes.iter().find(|(k, _)| k.eq_ignore_ascii_case("content")) {
                if let Some(cs) = extract_charset_from_content(content) {
                    if let Some(canon) = canonical_charset(&cs) {
                        self.doc.character_set = canon.to_string();
                    }
                }
            }
        }
    }

    fn insert_text(&mut self, text: &str) -> NodeId {
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_text(text);
        if let Some(before_id) = before {
            self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    fn insert_comment(&mut self, comment: &str) -> NodeId {
        let (parent, before) = self.appropriate_insertion_location();
        let child = self.doc.create_comment(comment);
        if let Some(before_id) = before {
            self.doc.insert_before(parent, child, before_id);
        } else {
            self.doc.append_child(parent, child);
        }
        child
    }

    fn pop_until(&mut self, tag_name: &str) {
        while let Some(&node_id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data
                    && el.tag_name.eq_ignore_ascii_case(tag_name) {
                        self.open_elements.pop();
                        break;
                    }
            self.open_elements.pop();
        }
    }

    fn is_in_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case(tag_name) {
                        return true;
                    }
                    if matches!(
                        el.tag_name.as_str(),
                        "applet" | "caption" | "html" | "table" | "td" | "th" | "marquee" | "object" | "template" | "body"
                    ) {
                        return false;
                    }
                }
        }
        false
    }

    fn is_in_table_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data {
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

    fn is_in_select_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data {
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

    fn is_in_button_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case(tag_name) {
                        return true;
                    }
                    if matches!(
                        el.tag_name.as_str(),
                        "applet" | "caption" | "html" | "table" | "td" | "th" | "marquee" | "object" | "template" | "body" | "button"
                    ) {
                        return false;
                    }
                }
        }
        false
    }

    fn is_in_list_item_scope(&self, tag_name: &str) -> bool {
        for &node_id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(node_id)
                && let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case(tag_name) {
                        return true;
                    }
                    if matches!(
                        el.tag_name.as_str(),
                        "applet" | "caption" | "html" | "table" | "td" | "th" | "marquee" | "object" | "template" | "body" | "ol" | "ul"
                    ) {
                        return false;
                    }
                }
        }
        false
    }

    fn clear_stack_back_to_table_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "table" | "template" | "html") {
                        break;
                    }
                }
            }
            self.open_elements.pop();
        }
    }

    fn clear_stack_back_to_table_body_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "tbody" | "thead" | "tfoot" | "template" | "html") {
                        break;
                    }
                }
            }
            self.open_elements.pop();
        }
    }

    fn clear_stack_back_to_table_row_context(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "tr" | "template" | "html") {
                        break;
                    }
                }
            }
            self.open_elements.pop();
        }
    }

    fn close_cell(&mut self) {
        self.generate_implied_end_tags(None);
        while let Some(top) = self.open_elements.pop() {
            if let Some(node) = self.doc.get(top) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "td" | "th") {
                        break;
                    }
                }
            }
        }
        self.clear_active_formatting_to_marker();
        self.mode = InsertionMode::InRow;
    }

    fn generate_implied_end_tags(&mut self, exclude: Option<&str>) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
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
            }
            break;
        }
    }

    fn reset_insertion_mode(&mut self) {
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
            let Some(node) = self.doc.get(node_id) else { continue };
            let NodeData::Element(el) = &node.data else { continue };
            let tag = el.tag_name.as_str();

            match tag {
                "select" => {
                    let mut in_table = false;
                    for &ancestor_id in self.open_elements[..i].iter().rev() {
                        if let Some(a_node) = self.doc.get(ancestor_id) {
                            if let NodeData::Element(a_el) = &a_node.data {
                                if matches!(a_el.tag_name.as_str(), "table" | "tbody" | "thead" | "tfoot" | "tr" | "td" | "th") {
                                    in_table = true;
                                    break;
                                }
                            }
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

    fn has_heading_in_scope(&self) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                        return true;
                    }
                    if matches!(el.tag_name.as_str(), "html" | "table" | "template" | "body") {
                        return false;
                    }
                }
            }
        }
        false
    }

    fn pop_heading_in_scope(&mut self) {
        while let Some(&id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if matches!(el.tag_name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                        self.open_elements.pop();
                        break;
                    }
                }
            }
            self.open_elements.pop();
        }
    }

    /// Reconstructs the active formatting elements according to WHATWG HTML5 §13.2.6.4.
    fn reconstruct_active_formatting_elements(&mut self) {
        if self.active_formatting_elements.is_empty() {
            return;
        }

        // 1. If the last entry is a marker or already in open_elements, return.
        match self.active_formatting_elements.last().unwrap() {
            FormattingEntry::Marker => return,
            FormattingEntry::Element { node_id, .. } => {
                if self.open_elements.contains(node_id) {
                    return;
                }
            }
        }

        // 2. Find the last entry before the end that is either a marker or in open_elements.
        let mut start_idx = self.active_formatting_elements.len() - 1;
        while start_idx > 0 {
            let prev = &self.active_formatting_elements[start_idx - 1];
            match prev {
                FormattingEntry::Marker => break,
                FormattingEntry::Element { node_id, .. } => {
                    if self.open_elements.contains(node_id) {
                        break;
                    }
                }
            }
            start_idx -= 1;
        }

        // 3. Reconstruct forwards from start_idx
        for i in start_idx..self.active_formatting_elements.len() {
            let (tag_name, attributes) = match &self.active_formatting_elements[i] {
                FormattingEntry::Marker => continue,
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    ..
                } => (tag_name.clone(), attributes.clone()),
            };

            let parent = self.current_node();
            let new_child = self.doc.create_element(&tag_name, attributes.clone());
            self.doc.append_child(parent, new_child);
            self.open_elements.push(new_child);

            self.active_formatting_elements[i] = FormattingEntry::Element {
                tag_name,
                attributes,
                node_id: new_child,
            };
        }
    }

    /// Pushes an element to the active formatting elements list, enforcing WHATWG "Noah's Ark" (rule of three).
    fn push_active_formatting_element(
        &mut self,
        tag_name: &str,
        attributes: &[(String, String)],
        node_id: NodeId,
    ) {
        let mut count = 0;
        let mut earliest_idx = None;
        for (i, entry) in self.active_formatting_elements.iter().enumerate().rev() {
            match entry {
                FormattingEntry::Marker => break,
                FormattingEntry::Element {
                    tag_name: t,
                    attributes: a,
                    ..
                } => {
                    if t.eq_ignore_ascii_case(tag_name) && a == attributes {
                        count += 1;
                        earliest_idx = Some(i);
                    }
                }
            }
        }

        if count >= 3 && let Some(idx) = earliest_idx {
            self.active_formatting_elements.remove(idx);
        }

        self.active_formatting_elements.push(FormattingEntry::Element {
            tag_name: tag_name.to_string(),
            attributes: attributes.to_vec(),
            node_id,
        });
    }

    /// Clears active formatting elements up to the last marker (or until empty if no marker).
    fn clear_active_formatting_to_marker(&mut self) {
        while let Some(entry) = self.active_formatting_elements.pop() {
            if matches!(entry, FormattingEntry::Marker) {
                break;
            }
        }
    }

    /// Executes the WHATWG Adoption Agency Algorithm (AAA) for formatting end tags.
    fn handle_formatting_end_tag(&mut self, subject: &str) {
        // If the current node is the target tag and not in active formatting elements, pop and return.
        if let Some(curr_tag) = self.current_tag_name() {
            if curr_tag.eq_ignore_ascii_case(subject) {
                let curr_id = self.current_node();
                let in_active = self.active_formatting_elements.iter().any(|entry| {
                    matches!(entry, FormattingEntry::Element { node_id, .. } if *node_id == curr_id)
                });
                if !in_active {
                    self.open_elements.pop();
                    return;
                }
            }
        }

        // Outer loop: up to 8 iterations per WHATWG HTML5 §13.2.6.4.7
        for _ in 0..8 {
            // Step 1: Find formatting_element: the last element in active_formatting_elements
            // between the end and the last marker that has tag name == subject.
            let fmt_pos = self.active_formatting_elements.iter().rposition(|entry| {
                match entry {
                    FormattingEntry::Marker => false,
                    FormattingEntry::Element { tag_name, .. } => tag_name.eq_ignore_ascii_case(subject),
                }
            });

            let fmt_idx = match fmt_pos {
                Some(idx) => {
                    let has_marker_after = self.active_formatting_elements[idx..]
                        .iter()
                        .any(|e| matches!(e, FormattingEntry::Marker));
                    if has_marker_after {
                        return;
                    }
                    idx
                }
                None => {
                    if self.is_in_scope(subject) {
                        self.pop_until(subject);
                    }
                    return;
                }
            };

            let (fmt_tag, fmt_attrs, fmt_node_id) = match &self.active_formatting_elements[fmt_idx] {
                FormattingEntry::Element {
                    tag_name,
                    attributes,
                    node_id,
                } => (tag_name.clone(), attributes.clone(), *node_id),
                FormattingEntry::Marker => return,
            };

            // Step 2: Check if formatting_element is in open_elements
            let stack_pos = self.open_elements.iter().rposition(|&id| id == fmt_node_id);
            let stack_idx = match stack_pos {
                Some(idx) => idx,
                None => {
                    self.active_formatting_elements.remove(fmt_idx);
                    return;
                }
            };

            // Step 3: Check if in scope
            if !self.is_in_scope(&fmt_tag) {
                return;
            }

            // Step 4: Find furthest block among open elements after fmt_node_id
            let mut furthest_block = None;
            for &id in &self.open_elements[stack_idx + 1..] {
                if let Some(node) = self.doc.get(id) {
                    if let NodeData::Element(el) = &node.data {
                        if is_special_element(&el.tag_name) {
                            furthest_block = Some(id);
                            break;
                        }
                    }
                }
            }

            // Step 5: If no furthest block, pop up to fmt_node_id and return
            if furthest_block.is_none() {
                while let Some(top) = self.open_elements.pop() {
                    if top == fmt_node_id {
                        break;
                    }
                }
                if fmt_idx < self.active_formatting_elements.len() {
                    self.active_formatting_elements.remove(fmt_idx);
                }
                return;
            }

            // Step 6: There IS a furthest block
            let fb_id = furthest_block.unwrap();
            let common_ancestor = if stack_idx > 0 {
                self.open_elements[stack_idx - 1]
            } else {
                self.doc.root()
            };

            let mut node;
            let mut last_node = fb_id;

            let fb_stack_idx = self.open_elements.iter().rposition(|&id| id == fb_id).unwrap();
            let mut curr_stack_idx = fb_stack_idx;

            for _ in 0..3 {
                if curr_stack_idx == 0 {
                    break;
                }
                curr_stack_idx -= 1;
                node = self.open_elements[curr_stack_idx];

                if node == fmt_node_id {
                    break;
                }

                let active_idx = self.active_formatting_elements.iter().position(|e| {
                    matches!(e, FormattingEntry::Element { node_id, .. } if *node_id == node)
                });

                let active_idx = match active_idx {
                    Some(idx) => idx,
                    None => {
                        self.open_elements.retain(|&id| id != node);
                        continue;
                    }
                };

                let (node_tag, node_attrs) = match &self.active_formatting_elements[active_idx] {
                    FormattingEntry::Element {
                        tag_name,
                        attributes,
                        ..
                    } => (tag_name.clone(), attributes.clone()),
                    FormattingEntry::Marker => continue,
                };

                let clone_id = self.doc.create_element(&node_tag, node_attrs.clone());
                self.active_formatting_elements[active_idx] = FormattingEntry::Element {
                    tag_name: node_tag,
                    attributes: node_attrs,
                    node_id: clone_id,
                };
                self.open_elements[curr_stack_idx] = clone_id;

                self.doc.append_child(clone_id, last_node);
                last_node = clone_id;
            }

            // Insert last_node into common_ancestor after fmt_node_id
            self.doc.insert_after(common_ancestor, last_node, fmt_node_id);

            // Create new element for formatting_element
            let new_fmt_elem = self.doc.create_element(&fmt_tag, fmt_attrs);

            // Move all existing children of furthest_block into new_fmt_elem
            let fb_children: Vec<NodeId> = self.doc.children(fb_id).map(|c| c.id).collect();
            for child_id in fb_children {
                self.doc.append_child(new_fmt_elem, child_id);
            }
            self.doc.append_child(fb_id, new_fmt_elem);

            // Remove old formatting_element from active_formatting_elements and open_elements
            self.active_formatting_elements.retain(|e| {
                !matches!(e, FormattingEntry::Element { node_id, .. } if *node_id == fmt_node_id)
            });
            self.open_elements.retain(|&id| id != fmt_node_id);
            return;
        }
    }

    fn ensure_html(&mut self) -> NodeId {
        if self.open_elements.is_empty() {
            let html = self.insert_element("html", Vec::new());
            self.open_elements.push(html);
            html
        } else {
            self.open_elements[0]
        }
    }

    fn ensure_head(&mut self) -> NodeId {
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

    fn ensure_body(&mut self) -> NodeId {
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
                    && el.tag_name == "body" {
                        self.mode = InsertionMode::InBody;
                        return id;
                    }
        }

        let body = self.insert_element("body", Vec::new());
        self.open_elements.push(body);
        self.mode = InsertionMode::InBody;
        body
    }

    fn is_current_node_foreign(&self) -> bool {
        if let Some(&node_id) = self.open_elements.last() {
            if let Some(node) = self.doc.get(node_id) {
                if let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case("foreignObject") {
                        return false;
                    }
                    return self.is_inside_foreign_content();
                }
            }
        }
        false
    }

    fn is_inside_foreign_content(&self) -> bool {
        for &id in self.open_elements.iter().rev() {
            if let Some(node) = self.doc.get(id) {
                if let NodeData::Element(el) = &node.data {
                    if el.tag_name.eq_ignore_ascii_case("foreignObject") {
                        return false;
                    }
                    if el.tag_name.eq_ignore_ascii_case("svg") || el.tag_name.eq_ignore_ascii_case("math") {
                        return true;
                    }
                    if matches!(el.tag_name.as_str(), "html" | "body" | "table") {
                        return false;
                    }
                }
            }
        }
        false
    }

    fn process_token_foreign(&mut self, token: Token) -> bool {
        match token {
            Token::Character(c) => {
                self.text_buffer.push(c);
                true
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
                true
            }
            Token::StartTag {
                name,
                mut attributes,
                self_closing,
            } => {
                if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "b" | "big"
                        | "blockquote"
                        | "body"
                        | "br"
                        | "center"
                        | "code"
                        | "dd"
                        | "div"
                        | "dl"
                        | "dt"
                        | "em"
                        | "embed"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "head"
                        | "hr"
                        | "i"
                        | "img"
                        | "li"
                        | "menu"
                        | "meta"
                        | "nobr"
                        | "ol"
                        | "p"
                        | "pre"
                        | "ruby"
                        | "s"
                        | "small"
                        | "span"
                        | "strong"
                        | "strike"
                        | "sub"
                        | "sup"
                        | "table"
                        | "tt"
                        | "u"
                        | "ul"
                        | "var"
                ) {
                    while let Some(&top_id) = self.open_elements.last() {
                        if let Some(node) = self.doc.get(top_id) {
                            if let NodeData::Element(el) = &node.data {
                                if el.tag_name.eq_ignore_ascii_case("svg") || el.tag_name.eq_ignore_ascii_case("math") {
                                    self.open_elements.pop();
                                    break;
                                }
                            }
                        }
                        self.open_elements.pop();
                    }
                    return false;
                }

                let adjusted_name = adjust_svg_tag_name(&name);
                adjust_svg_attributes(&mut attributes);
                let elem = self.insert_element(&adjusted_name, attributes);
                if !self_closing {
                    self.open_elements.push(elem);
                }
                true
            }
            Token::EndTag { name } => {
                let name_lower = name.to_ascii_lowercase();
                let mut found = false;
                for &id in self.open_elements.iter().rev() {
                    if let Some(node) = self.doc.get(id) {
                        if let NodeData::Element(el) = &node.data {
                            if el.tag_name.eq_ignore_ascii_case(&name_lower) {
                                found = true;
                                break;
                            }
                            if el.tag_name.eq_ignore_ascii_case("svg") || el.tag_name.eq_ignore_ascii_case("math") {
                                break;
                            }
                        }
                    }
                }
                if found {
                    while let Some(top_id) = self.open_elements.pop() {
                        if let Some(node) = self.doc.get(top_id) {
                            if let NodeData::Element(el) = &node.data {
                                if el.tag_name.eq_ignore_ascii_case(&name_lower) {
                                    break;
                                }
                            }
                        }
                    }
                    if name_lower == "svg" || name_lower == "math" {
                        self.reset_insertion_mode();
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn process_token_initial(&mut self, token: Token) {
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
                    || (pub_lower.starts_with("-//w3c//dtd html 4.0 transitional//") && sys_id.is_empty())
                    || (pub_lower.starts_with("-//w3c//dtd html 4.01 transitional//") && sys_id.is_empty())
                {
                    self.doc.quirks_mode = QuirksMode::Quirks;
                } else if pub_lower.starts_with("-//w3c//dtd xhtml 1.0 frameset//")
                    || pub_lower.starts_with("-//w3c//dtd xhtml 1.0 transitional//")
                    || (pub_lower.starts_with("-//w3c//dtd html 4.0 transitional//") && !sys_id.is_empty())
                    || (pub_lower.starts_with("-//w3c//dtd html 4.01 transitional//") && !sys_id.is_empty())
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

    fn process_token_before_html(&mut self, token: Token) {
        match token {
            Token::StartTag { name, attributes, .. } if name == "html" => {
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

    fn process_token_before_head(&mut self, token: Token) {
        match token {
            Token::StartTag { name, attributes, .. } if name == "head" => {
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

    fn process_token_in_head(&mut self, token: Token) {
        match token {
            Token::StartTag { name, attributes, .. } if name == "title" || name == "style" || name == "script" => {
                self.flush_text_buffer();
                let is_raw = is_raw_text_element(&name) || is_escapable_raw_text_element(&name);
                let elem = self.insert_element(&name, attributes);
                self.open_elements.push(elem);
                if is_raw {
                    self.tokenizer.set_raw_text_mode(&name);
                }
            }
            Token::StartTag { name, attributes, .. } if name == "template" => {
                self.flush_text_buffer();
                let elem = self.insert_element(&name, attributes);
                self.open_elements.push(elem);
                self.active_formatting_elements.push(FormattingEntry::Marker);
                self.template_insertion_modes.push(InsertionMode::InTemplate);
                self.mode = InsertionMode::InTemplate;
            }
            Token::StartTag { name, attributes, .. } if is_void_element(&name) => {
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
                self.flush_text_buffer();
                if self.open_elements.iter().any(|&id| {
                    self.doc.get(id).map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template")).unwrap_or(false)
                }) {
                    self.generate_implied_end_tags(None);
                    self.pop_until("template");
                    self.clear_active_formatting_to_marker();
                    self.template_insertion_modes.pop();
                    self.reset_insertion_mode();
                }
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

    fn process_token_after_head(&mut self, token: Token) {
        match token {
            Token::Character(c) if c.is_ascii_whitespace() => {}
            Token::StartTag { name, attributes, .. } if name == "body" => {
                let body = self.insert_element(&name, attributes);
                self.open_elements.push(body);
                self.mode = InsertionMode::InBody;
            }
            Token::StartTag { name, attributes, .. } if name == "frameset" => {
                let frameset = self.insert_element(&name, attributes);
                self.open_elements.push(frameset);
                self.mode = InsertionMode::InFrameset;
            }
            Token::StartTag { name, attributes, .. } if name == "template" => {
                let elem = self.insert_element(&name, attributes);
                self.open_elements.push(elem);
                self.active_formatting_elements.push(FormattingEntry::Marker);
                self.template_insertion_modes.push(InsertionMode::InTemplate);
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

    fn process_token_in_body(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "body" {
                    for &id in &self.open_elements {
                        if let Some(node) = self.doc.get_mut(id) {
                            if let NodeData::Element(el) = &mut node.data {
                                if el.tag_name == "body" {
                                    for (attr_name, attr_val) in attributes {
                                        if !el.attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case(&attr_name)) {
                                            el.attributes.push((attr_name, attr_val));
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    return;
                }

                if name == "frameset" && self.frameset_ok {
                    while let Some(top) = self.open_elements.last() {
                        if let Some(node) = self.doc.get(*top) {
                            if let NodeData::Element(el) = &node.data {
                                if el.tag_name == "body" {
                                    self.open_elements.pop();
                                    break;
                                }
                            }
                        }
                        self.open_elements.pop();
                    }
                    let frameset = self.insert_element(&name, attributes);
                    self.open_elements.push(frameset);
                    self.mode = InsertionMode::InFrameset;
                    return;
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
                    self.active_formatting_elements.push(FormattingEntry::Marker);
                    self.mode = InsertionMode::InTable;
                    return;
                }

                if name == "select" {
                    self.reconstruct_active_formatting_elements();
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.active_formatting_elements.push(FormattingEntry::Marker);
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
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.active_formatting_elements.push(FormattingEntry::Marker);
                    self.template_insertion_modes.push(InsertionMode::InTemplate);
                    self.mode = InsertionMode::InTemplate;
                    return;
                }

                if is_active_formatting_element(&name) {
                    if name == "a" {
                        let has_a = self.active_formatting_elements.iter().rposition(|e| {
                            match e {
                                FormattingEntry::Marker => false,
                                FormattingEntry::Element { tag_name, .. } => tag_name.eq_ignore_ascii_case("a"),
                            }
                        });
                        if let Some(idx) = has_a {
                            self.handle_formatting_end_tag("a");
                            if idx < self.active_formatting_elements.len() {
                                if let FormattingEntry::Element { node_id, .. } = self.active_formatting_elements[idx] {
                                    self.open_elements.retain(|&id| id != node_id);
                                    self.active_formatting_elements.remove(idx);
                                }
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
                    "span" | "sub" | "sup" | "mark" | "img" | "br" | "input" | "time" | "abbr" | "cite"
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

                if matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") && self.has_heading_in_scope() {
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
                    if let Some(curr) = self.current_tag_name() && curr == "option" {
                        self.open_elements.pop();
                    }
                    if let Some(curr) = self.current_tag_name() && curr == "optgroup" {
                        self.open_elements.pop();
                    }
                }

                if name == "option" {
                    if let Some(curr) = self.current_tag_name() && curr == "option" {
                        self.open_elements.pop();
                    }
                }

                let is_foreign_self_closing = self_closing && matches!(name.to_ascii_lowercase().as_str(), "path" | "circle" | "rect" | "line" | "polygon" | "polyline" | "ellipse" | "use" | "stop");
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
                    if self.open_elements.iter().any(|&id| {
                        self.doc.get(id).map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template")).unwrap_or(false)
                    }) {
                        self.generate_implied_end_tags(None);
                        self.pop_until("template");
                        self.clear_active_formatting_to_marker();
                        self.template_insertion_modes.pop();
                        self.reset_insertion_mode();
                    }
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

    fn process_token_in_table(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if name == "caption" {
                    self.clear_stack_back_to_table_context();
                    self.active_formatting_elements.push(FormattingEntry::Marker);
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
                    let is_hidden = attributes
                        .iter()
                        .any(|(k, v)| k.eq_ignore_ascii_case("type") && v.eq_ignore_ascii_case("hidden"));
                    if is_hidden {
                        self.insert_element(&name, attributes);
                        return;
                    }
                }
                if name == "template" {
                    let elem = self.insert_element(&name, attributes);
                    self.open_elements.push(elem);
                    self.active_formatting_elements.push(FormattingEntry::Marker);
                    self.template_insertion_modes.push(InsertionMode::InTemplate);
                    self.mode = InsertionMode::InTemplate;
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
                    "body" | "caption" | "col" | "colgroup" | "html" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr"
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

    fn process_token_in_table_body(&mut self, token: Token) {
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

    fn process_token_in_row(&mut self, token: Token) {
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
                    self.active_formatting_elements.push(FormattingEntry::Marker);
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
                if matches!(name.as_str(), "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th") {
                    return;
                }
                self.process_token_in_table(Token::EndTag { name });
            }
            _ => {
                self.process_token_in_table(token);
            }
        }
    }

    fn process_token_in_cell(&mut self, token: Token) {
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
                if matches!(name.as_str(), "body" | "caption" | "col" | "colgroup" | "html") {
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
                    "caption" | "col" | "colgroup" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr"
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

    fn process_token_in_caption(&mut self, token: Token) {
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
                    "body" | "col" | "colgroup" | "html" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr"
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
                    "caption" | "col" | "colgroup" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr"
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

    fn process_token_in_column_group(&mut self, token: Token) {
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

    fn process_token_in_select(&mut self, token: Token) {
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

    fn process_token_in_select_in_table(&mut self, token: Token) {
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

    fn process_token_in_template(&mut self, token: Token) {
        match token {
            Token::StartTag {
                name,
                attributes,
                self_closing,
            } => {
                if matches!(
                    name.as_str(),
                    "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script" | "style" | "template" | "title"
                ) {
                    self.process_token_in_head(Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                    });
                    return;
                }
                if matches!(name.as_str(), "caption" | "colgroup" | "tbody" | "tfoot" | "thead") {
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
                    self.template_insertion_modes.push(InsertionMode::InColumnGroup);
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
                    self.template_insertion_modes.push(InsertionMode::InTableBody);
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
                    if self.open_elements.iter().any(|&id| {
                        self.doc.get(id).map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template")).unwrap_or(false)
                    }) {
                        self.generate_implied_end_tags(None);
                        self.pop_until("template");
                        self.clear_active_formatting_to_marker();
                        self.template_insertion_modes.pop();
                        self.reset_insertion_mode();
                    }
                }
            }
            Token::Character(c) => {
                self.template_insertion_modes.pop();
                self.template_insertion_modes.push(InsertionMode::InBody);
                self.mode = InsertionMode::InBody;
                self.process_token(Token::Character(c));
            }
            Token::Comment(c) => {
                self.insert_comment(&c);
            }
            Token::EndOfFile => {
                if self.open_elements.iter().any(|&id| {
                    self.doc.get(id).map(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "template")).unwrap_or(false)
                }) {
                    self.pop_until("template");
                    self.clear_active_formatting_to_marker();
                    self.template_insertion_modes.pop();
                    self.reset_insertion_mode();
                    self.process_token(Token::EndOfFile);
                }
            }
            _ => {}
        }
    }

    fn process_token_in_frameset(&mut self, token: Token) {
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
            Token::EndTag { name } if name == "frameset" => {
                if self.current_tag_name().as_deref() != Some("html") {
                    self.open_elements.pop();
                    if self.current_tag_name().as_deref() != Some("frameset") {
                        self.mode = InsertionMode::AfterFrameset;
                    }
                }
            }
            _ => {}
        }
    }

    fn process_token_after_frameset(&mut self, token: Token) {
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

    fn process_token_after_body(&mut self, token: Token) {
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

    fn process_token_after_after_body(&mut self, token: Token) {
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

    fn process_token(&mut self, token: Token) {
        if self.is_current_node_foreign() && self.process_token_foreign(token.clone()) {
            return;
        }

        match self.mode {
            InsertionMode::Initial => self.process_token_initial(token),
            InsertionMode::BeforeHtml => self.process_token_before_html(token),
            InsertionMode::BeforeHead => self.process_token_before_head(token),
            InsertionMode::InHead => self.process_token_in_head(token),
            InsertionMode::AfterHead => self.process_token_after_head(token),
            InsertionMode::InBody => self.process_token_in_body(token),
            InsertionMode::InTable => self.process_token_in_table(token),
            InsertionMode::InTableText => self.process_token_in_table(token),
            InsertionMode::InCaption => self.process_token_in_caption(token),
            InsertionMode::InColumnGroup => self.process_token_in_column_group(token),
            InsertionMode::InTableBody => self.process_token_in_table_body(token),
            InsertionMode::InRow => self.process_token_in_row(token),
            InsertionMode::InCell => self.process_token_in_cell(token),
            InsertionMode::InSelect => self.process_token_in_select(token),
            InsertionMode::InSelectInTable => self.process_token_in_select_in_table(token),
            InsertionMode::InTemplate => self.process_token_in_template(token),
            InsertionMode::InFrameset => self.process_token_in_frameset(token),
            InsertionMode::AfterFrameset => self.process_token_after_frameset(token),
            InsertionMode::AfterBody => self.process_token_after_body(token),
            InsertionMode::AfterAfterBody => self.process_token_after_after_body(token),
        }
    }
}

/// Adjusts SVG tag name case per WHATWG HTML §13.2.6.2.
fn adjust_svg_tag_name(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "altglyph" => "altGlyph".to_string(),
        "altglyphdef" => "altGlyphDef".to_string(),
        "altglyphitem" => "altGlyphItem".to_string(),
        "animatecolor" => "animateColor".to_string(),
        "animatemotion" => "animateMotion".to_string(),
        "animatetransform" => "animateTransform".to_string(),
        "clippath" => "clipPath".to_string(),
        "feblend" => "feBlend".to_string(),
        "fecolormatrix" => "feColorMatrix".to_string(),
        "fecomponenttransfer" => "feComponentTransfer".to_string(),
        "fecomposite" => "feComposite".to_string(),
        "feconvolvematrix" => "feConvolveMatrix".to_string(),
        "fediffuselighting" => "feDiffuseLighting".to_string(),
        "fedisplacementmap" => "feDisplacementMap".to_string(),
        "fedistantlight" => "feDistantLight".to_string(),
        "fedropshadow" => "feDropShadow".to_string(),
        "feflood" => "feFlood".to_string(),
        "fefunca" => "feFuncA".to_string(),
        "fefuncb" => "feFuncB".to_string(),
        "fefuncg" => "feFuncG".to_string(),
        "fefuncr" => "feFuncR".to_string(),
        "fegaussianblur" => "feGaussianBlur".to_string(),
        "feimage" => "feImage".to_string(),
        "femerge" => "feMerge".to_string(),
        "femergenode" => "feMergeNode".to_string(),
        "femorphology" => "feMorphology".to_string(),
        "feoffset" => "feOffset".to_string(),
        "fepointlight" => "fePointLight".to_string(),
        "fespecularlighting" => "feSpecularLighting".to_string(),
        "fespotlight" => "feSpotLight".to_string(),
        "fetile" => "feTile".to_string(),
        "feturbulence" => "feTurbulence".to_string(),
        "foreignobject" => "foreignObject".to_string(),
        "glyphref" => "glyphRef".to_string(),
        "lineargradient" => "linearGradient".to_string(),
        "radialgradient" => "radialGradient".to_string(),
        "textpath" => "textPath".to_string(),
        _ => name.to_string(),
    }
}

/// Adjusts SVG attribute name case per WHATWG HTML §13.2.6.2.
fn adjust_svg_attributes(attributes: &mut [(String, String)]) {
    for (key, _) in attributes.iter_mut() {
        let replacement = match key.to_ascii_lowercase().as_str() {
            "attributename" => "attributeName",
            "attributetype" => "attributeType",
            "basefrequency" => "baseFrequency",
            "baseprofile" => "baseProfile",
            "calcmode" => "calcMode",
            "clippathunits" => "clipPathUnits",
            "diffuseconstant" => "diffuseConstant",
            "edgemode" => "edgeMode",
            "filterunits" => "filterUnits",
            "glyphref" => "glyphRef",
            "gradienttransform" => "gradientTransform",
            "gradientunits" => "gradientUnits",
            "kernelmatrix" => "kernelMatrix",
            "kernelunitlength" => "kernelUnitLength",
            "keypoints" => "keyPoints",
            "keysplines" => "keySplines",
            "keytimes" => "keyTimes",
            "lengthadjust" => "lengthAdjust",
            "limitingconeangle" => "limitingConeAngle",
            "markerheight" => "markerHeight",
            "markerunits" => "markerUnits",
            "markerwidth" => "markerWidth",
            "maskcontentunits" => "maskContentUnits",
            "maskunits" => "maskUnits",
            "numoctaves" => "numOctaves",
            "pathlength" => "pathLength",
            "patterncontentunits" => "patternContentUnits",
            "patterntransform" => "patternTransform",
            "patternunits" => "patternUnits",
            "pointsatx" => "pointsAtX",
            "pointsaty" => "pointsAtY",
            "pointsatz" => "pointsAtZ",
            "preservealpha" => "preserveAlpha",
            "preserveaspectratio" => "preserveAspectRatio",
            "primitiveunits" => "primitiveUnits",
            "refx" => "refX",
            "refy" => "refY",
            "repeatcount" => "repeatCount",
            "repeatdur" => "repeatDur",
            "requiredextensions" => "requiredExtensions",
            "requiredfeatures" => "requiredFeatures",
            "specularconstant" => "specularConstant",
            "specularexponent" => "specularExponent",
            "spreadmethod" => "spreadMethod",
            "startoffset" => "startOffset",
            "stddeviation" => "stdDeviation",
            "stitchtiles" => "stitchTiles",
            "surfacescale" => "surfaceScale",
            "systemlanguage" => "systemLanguage",
            "tablevalues" => "tableValues",
            "targetx" => "targetX",
            "targety" => "targetY",
            "textlength" => "textLength",
            "viewbox" => "viewBox",
            "viewtarget" => "viewTarget",
            "xchannelselector" => "xChannelSelector",
            "ychannelselector" => "yChannelSelector",
            "zoomandpan" => "zoomAndPan",
            _ => continue,
        };
        *key = replacement.to_string();
    }
}

/// Extracts the charset parameter from a `Content-Type` style header value.
fn extract_charset_from_content(content: &str) -> Option<String> {
    for part in content.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            if k.trim().eq_ignore_ascii_case("charset") {
                let val = v.trim().trim_matches(['"', '\'']).trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Maps standard charset labels and aliases to their WHATWG canonical encoding name.
fn canonical_charset(label: &str) -> Option<&'static str> {
    let normalized = label.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    let name = normalized.split([';', ',']).next().unwrap_or("").trim();
    match name {
        "utf-8" | "utf8" | "unicode-1-1-utf-8" | "unicode11utf8" | "unicode20utf8"
        | "x-unicode20utf8" | "csunicode11utf8" | "utf_8" => Some("UTF-8"),
        "utf-16" | "utf-16le" | "csunicode" | "iso-10646-ucs-2" | "ucs-2" | "unicode"
        | "unicodefeff" => Some("UTF-16LE"),
        "utf-16be" | "unicodefffe" => Some("UTF-16BE"),
        "windows-1252" | "cp1252" | "x-cp1252"
        | "iso-8859-1" | "iso8859-1" | "iso_8859-1" | "iso_8859-1:1987" | "iso-ir-100"
        | "latin1" | "latin-1" | "l1" | "csisolatin1"
        | "us-ascii" | "ascii" | "ansi_x3.4-1968" | "iso-ir-6" | "ansi_x3.4-1986"
        | "iso_646.irv:1991" | "iso646-us" | "us" | "ibm367" | "cp367" | "csascii" => {
            Some("windows-1252")
        }
        _ => None,
    }
}

/// Convenience function: parses raw HTML string into a [`Document`].
pub fn parse_html(html: &str) -> Document {
    TreeBuilder::parse(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_page() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Hello Mango</title>
</head>
<body>
    <h1 id="headline">Mango Browser</h1>
    <p class="intro">Ultra-lightweight browser in Rust.</p>
</body>
</html>"#;

        let doc = parse_html(html);
        let root = doc.root();

        // Check for h1 element
        let h1_id = doc.find_element_by_tag(root, "h1").expect("h1 found");
        let h1_node = doc.get(h1_id).unwrap();
        if let NodeData::Element(el) = &h1_node.data {
            assert_eq!(el.id(), Some("headline"));
        } else {
            panic!("expected element");
        }
        assert_eq!(doc.text_content(h1_id), "Mango Browser");

        // Check for p element
        let p_id = doc.find_element_by_tag(root, "p").expect("p found");
        assert_eq!(doc.text_content(p_id), "Ultra-lightweight browser in Rust.");
    }

    #[test]
    fn test_parse_missing_tags_recovery() {
        // Missing <!DOCTYPE>, <html>, <head>, and <body> tags
        let html = "<h1>Auto Recovered</h1><p>Paragraph 1<p>Paragraph 2";
        let doc = parse_html(html);
        let root = doc.root();

        let h1_id = doc.find_element_by_tag(root, "h1").expect("h1 found");
        assert_eq!(doc.text_content(h1_id), "Auto Recovered");

        // Verify that html and body were created automatically
        assert!(doc.find_element_by_tag(root, "html").is_some());
        assert!(doc.find_element_by_tag(root, "body").is_some());
    }

    #[test]
    fn test_void_elements_do_not_nest() {
        let html = "<div><img src=\"logo.png\"><p>Text after img</p></div>";
        let doc = parse_html(html);
        let root = doc.root();

        let img_id = doc.find_element_by_tag(root, "img").expect("img found");
        let p_id = doc.find_element_by_tag(root, "p").expect("p found");

        // img must have no children
        assert_eq!(doc.children(img_id).count(), 0);
        // p must NOT be a child of img
        let p_node = doc.get(p_id).unwrap();
        assert_ne!(p_node.parent, Some(img_id));
    }

    #[test]
    fn test_implied_closing_dl_dt_dd_and_table() {
        let html = "<dl><dt>Term 1<dd>Def 1<dt>Term 2<dd>Def 2</dl>";
        let doc = parse_html(html);
        let root = doc.root();

        let dl_id = doc.find_element_by_tag(root, "dl").expect("dl found");
        let dl_children: Vec<_> = doc.children(dl_id).collect();
        // dt and dd elements should all be direct children of dl (siblings), not nested inside each other
        assert_eq!(dl_children.len(), 4);

        // Table implied closing without closing tags and implicit tbody creation
        let table_html = "<table>Foster Text<tr><td>Cell 1<td>Cell 2<tr><td>Cell 3<td>Cell 4</table>";
        let table_doc = parse_html(table_html);
        let t_root = table_doc.root();
        let table_id = table_doc.find_element_by_tag(t_root, "table").expect("table found");
        let tbody_id = table_doc.find_element_by_tag(table_id, "tbody").expect("implicit tbody found");
        let trs: Vec<_> = table_doc.children(tbody_id).filter(|node| {
            if let NodeData::Element(el) = &node.data {
                el.tag_name == "tr"
            } else {
                false
            }
        }).collect();
        assert_eq!(trs.len(), 2);

        // Foster parenting: "Foster Text" was moved before <table> in body
        let body_id = table_doc.find_element_by_tag(t_root, "body").expect("body found");
        let body_text = table_doc.text_content(body_id);
        assert!(body_text.contains("Foster Text"));
    }

    #[test]
    fn test_quirks_mode_detection() {
        use crate::dom::QuirksMode;

        // Modern HTML5 DOCTYPE -> NoQuirks
        let doc1 = parse_html("<!DOCTYPE html><html><body></body></html>");
        assert_eq!(doc1.quirks_mode, QuirksMode::NoQuirks);

        // Missing DOCTYPE -> Quirks
        let doc2 = parse_html("<html><body><h1>No doctype</h1></body></html>");
        assert_eq!(doc2.quirks_mode, QuirksMode::Quirks);

        // HTML 4.01 Transitional with System ID -> LimitedQuirks (Almost Standards)
        let doc3 = parse_html("<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\" \"http://www.w3.org/TR/html4/loose.dtd\"><html><body></body></html>");
        assert_eq!(doc3.quirks_mode, QuirksMode::LimitedQuirks);

        // Obsolete permitted legacy-compat -> NoQuirks
        let doc4 = parse_html("<!DOCTYPE html SYSTEM \"about:legacy-compat\"><html><body></body></html>");
        assert_eq!(doc4.quirks_mode, QuirksMode::NoQuirks);
    }

    #[test]
    fn test_adoption_agency_misnested_formatting() {
        // Misnested <b> and <i> tags: <b>bold <i>bold-italic</b> still-italic</i> normal
        let html = "<b>bold <i>bold-italic</b> still-italic</i> normal";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body found");

        let body_children: Vec<_> = doc.children(body_id).collect();
        // Expected children of body:
        // 1. <b> containing "bold " and <i> containing "bold-italic"
        // 2. <i> containing " still-italic"
        // 3. Text(" normal")
        assert!(body_children.len() >= 3);

        let first = body_children[0];
        if let NodeData::Element(el) = &first.data {
            assert_eq!(el.tag_name, "b");
            assert_eq!(doc.text_content(first.id), "bold bold-italic");
        } else {
            panic!("expected <b> element");
        }

        let second = body_children[1];
        if let NodeData::Element(el) = &second.data {
            assert_eq!(el.tag_name, "i");
            assert_eq!(doc.text_content(second.id), " still-italic");
        } else {
            panic!("expected <i> element");
        }

        let third = body_children[2];
        if let NodeData::Text(t) = &third.data {
            assert_eq!(t, " normal");
        } else {
            panic!("expected text node ' normal'");
        }
    }

    #[test]
    fn test_adoption_agency_multiple_misnested_formatting() {
        // Deeply misnested: <b>1<i>2<u>3</b>4</u>5</i>
        let html = "<b>1<i>2<u>3</b>4</u>5</i>";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body found");

        let body_children: Vec<_> = doc.children(body_id).collect();
        assert_eq!(body_children.len(), 2);

        // First child of body: <b> with "123"
        let b_node = body_children[0];
        assert_eq!(doc.text_content(b_node.id), "123");

        // Second child of body: <i> with "45"
        let i_node = body_children[1];
        assert_eq!(doc.text_content(i_node.id), "45");
        // Inside second <i>, there should be <u> containing "4"
        let u_elem = doc.find_element_by_tag(i_node.id, "u").expect("u found in second i");
        assert_eq!(doc.text_content(u_elem), "4");
    }

    #[test]
    fn test_active_formatting_reconstruction_across_paragraphs() {
        // <p><b>hello<p>world
        let html = "<p><b>hello<p>world";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body found");

        let p_elements: Vec<_> = doc
            .children(body_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "p"))
            .collect();
        assert_eq!(p_elements.len(), 2);

        // Paragraph 1: contains <b> with "hello"
        let p1_b = doc.find_element_by_tag(p_elements[0].id, "b").expect("b in p1");
        assert_eq!(doc.text_content(p1_b), "hello");

        // Paragraph 2: active formatting reconstructed <b> with "world"
        let p2_b = doc.find_element_by_tag(p_elements[1].id, "b").expect("b in p2");
        assert_eq!(doc.text_content(p2_b), "world");
    }

    #[test]
    fn test_adoption_agency_with_furthest_block() {
        // <b>hello <div>world</b> more</div>
        let html = "<b>hello <div>world</b> more</div>";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body found");

        // div must be adopted into body as sibling after b
        let div_id = doc.find_element_by_tag(body_id, "div").expect("div found");
        let b_inside_div = doc.find_element_by_tag(div_id, "b").expect("b inside div");
        assert_eq!(doc.text_content(b_inside_div), "world");

        // " more" should be inside div after <b>
        assert!(doc.text_content(div_id).contains("world"));
        assert!(doc.text_content(div_id).contains(" more"));
    }

    #[test]
    fn test_nested_anchor_tags_adoption() {
        let html = r#"<a href="/one">one <a href="/two">two</a></a>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body found");

        let a_elements: Vec<_> = doc
            .children(body_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "a"))
            .collect();

        // <a> tags must not nest; they should be siblings under body
        assert_eq!(a_elements.len(), 2);
        assert_eq!(doc.text_content(a_elements[0].id), "one ");
        assert_eq!(doc.text_content(a_elements[1].id), "two");
    }

    #[test]
    fn test_table_marker_clearing() {
        let html = "<table><tr><td><b>cell 1<td>cell 2</table>";
        let doc = parse_html(html);
        let root = doc.root();

        let table_id = doc.find_element_by_tag(root, "table").expect("table found");
        let tds: Vec<_> = doc
            .children(table_id)
            .flat_map(|tbody| doc.children(tbody.id)) // tr
            .flat_map(|tr| doc.children(tr.id)) // td
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "td"))
            .collect();

        assert_eq!(tds.len(), 2);
        // Cell 1 has <b>
        assert!(doc.find_element_by_tag(tds[0].id, "b").is_some());
        assert_eq!(doc.text_content(tds[0].id), "cell 1");

        // Cell 2 must NOT have <b>
        assert!(doc.find_element_by_tag(tds[1].id, "b").is_none());
        assert_eq!(doc.text_content(tds[1].id), "cell 2");
    }

    #[test]
    fn test_parse_iframe_element() {
        let html = r#"<!DOCTYPE html><html><body><iframe src="https://example.com/embed" width="500" height="300" sandbox="allow-scripts">Fallback text content</iframe></body></html>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let iframe_id = doc.find_element_by_tag(root, "iframe").expect("iframe element should be found");
        let node = doc.get(iframe_id).unwrap();
        if let NodeData::Element(el) = &node.data {
            assert_eq!(el.tag_name, "iframe");
            assert_eq!(el.get_attribute("src"), Some("https://example.com/embed"));
            assert_eq!(el.get_attribute("width"), Some("500"));
            assert_eq!(el.get_attribute("height"), Some("300"));
            assert_eq!(el.get_attribute("sandbox"), Some("allow-scripts"));
        } else {
            panic!("Expected Element node for iframe");
        }
        assert_eq!(doc.text_content(iframe_id), "Fallback text content");
    }

    #[test]
    fn test_in_table_modes_implicit_tbody() {
        let html = "<table><tr><td>Cell 1</td><th>Header 1</th></tr></table>";
        let doc = parse_html(html);
        let root = doc.root();
        let table_id = doc.find_element_by_tag(root, "table").expect("table exists");
        let tbody_id = doc.find_element_by_tag(table_id, "tbody").expect("implicit tbody inserted");
        let tr_id = doc.find_element_by_tag(tbody_id, "tr").expect("tr inserted");
        let td_id = doc.find_element_by_tag(tr_id, "td").expect("td inserted");
        let th_id = doc.find_element_by_tag(tr_id, "th").expect("th inserted");

        assert_eq!(doc.text_content(td_id), "Cell 1");
        assert_eq!(doc.text_content(th_id), "Header 1");
    }

    #[test]
    fn test_in_table_body_row_cell_implicit_closing() {
        let html = "<table><tbody><tr><td>Cell 1<td>Cell 2<tr><td>Row 2</table>";
        let doc = parse_html(html);
        let root = doc.root();
        let table_id = doc.find_element_by_tag(root, "table").expect("table exists");
        let tbody_id = doc.find_element_by_tag(table_id, "tbody").expect("tbody exists");
        let rows: Vec<_> = doc
            .children(tbody_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "tr"))
            .collect();
        assert_eq!(rows.len(), 2, "Should have 2 tr rows");

        let row1_cells: Vec<_> = doc
            .children(rows[0].id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "td"))
            .collect();
        assert_eq!(row1_cells.len(), 2, "First row should have 2 td cells");
        assert_eq!(doc.text_content(row1_cells[0].id), "Cell 1");
        assert_eq!(doc.text_content(row1_cells[1].id), "Cell 2");

        let row2_cells: Vec<_> = doc
            .children(rows[1].id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "td"))
            .collect();
        assert_eq!(row2_cells.len(), 1, "Second row should have 1 td cell");
        assert_eq!(doc.text_content(row2_cells[0].id), "Row 2");
    }

    #[test]
    fn test_in_select_and_in_select_in_table() {
        let html = "<select><option>Opt 1<option>Opt 2</select>";
        let doc = parse_html(html);
        let root = doc.root();
        let select_id = doc.find_element_by_tag(root, "select").expect("select exists");
        let options: Vec<_> = doc
            .children(select_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "option"))
            .collect();
        assert_eq!(options.len(), 2, "Options should be siblings inside select");
        assert_eq!(doc.text_content(options[0].id), "Opt 1");
        assert_eq!(doc.text_content(options[1].id), "Opt 2");

        // Select inside table:
        let table_html = "<table><tr><td><select><option>Table Opt</select></td></tr></table>";
        let table_doc = parse_html(table_html);
        let t_select = table_doc.find_element_by_tag(table_doc.root(), "select").expect("select in table");
        assert_eq!(table_doc.text_content(t_select), "Table Opt");
    }

    #[test]
    fn test_in_caption_and_in_column_group() {
        let html = "<table><caption>Table Title</caption><colgroup><col width=\"50\"><col width=\"100\"></colgroup><tr><td>Data</td></tr></table>";
        let doc = parse_html(html);
        let root = doc.root();
        let table_id = doc.find_element_by_tag(root, "table").expect("table exists");

        let caption_id = doc.find_element_by_tag(table_id, "caption").expect("caption exists");
        assert_eq!(doc.text_content(caption_id), "Table Title");

        let colgroup_id = doc.find_element_by_tag(table_id, "colgroup").expect("colgroup exists");
        let cols: Vec<_> = doc
            .children(colgroup_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "col"))
            .collect();
        assert_eq!(cols.len(), 2, "Should have 2 col elements");

        let td_id = doc.find_element_by_tag(table_id, "td").expect("td exists");
        assert_eq!(doc.text_content(td_id), "Data");
    }

    #[test]
    fn test_in_template_and_template_fragment() {
        let html = "<template><div>Hello inside template</div><p>Another node</p></template>";
        let doc = parse_html(html);
        let root = doc.root();
        let t_id = doc.find_element_by_tag(root, "template").expect("template element exists");

        // Direct text_content on template should be empty per WHATWG §4.12.3
        assert_eq!(doc.text_content(t_id), "");

        // template_contents should be a DocumentFragment containing div and p
        let frag_id = doc.template_contents(t_id).expect("template_contents DocumentFragment exists");
        let frag_node = doc.get(frag_id).expect("fragment node exists");
        assert_eq!(frag_node.data, NodeData::DocumentFragment);

        let div_id = doc.find_element_by_tag(frag_id, "div").expect("div inside fragment");
        assert_eq!(doc.text_content(div_id), "Hello inside template");

        let p_id = doc.find_element_by_tag(frag_id, "p").expect("p inside fragment");
        assert_eq!(doc.text_content(p_id), "Another node");

        // Serialized HTML of template should serialize template_contents
        let serialized = doc.serialize_html(t_id);
        assert!(serialized.contains("<div>Hello inside template</div>"));
        assert!(serialized.contains("<p>Another node</p>"));
    }

    #[test]
    fn test_nested_templates_stack() {
        let html = "<template><div id=\"outer\"><template><span id=\"inner\">Nested</span></template></div></template>";
        let doc = parse_html(html);
        let root = doc.root();
        let outer_t = doc.find_element_by_tag(root, "template").expect("outer template exists");
        let outer_frag = doc.template_contents(outer_t).expect("outer template contents");
        let outer_div = doc.find_element_by_tag(outer_frag, "div").expect("outer div");

        let inner_t = doc.find_element_by_tag(outer_div, "template").expect("inner template");
        let inner_frag = doc.template_contents(inner_t).expect("inner template contents");
        let inner_span = doc.find_element_by_tag(inner_frag, "span").expect("inner span");
        assert_eq!(doc.text_content(inner_span), "Nested");
    }

    #[test]
    fn test_in_frameset_and_after_frameset() {
        let html = r#"<!DOCTYPE html><html><head><title>Frames</title></head><frameset cols="50%,50%"><frame src="a.html"><frame src="b.html"><noframes><p>No frames</p></noframes></frameset></html>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let html_id = doc.find_element_by_tag(root, "html").expect("html element exists");
        let frameset_id = doc.find_element_by_tag(html_id, "frameset").expect("frameset exists");
        let frames: Vec<_> = doc
            .children(frameset_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "frame"))
            .collect();
        assert_eq!(frames.len(), 2, "Should have 2 frame elements");
    }

    #[test]
    fn test_foster_parenting_misnested_table_content() {
        let html = "<table><div>Misplaced Div</div><tr><td>Cell Content</td></tr></table>";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body exists");
        let table_id = doc.find_element_by_tag(body_id, "table").expect("table exists");

        // The misplaced <div> must be foster-parented: inserted into body BEFORE the table!
        let body_children: Vec<_> = doc.children(body_id).collect();
        let div_pos = body_children
            .iter()
            .position(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "div"))
            .expect("div in body");
        let table_pos = body_children
            .iter()
            .position(|n| n.id == table_id)
            .expect("table in body");
        assert!(div_pos < table_pos, "Misplaced div must appear BEFORE table in body due to foster parenting");

        // Cell content remains in table
        let td_id = doc.find_element_by_tag(table_id, "td").expect("td in table");
        assert_eq!(doc.text_content(td_id), "Cell Content");

        // Misplaced text also foster parented
        let text_html = "<table>Misplaced Text<tr><td>Cell</td></tr></table>";
        let text_doc = parse_html(text_html);
        let text_body = text_doc.find_element_by_tag(text_doc.root(), "body").expect("body exists");
        let text_table = text_doc.find_element_by_tag(text_body, "table").expect("table exists");
        let body_text = text_doc.text_content(text_body);
        assert!(body_text.contains("Misplaced Text"));
        let table_text = text_doc.text_content(text_table);
        assert!(!table_text.contains("Misplaced Text"), "Table must not contain misplaced text");
    }

    #[test]
    fn test_implicit_tag_closing_special_elements() {
        // <p> implicitly closes preceding <p>
        let html = "<p>First<p>Second<p>Third";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body exists");
        let p_elements: Vec<_> = doc
            .children(body_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "p"))
            .collect();
        assert_eq!(p_elements.len(), 3, "Three sibling p elements");

        // <li> implicitly closes preceding <li>
        let list_html = "<ul><li>One<li>Two<li>Three</ul>";
        let list_doc = parse_html(list_html);
        let ul_id = list_doc.find_element_by_tag(list_doc.root(), "ul").expect("ul exists");
        let li_elements: Vec<_> = list_doc
            .children(ul_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "li"))
            .collect();
        assert_eq!(li_elements.len(), 3, "Three sibling li elements");
    }

    #[test]
    fn test_foreign_content_parsing_svg() {
        let html = r#"<svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="red" /><rect x="0" y="0" width="10" height="10" /><linearGradient id="grad1"><stop offset="0%" /></linearGradient></svg><p>After SVG</p>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body exists");

        let svg_id = doc.find_element_by_tag(body_id, "svg").expect("svg exists");
        let svg_node = doc.get(svg_id).unwrap();
        if let NodeData::Element(el) = &svg_node.data {
            assert_eq!(el.get_attribute("viewBox"), Some("0 0 100 100"), "viewBox attribute casing must be preserved");
        } else {
            panic!("Expected element for svg");
        }

        let svg_children: Vec<_> = doc
            .children(svg_id)
            .filter_map(|n| {
                if let NodeData::Element(el) = &n.data {
                    Some((el.tag_name.clone(), n.id))
                } else {
                    None
                }
            })
            .collect();

        // circle, rect, linearGradient should be siblings under svg (circle self-closing was recognized)
        assert_eq!(svg_children.len(), 3, "SVG should contain 3 top-level elements: circle, rect, linearGradient");
        assert_eq!(svg_children[0].0, "circle");
        assert_eq!(svg_children[1].0, "rect");
        assert_eq!(svg_children[2].0, "linearGradient", "linearGradient tag name casing must be preserved");

        // <p>After SVG</p> is sibling of <svg> in <body>
        let p_id = doc.find_element_by_tag(body_id, "p").expect("p exists in body");
        assert_eq!(doc.text_content(p_id), "After SVG");
    }

    #[test]
    fn test_dialog_and_ruby_tree_building() {
        let html = "<p>Intro<dialog open>Dialog text</dialog><p>Outro</p>";
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body exists");
        let dialog_id = doc.find_element_by_tag(body_id, "dialog").expect("dialog in body");
        assert_eq!(doc.text_content(dialog_id), "Dialog text");

        let ruby_html = "<ruby>漢<rp>(<rt>かん<rp>)<rt>kan</ruby>";
        let rdoc = parse_html(ruby_html);
        let rroot = rdoc.root();
        let rbody_id = rdoc.find_element_by_tag(rroot, "body").expect("body exists");
        let ruby_id = rdoc.find_element_by_tag(rbody_id, "ruby").expect("ruby in body");
        let rts: Vec<_> = rdoc
            .children(ruby_id)
            .filter(|n| matches!(&n.data, NodeData::Element(el) if el.tag_name == "rt"))
            .collect();
        assert_eq!(rts.len(), 2, "Second rt should auto-close first rt");
        assert_eq!(rdoc.text_content(rts[0].id), "かん");
        assert_eq!(rdoc.text_content(rts[1].id), "kan");
    }

    #[test]
    fn test_optgroup_and_option_tree_building() {
        let html = r#"<select>
            <option>Option 1
            <option>Option 2
            <optgroup label="Group A">
                <option>Option A1
                <option>Option A2
            <optgroup label="Group B">
                <option>Option B1
        </select>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let body_id = doc.find_element_by_tag(root, "body").expect("body exists");
        let select_id = doc.find_element_by_tag(body_id, "select").expect("select in body");

        let select_children: Vec<_> = doc
            .children(select_id)
            .filter(|n| matches!(&n.data, NodeData::Element(_)))
            .map(|n| {
                if let NodeData::Element(el) = &n.data {
                    (el.tag_name.clone(), n.id)
                } else {
                    unreachable!()
                }
            })
            .collect();

        // 2 direct options + 2 optgroups
        assert_eq!(select_children.len(), 4, "Select should have 4 children: option, option, optgroup, optgroup");
        assert_eq!(select_children[0].0, "option");
        assert_eq!(select_children[1].0, "option");
        assert_eq!(select_children[2].0, "optgroup");
        assert_eq!(select_children[3].0, "optgroup");

        // Group A should have 2 options
        let group_a_children: Vec<_> = doc
            .children(select_children[2].1)
            .filter(|n| matches!(&n.data, NodeData::Element(_)))
            .collect();
        assert_eq!(group_a_children.len(), 2, "Group A should contain 2 options");
    }

    #[test]
    fn test_meta_charset_detection() {
        let html = r#"<!DOCTYPE html><html><head><meta charset="windows-1252"></head><body></body></html>"#;
        let doc = parse_html(html);
        assert_eq!(doc.character_set, "windows-1252");

        let html_utf8 = r#"<!DOCTYPE html><html><head><meta charset="utf-8"></head><body></body></html>"#;
        let doc2 = parse_html(html_utf8);
        assert_eq!(doc2.character_set, "UTF-8");

        let html_latin1 = r#"<!DOCTYPE html><html><head><meta charset="iso-8859-1"></head><body></body></html>"#;
        let doc3 = parse_html(html_latin1);
        assert_eq!(doc3.character_set, "windows-1252");
    }

    #[test]
    fn test_meta_http_equiv_content_type() {
        let html = r#"<!DOCTYPE html><html><head><meta http-equiv="Content-Type" content="text/html; charset=windows-1252"></head><body></body></html>"#;
        let doc = parse_html(html);
        assert_eq!(doc.character_set, "windows-1252");

        // Order independence of attributes
        let html_reverse = r#"<!DOCTYPE html><html><head><meta content="text/html; charset=utf-8" http-equiv="content-type"></head><body></body></html>"#;
        let doc2 = parse_html(html_reverse);
        assert_eq!(doc2.character_set, "UTF-8");
    }
}

