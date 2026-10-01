//! CSS cascade resolution, user-agent stylesheet defaults, and priority sorting.

use std::collections::{HashMap, HashSet};
use mango_core::Color;
use mango_html::dom::{Document, NodeData, NodeId};

use crate::parser::{parse_declaration_list, parse_stylesheet, Rule, StyleRule, Stylesheet};
use crate::properties::Declaration;
use crate::specificity::Specificity;
use crate::values::{Length, Value};

/// Style origin for cascade ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    UserAgent = 0,
    PresentationalHint = 1,
    Author = 2,
    Inline = 3,
}

/// A property declaration that matched a target DOM node during cascade evaluation.
#[derive(Debug, Clone)]
pub struct MatchedDeclaration {
    pub declaration: Declaration,
    pub specificity: Specificity,
    pub origin: Origin,
    pub source_order: usize,
}

impl MatchedDeclaration {
    /// Computes a sort key according to CSS Cascade Level 4 precedence:
    ///
    /// 1. Origin + Importance:
    ///    - `Author !important` (highest)
    ///    - `UserAgent !important`
    ///    - `Inline` (normal)
    ///    - `Author` (normal)
    ///    - `PresentationalHint` (normal)
    ///    - `UserAgent` (normal)
    /// 2. Specificity (higher wins)
    /// 3. Source order (later wins)
    fn cascade_weight(&self) -> (u8, Specificity, usize) {
        let precedence = match (self.origin, self.declaration.important) {
            (Origin::UserAgent, true) => 7,
            (Origin::Inline, true) => 6,
            (Origin::Author, true) => 5,
            (Origin::Inline, false) => 4,
            (Origin::Author, false) => 3,
            (Origin::PresentationalHint, false) => 2,
            (Origin::PresentationalHint, true) => 2,
            (Origin::UserAgent, false) => 1,
        };
        (precedence, self.specificity, self.source_order)
    }
}

/// The built-in HTML5 default User-Agent stylesheet.
pub fn user_agent_stylesheet() -> &'static Stylesheet {
    static UA_STYLESHEET: std::sync::OnceLock<Stylesheet> = std::sync::OnceLock::new();
    UA_STYLESHEET.get_or_init(|| {
        let css = r#"
            head, title, style, script, link, meta, noscript, template {
                display: none;
            }
            slot {
                display: contents;
            }
            [hidden] {
                display: none;
            }
            html, body, div, p, h1, h2, h3, h4, h5, h6,
            ul, ol, dl, dt, dd, blockquote, pre, form,
            header, footer, nav, section, article,
            aside, main, figure, figcaption,
            center, address, details, summary,
            ytd-app, ytd-masthead, ytd-page-manager, ytd-browse,
            ytd-two-column-browse-results-renderer, ytd-rich-grid-renderer {
                display: block;
            }
            details:not([open]) > :not(summary) {
                display: none;
            }
            body {
                margin: 8px;
                color: black;
                background-color: white;
                font-size: 16px;
                font-family: serif;
                font-weight: normal;
                font-style: normal;
            }
            code, kbd, samp, pre, tt {
                font-family: monospace;
            }
            h1 {
                font-size: 32px;
                font-weight: bold;
                margin: 21px 0px;
            }
            h2 {
                font-size: 24px;
                font-weight: bold;
                margin: 19px 0px;
            }
            h3 {
                font-size: 19px;
                font-weight: bold;
                margin: 18px 0px;
            }
            h4 {
                font-size: 16px;
                font-weight: bold;
                margin: 21px 0px;
            }
            h5 {
                font-size: 13px;
                font-weight: bold;
                margin: 22px 0px;
            }
            h6 {
                font-size: 11px;
                font-weight: bold;
                margin: 25px 0px;
            }
            p {
                margin: 16px 0px;
            }
            b, strong {
                font-weight: bold;
            }
            i, em {
                font-style: italic;
            }
            a {
                color: #0000ee;
                text-decoration: underline;
                cursor: pointer;
            }
            sup {
                vertical-align: super;
                font-size: 0.8em;
            }
            sub {
                vertical-align: sub;
                font-size: 0.8em;
            }
            ul {
                display: block;
                list-style-type: disc;
                margin: 16px 0px;
                padding-left: 30px;
            }
            ol {
                display: block;
                list-style-type: decimal;
                margin: 16px 0px;
                padding-left: 30px;
            }
            li {
                display: list-item;
            }
            center {
                display: block;
                text-align: center;
            }
            center > table {
                margin-left: auto;
                margin-right: auto;
            }
            table {
                display: table;
                margin: 8px 0px;
                text-align: left;
            }
            caption {
                display: table-caption;
                text-align: center;
            }
            colgroup {
                display: table-column-group;
            }
            col {
                display: table-column;
            }
            thead {
                display: table-header-group;
            }
            tbody {
                display: table-row-group;
            }
            tfoot {
                display: table-footer-group;
            }
            tr {
                display: table-row;
            }
            td, th {
                display: table-cell;
                padding: 4px 8px;
            }
            th {
                font-weight: bold;
                text-align: center;
            }
            form {
                display: block;
                margin: 0px;
            }
            fieldset {
                display: block;
                margin: 8px 2px;
                padding: 6px 10px;
                border: 1px solid #c0c0c0;
                border-radius: 4px;
            }
            legend {
                display: block;
                padding: 0px 4px;
                font-weight: bold;
            }
            label {
                display: inline-block;
                cursor: default;
            }
            input, button, select, textarea {
                display: inline-block;
                font-family: sans-serif;
                font-size: 13px;
                box-sizing: border-box;
            }
            input, select, textarea {
                background-color: #ffffff;
                border: 1px solid #767676;
                border-radius: 3px;
                padding: 3px 6px;
            }
            iframe {
                display: inline-block;
                width: 300px;
                height: 150px;
                border: 2px inset #767676;
                box-sizing: border-box;
            }
            video {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            canvas {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            audio {
                display: none;
                box-sizing: border-box;
            }
            audio[controls] {
                display: inline-block;
                width: 300px;
                height: 36px;
            }
            :focus-visible, input:focus, select:focus, textarea:focus, button:focus {
                outline: 2px solid #0078d7;
            }
            input[type="checkbox"], input[type="radio"] {
                border-top-width: 0px;
                border-right-width: 0px;
                border-bottom-width: 0px;
                border-left-width: 0px;
                border-top-style: none;
                border-right-style: none;
                border-bottom-style: none;
                border-left-style: none;
                background-color: transparent;
                padding: 0px;
            }
            input[type="hidden"] {
                display: none;
            }
            button, input[type="submit"], input[type="button"], input[type="reset"] {
                background-color: #f0f0f0;
                border: 1px solid #767676;
                border-radius: 4px;
                padding: 4px 12px;
                cursor: pointer;
                text-align: center;
                color: #000000;
            }
            img {
                display: inline-block;
            }
            svg {
                display: inline-block;
                vertical-align: middle;
            }
            svg:not(:root) {
                overflow: hidden;
            }
            hr {
                display: block;
                margin: 8px 0px;
                border-top-width: 1px;
                border-top-style: solid;
                border-top-color: gray;
            }
            dialog {
                display: none;
                position: absolute;
                left: 0px;
                right: 0px;
                top: 0px;
                bottom: 0px;
                margin: auto;
                border: 1px solid black;
                padding: 16px;
                background-color: white;
                color: black;
                box-sizing: border-box;
            }
            dialog[open] {
                display: block;
            }
            datalist {
                display: none;
            }
            optgroup {
                display: block;
                font-weight: bold;
                font-style: italic;
            }
            optgroup > option {
                font-weight: normal;
                font-style: normal;
                padding-left: 12px;
            }
            select[multiple], select[size] {
                vertical-align: top;
            }
            progress {
                display: inline-block;
                vertical-align: -0.2em;
                width: 160px;
                height: 16px;
                box-sizing: border-box;
            }
            meter {
                display: inline-block;
                vertical-align: -0.2em;
                width: 80px;
                height: 16px;
                box-sizing: border-box;
            }
            output {
                display: inline;
            }
            map {
                display: inline;
            }
            area {
                display: none;
            }
            object, embed {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            wbr {
                display: inline;
            }
            ruby {
                display: inline;
            }
            rt {
                font-size: 50%;
                line-height: 1;
                vertical-align: super;
            }
            rp {
                display: none;
            }
            bdi {
                unicode-bidi: isolate;
            }
            bdo {
                unicode-bidi: bidi-override;
            }
            [dir="rtl"] {
                direction: rtl;
            }
            [dir="ltr"] {
                direction: ltr;
            }
            bdo[dir="rtl"] {
                direction: rtl;
                unicode-bidi: bidi-override;
            }
            bdo[dir="ltr"] {
                direction: ltr;
                unicode-bidi: bidi-override;
            }
            #masthead-logo a:first-of-type,
            ytd-masthead #masthead-logo a:first-of-type {
                display: flex !important;
                align-items: center;
            }
        "#;
        parse_stylesheet(css)
    })
}

/// An indexed style rule for accelerated lookup.
#[derive(Debug, Clone)]
pub struct IndexedRule<'a> {
    pub style_rule: &'a StyleRule,
    pub origin: Origin,
    pub source_order: usize,
}

/// An indexed container query rule for accelerated container matching.
#[derive(Debug, Clone)]
pub struct IndexedContainerRule<'a> {
    pub name: Option<String>,
    pub query: String,
    pub style_rule: &'a StyleRule,
    pub origin: Origin,
    pub source_order: usize,
}

/// An index of CSS rules grouped by the key selector (ID, class, tag, universal, pseudo)
/// to reduce cascade resolution complexity from O(Rules) to O(1) candidate lookup per element.
#[derive(Debug, Clone)]
pub struct RuleIndex<'a> {
    pub viewport_width: f32,
    pub viewport_height: f32,
    id_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    class_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    tag_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    universal_rules: Vec<IndexedRule<'a>>,
    pseudo_rules: HashMap<String, Vec<IndexedRule<'a>>>,
    pub container_rules: Vec<IndexedContainerRule<'a>>,
}

impl<'a> Default for RuleIndex<'a> {
    fn default() -> Self {
        Self::new_with_size(1280.0, 900.0)
    }
}

impl<'a> RuleIndex<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_size(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            viewport_width,
            viewport_height,
            id_rules: HashMap::new(),
            class_rules: HashMap::new(),
            tag_rules: HashMap::new(),
            universal_rules: Vec::new(),
            pseudo_rules: HashMap::new(),
            container_rules: Vec::new(),
        }
    }

    pub fn from_stylesheets(
        stylesheets: &[&'a Stylesheet],
        origin: Origin,
        order: &mut usize,
    ) -> Self {
        Self::from_stylesheets_with_size(stylesheets, origin, order, 1280.0, 900.0)
    }

    pub fn from_stylesheets_with_size(
        stylesheets: &[&'a Stylesheet],
        origin: Origin,
        order: &mut usize,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Self {
        let mut index = Self::new_with_size(viewport_width, viewport_height);
        for sheet in stylesheets {
            index.add_stylesheet(sheet, origin, order);
        }
        index
    }

    pub fn add_stylesheet(&mut self, sheet: &'a Stylesheet, origin: Origin, order: &mut usize) {
        for rule in &sheet.rules {
            match rule {
                Rule::Style(style_rule) => {
                    self.add_style_rule(style_rule, origin, order);
                }
                Rule::Media(media_rule)
                    if matches_media_query_size(&media_rule.query, self.viewport_width, self.viewport_height) =>
                {
                    for style_rule in &media_rule.rules {
                        self.add_style_rule(style_rule, origin, order);
                    }
                }
                Rule::Container(container_rule) => {
                    for style_rule in &container_rule.rules {
                        *order += 1;
                        self.container_rules.push(IndexedContainerRule {
                            name: container_rule.name.clone(),
                            query: container_rule.query.clone(),
                            style_rule,
                            origin,
                            source_order: *order,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn add_style_rule(&mut self, style_rule: &'a StyleRule, origin: Origin, order: &mut usize) {
        *order += 1;
        let rule_order = *order;

        for complex_sel in &style_rule.selectors.selectors {
            let subject = if let Some((_, compound)) = complex_sel.tail.last() {
                compound
            } else {
                &complex_sel.head
            };

            let indexed = IndexedRule {
                style_rule,
                origin,
                source_order: rule_order,
            };

            // 1. Check if subject targets a pseudo-element
            let mut pseudo_target = None;
            for s in &subject.simple_selectors {
                if let crate::selectors::SimpleSelector::PseudoElement(p) = s {
                    pseudo_target = Some(p.to_ascii_lowercase());
                    break;
                }
            }

            if let Some(mut pseudo) = pseudo_target {
                if pseudo == "-webkit-input-placeholder"
                    || pseudo == "-moz-placeholder"
                    || pseudo == "-ms-input-placeholder"
                {
                    pseudo = "placeholder".to_string();
                } else if pseudo == "-moz-selection" {
                    pseudo = "selection".to_string();
                }
                self.pseudo_rules.entry(pseudo).or_default().push(indexed);
                continue;
            }

            // 2. Classify key selector: ID > Class > Tag > Universal
            let mut key_id = None;
            let mut key_class = None;
            let mut key_tag = None;

            for s in &subject.simple_selectors {
                match s {
                    crate::selectors::SimpleSelector::Id(id) if key_id.is_none() => {
                        key_id = Some(id.clone());
                    }
                    crate::selectors::SimpleSelector::Class(cls) if key_class.is_none() => {
                        key_class = Some(cls.clone());
                    }
                    crate::selectors::SimpleSelector::Type(tag) if key_tag.is_none() => {
                        key_tag = Some(tag.to_ascii_lowercase());
                    }
                    _ => {}
                }
            }

            if let Some(id) = key_id {
                self.id_rules.entry(id).or_default().push(indexed);
            } else if let Some(cls) = key_class {
                self.class_rules.entry(cls).or_default().push(indexed);
            } else if let Some(tag) = key_tag {
                self.tag_rules.entry(tag).or_default().push(indexed);
            } else {
                self.universal_rules.push(indexed);
            }
        }
    }

    /// Collects candidate style rules that could match the given element node.
    pub fn collect_matches_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        elem: &mango_html::dom::ElementData,
        source_order: &mut usize,
        out: &mut Vec<MatchedDeclaration>,
    ) {
        let mut seen_rules: HashSet<usize> = HashSet::new();

        // 1. Universal rules (always candidates)
        for cand in &self.universal_rules {
            if seen_rules.insert(cand.source_order) {
                apply_style_rule(cand.style_rule, node_id, doc, cand.origin, source_order, out);
            }
        }

        // 2. Tag name bucket
        let tag_lower = elem.tag_name.to_ascii_lowercase();
        if let Some(rules) = self.tag_rules.get(&tag_lower) {
            for cand in rules {
                if seen_rules.insert(cand.source_order) {
                    apply_style_rule(cand.style_rule, node_id, doc, cand.origin, source_order, out);
                }
            }
        }

        // 3. ID bucket
        if let Some(id) = elem.id() {
            if let Some(rules) = self.id_rules.get(id) {
                for cand in rules {
                    if seen_rules.insert(cand.source_order) {
                        apply_style_rule(cand.style_rule, node_id, doc, cand.origin, source_order, out);
                    }
                }
            }
        }

        // 4. Class buckets
        for cls in elem.classes() {
            if let Some(rules) = self.class_rules.get(cls) {
                for cand in rules {
                    if seen_rules.insert(cand.source_order) {
                        apply_style_rule(cand.style_rule, node_id, doc, cand.origin, source_order, out);
                    }
                }
            }
        }

        // 5. Container rules
        for cand in &self.container_rules {
            if seen_rules.insert(cand.source_order) {
                if let Some((cw, ch)) = self.find_container_size_for_node(node_id, doc, cand.name.as_deref()) {
                    if matches_container_query(&cand.query, cw, ch) {
                        apply_style_rule(cand.style_rule, node_id, doc, cand.origin, source_order, out);
                    }
                }
            }
        }
    }

    /// Collects candidate rules for a pseudo-element on the target node.
    pub fn collect_pseudo_matches_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        pseudo: &str,
        source_order: &mut usize,
        out: &mut Vec<MatchedDeclaration>,
    ) {
        let pseudo_lower = pseudo.to_ascii_lowercase();
        let mut seen_rules: HashSet<usize> = HashSet::new();

        if let Some(rules) = self.pseudo_rules.get(&pseudo_lower) {
            for cand in rules {
                if !seen_rules.insert(cand.source_order) {
                    continue;
                }
                for complex_sel in &cand.style_rule.selectors.selectors {
                    if complex_sel.matches_pseudo_element(node_id, doc, pseudo) {
                        let specificity = Specificity::of(complex_sel);
                        for decl in &cand.style_rule.declarations {
                            for expanded in decl.clone().expand_shorthand() {
                                *source_order += 1;
                                out.push(MatchedDeclaration {
                                    declaration: expanded,
                                    specificity,
                                    origin: cand.origin,
                                    source_order: *source_order,
                                });
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Container rules targeting pseudo-elements
        for cand in &self.container_rules {
            if !seen_rules.insert(cand.source_order) {
                continue;
            }
            if let Some((cw, ch)) = self.find_container_size_for_node(node_id, doc, cand.name.as_deref()) {
                if matches_container_query(&cand.query, cw, ch) {
                    for complex_sel in &cand.style_rule.selectors.selectors {
                        if complex_sel.matches_pseudo_element(node_id, doc, pseudo) {
                            let specificity = Specificity::of(complex_sel);
                            for decl in &cand.style_rule.declarations {
                                for expanded in decl.clone().expand_shorthand() {
                                    *source_order += 1;
                                    out.push(MatchedDeclaration {
                                        declaration: expanded,
                                        specificity,
                                        origin: cand.origin,
                                        source_order: *source_order,
                                    });
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Locates the appropriate ancestor container element for `node_id`, matching `name_filter` if given,
    /// and returns its resolved (width, height) in pixels.
    pub fn find_container_size_for_node(
        &self,
        node_id: NodeId,
        doc: &Document,
        name_filter: Option<&str>,
    ) -> Option<(f32, f32)> {
        let mut current = doc.get(node_id).and_then(|n| n.parent);
        while let Some(ancestor_id) = current {
            let Some(ancestor_node) = doc.get(ancestor_id) else { break; };
            if let NodeData::Element(elem) = &ancestor_node.data {
                let mut container_type = None;
                let mut container_name = None;
                let mut explicit_width = None;
                let mut explicit_height = None;

                // 1. Inspect element's inline style
                if let Some(style_attr) = elem.get_attribute("style") {
                    let decls = parse_declaration_list(style_attr);
                    for d in decls {
                        let name = d.name.to_ascii_lowercase();
                        if name == "container-type" {
                            container_type = Some(d.value);
                        } else if name == "container-name" {
                            if let Value::String(s) = d.value {
                                container_name = Some(s);
                            } else if let Value::Keyword(s) = d.value {
                                container_name = Some(s);
                            }
                        } else if name == "container" {
                            for exp in d.expand_shorthand() {
                                if exp.name == "container-name" {
                                    if let Value::String(s) = exp.value {
                                        container_name = Some(s);
                                    } else if let Value::Keyword(s) = exp.value {
                                        container_name = Some(s);
                                    }
                                } else if exp.name == "container-type" {
                                    container_type = Some(exp.value);
                                }
                            }
                        } else if name == "width" {
                            if let Value::Length(Length::Px(px)) = d.value {
                                explicit_width = Some(px);
                            }
                        } else if name == "height" {
                            if let Value::Length(Length::Px(px)) = d.value {
                                explicit_height = Some(px);
                            }
                        }
                    }
                }

                // 2. Inspect author stylesheet rules matching ancestor_id
                let mut candidate_rules: Vec<&IndexedRule<'a>> = Vec::new();
                for cand in &self.universal_rules {
                    candidate_rules.push(cand);
                }
                let tag_lower = elem.tag_name.to_ascii_lowercase();
                if let Some(rules) = self.tag_rules.get(&tag_lower) {
                    for cand in rules {
                        candidate_rules.push(cand);
                    }
                }
                if let Some(id) = elem.id() {
                    let id_lower = id.to_ascii_lowercase();
                    if let Some(rules) = self.id_rules.get(&id_lower) {
                        for cand in rules {
                            candidate_rules.push(cand);
                        }
                    }
                }
                for cls in elem.classes() {
                    let cls_lower = cls.to_ascii_lowercase();
                    if let Some(rules) = self.class_rules.get(&cls_lower) {
                        for cand in rules {
                            candidate_rules.push(cand);
                        }
                    }
                }

                for cand in candidate_rules {
                    for complex_sel in &cand.style_rule.selectors.selectors {
                        if complex_sel.matches(ancestor_id, doc) {
                            for decl in &cand.style_rule.declarations {
                                for exp in decl.clone().expand_shorthand() {
                                    let dname = exp.name.to_ascii_lowercase();
                                    if dname == "container-type" && container_type.is_none() {
                                        container_type = Some(exp.value);
                                    } else if dname == "container-name" && container_name.is_none() {
                                        if let Value::String(s) = exp.value {
                                            container_name = Some(s);
                                        } else if let Value::Keyword(s) = exp.value {
                                            container_name = Some(s);
                                        }
                                    } else if dname == "width" && explicit_width.is_none() {
                                        if let Value::Length(Length::Px(px)) = exp.value {
                                            explicit_width = Some(px);
                                        }
                                    } else if dname == "height" && explicit_height.is_none() {
                                        if let Value::Length(Length::Px(px)) = exp.value {
                                            explicit_height = Some(px);
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                }

                // 3. HTML attributes
                if explicit_width.is_none() {
                    if let Some(w_str) = elem.get_attribute("width") {
                        if let Ok(w) = w_str.trim_end_matches("px").parse::<f32>() {
                            explicit_width = Some(w);
                        }
                    }
                }
                if explicit_height.is_none() {
                    if let Some(h_str) = elem.get_attribute("height") {
                        if let Ok(h) = h_str.trim_end_matches("px").parse::<f32>() {
                            explicit_height = Some(h);
                        }
                    }
                }
                if container_name.is_none() {
                    if let Some(cn) = elem.get_attribute("data-container-name") {
                        container_name = Some(cn.to_string());
                    }
                }
                if container_type.is_none() {
                    if let Some(ct) = elem.get_attribute("data-container-type") {
                        if let Some(parsed_ct) = crate::values::ContainerType::parse(ct) {
                            container_type = Some(Value::ContainerType(parsed_ct));
                        }
                    }
                }

                // Check if this element establishes a container
                let is_container = match &container_type {
                    Some(Value::ContainerType(ct)) => ct != &crate::values::ContainerType::Normal,
                    Some(Value::Keyword(k)) => k != "normal",
                    _ => container_name.is_some(),
                };

                if is_container {
                    let matches_name = match name_filter {
                        Some(filter) => {
                            if let Some(ref cn) = container_name {
                                cn.split_whitespace().any(|part| part.eq_ignore_ascii_case(filter))
                            } else {
                                false
                            }
                        }
                        None => true,
                    };

                    if matches_name {
                        let w = explicit_width.unwrap_or(self.viewport_width);
                        let h = explicit_height.unwrap_or(self.viewport_height);
                        return Some((w, h));
                    }
                }
            }
            current = ancestor_node.parent;
        }
        None
    }
}

/// The cached global User-Agent RuleIndex.
pub fn user_agent_rule_index() -> &'static RuleIndex<'static> {
    static UA_RULE_INDEX: std::sync::OnceLock<RuleIndex<'static>> = std::sync::OnceLock::new();
    UA_RULE_INDEX.get_or_init(|| {
        let sheet = user_agent_stylesheet();
        let mut order = 0;
        let mut index = RuleIndex::new();
        index.add_stylesheet(sheet, Origin::UserAgent, &mut order);
        index
    })
}

/// Resolves all specified property values for a given DOM node by applying
/// user-agent styles, author stylesheets (via `RuleIndex`), inline styles, and cascade priority sorting.
pub fn resolve_cascade_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
) -> HashMap<String, Value> {
    let mut matched_decls: Vec<MatchedDeclaration> = Vec::new();
    let mut source_order = 0;

    let Some(node) = doc.get(node_id) else {
        return HashMap::new();
    };

    if let NodeData::Element(elem) = &node.data {
        // 1. User Agent Stylesheet (via cached UA index)
        user_agent_rule_index().collect_matches_for_node(
            node_id,
            doc,
            elem,
            &mut source_order,
            &mut matched_decls,
        );

        // 1.5 Presentational hints from HTML attributes
        collect_presentational_hints(node_id, doc, elem, &mut source_order, &mut matched_decls);

        // 2. Author Stylesheets (via author index)
        author_index.collect_matches_for_node(
            node_id,
            doc,
            elem,
            &mut source_order,
            &mut matched_decls,
        );

        // 3. Inline style="..." attribute on the element itself
        if let Some(style_attr) = elem.get_attribute("style") {
            let inline_decls = parse_declaration_list(style_attr);
            for decl in inline_decls {
                for expanded in decl.expand_shorthand() {
                    source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: expanded,
                        specificity: Specificity(1, 0, 0),
                        origin: Origin::Inline,
                        source_order,
                    });
                }
            }
        }
    }

    matched_decls.sort_by_key(|a| a.cascade_weight());
    let mut resolved = HashMap::new();
    for matched in matched_decls {
        resolved.insert(matched.declaration.name, matched.declaration.value);
    }
    resolved
}

/// Resolves pseudo-element cascade matching using the indexed author stylesheet rules.
pub fn resolve_pseudo_element_cascade_with_index(
    node_id: NodeId,
    doc: &Document,
    author_index: &RuleIndex,
    pseudo: &str,
) -> HashMap<String, Value> {
    let mut matched_decls: Vec<MatchedDeclaration> = Vec::new();
    let mut source_order = 0;

    author_index.collect_pseudo_matches_for_node(
        node_id,
        doc,
        pseudo,
        &mut source_order,
        &mut matched_decls,
    );

    matched_decls.sort_by_key(|a| a.cascade_weight());
    let mut resolved = HashMap::new();
    for matched in matched_decls {
        resolved.insert(matched.declaration.name, matched.declaration.value);
    }
    resolved
}

/// Resolves all specified property values for a given DOM node by applying
/// user-agent styles, author stylesheets, inline styles, and cascade priority sorting.
pub fn resolve_cascade(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
) -> HashMap<String, Value> {
    let mut order = 1000;
    let author_index = RuleIndex::from_stylesheets(author_stylesheets, Origin::Author, &mut order);
    resolve_cascade_with_index(node_id, doc, &author_index)
}

/// Resolves all specified property values for a given DOM node's pseudo-element (`before`, `after`, etc.)
/// by applying author stylesheets matching that pseudo-element.
pub fn resolve_pseudo_element_cascade(
    node_id: NodeId,
    doc: &Document,
    author_stylesheets: &[&Stylesheet],
    pseudo: &str,
) -> HashMap<String, Value> {
    let mut order = 1000;
    let author_index = RuleIndex::from_stylesheets(author_stylesheets, Origin::Author, &mut order);
    resolve_pseudo_element_cascade_with_index(node_id, doc, &author_index, pseudo)
}

fn parse_presentational_color(raw: &str) -> Option<Color> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    Value::parse_color(s).or_else(|| Value::parse_color(&format!("#{s}")))
}

fn parse_presentational_dimension(raw: &str) -> Option<Value> {
    let s = raw.trim();
    if let Some(rest) = s.strip_suffix('%') {
        if let Ok(pct) = rest.trim().parse::<f32>() {
            return Some(Value::Length(Length::Percent(pct)));
        }
    }
    let num_str = s.strip_suffix("px").unwrap_or(s).trim();
    if let Ok(px) = num_str.parse::<f32>() {
        return Some(Value::Length(Length::Px(px)));
    }
    None
}

fn collect_presentational_hints(
    node_id: NodeId,
    doc: &Document,
    elem: &mango_html::dom::ElementData,
    source_order: &mut usize,
    matched_decls: &mut Vec<MatchedDeclaration>,
) {
    let tag = elem.tag_name.to_ascii_lowercase();

    // bgcolor -> background-color
    if let Some(bg) = elem.get_attribute("bgcolor") {
        if let Some(c) = parse_presentational_color(bg) {
            *source_order += 1;
            matched_decls.push(MatchedDeclaration {
                declaration: Declaration::new("background-color", Value::Color(c), false),
                specificity: Specificity(0, 0, 0),
                origin: Origin::PresentationalHint,
                source_order: *source_order,
            });
        }
    }

    // background -> background-image
    if let Some(bg_img) = elem.get_attribute("background") {
        let trimmed = bg_img.trim();
        if !trimmed.is_empty() {
            *source_order += 1;
            matched_decls.push(MatchedDeclaration {
                declaration: Declaration::new("background-image", Value::Url(trimmed.to_string()), false),
                specificity: Specificity(0, 0, 0),
                origin: Origin::PresentationalHint,
                source_order: *source_order,
            });
        }
    }

    // width
    if let Some(w) = elem.get_attribute("width") {
        if let Some(dim) = parse_presentational_dimension(w) {
            *source_order += 1;
            matched_decls.push(MatchedDeclaration {
                declaration: Declaration::new("width", dim, false),
                specificity: Specificity(0, 0, 0),
                origin: Origin::PresentationalHint,
                source_order: *source_order,
            });
        }
    }

    // height
    if let Some(h) = elem.get_attribute("height") {
        if let Some(dim) = parse_presentational_dimension(h) {
            *source_order += 1;
            matched_decls.push(MatchedDeclaration {
                declaration: Declaration::new("height", dim, false),
                specificity: Specificity(0, 0, 0),
                origin: Origin::PresentationalHint,
                source_order: *source_order,
            });
        }
    }

    // align
    if let Some(align) = elem.get_attribute("align") {
        let a = align.trim().to_ascii_lowercase();
        if tag == "table" || tag == "hr" {
            match a.as_str() {
                "center" => {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-left", Value::Keyword("auto".to_string()), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-right", Value::Keyword("auto".to_string()), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                "right" => {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-left", Value::Keyword("auto".to_string()), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-right", Value::Length(Length::Px(0.0)), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                "left" => {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-left", Value::Length(Length::Px(0.0)), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new("margin-right", Value::Keyword("auto".to_string()), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                _ => {}
            }
        } else {
            let ta = match a.as_str() {
                "center" => Some(crate::values::TextAlign::Center),
                "right" => Some(crate::values::TextAlign::Right),
                "left" => Some(crate::values::TextAlign::Left),
                "justify" => Some(crate::values::TextAlign::Justify),
                _ => None,
            };
            if let Some(ta_val) = ta {
                *source_order += 1;
                matched_decls.push(MatchedDeclaration {
                    declaration: Declaration::new("text-align", Value::TextAlign(ta_val), false),
                    specificity: Specificity(0, 0, 0),
                    origin: Origin::PresentationalHint,
                    source_order: *source_order,
                });
            }
        }
    }

    // valign
    if let Some(valign) = elem.get_attribute("valign") {
        let va = match valign.trim().to_ascii_lowercase().as_str() {
            "top" => Some(crate::values::VerticalAlign::Top),
            "middle" => Some(crate::values::VerticalAlign::Middle),
            "bottom" => Some(crate::values::VerticalAlign::Bottom),
            "baseline" => Some(crate::values::VerticalAlign::Baseline),
            _ => None,
        };
        if let Some(va_val) = va {
            *source_order += 1;
            matched_decls.push(MatchedDeclaration {
                declaration: Declaration::new("vertical-align", Value::VerticalAlign(va_val), false),
                specificity: Specificity(0, 0, 0),
                origin: Origin::PresentationalHint,
                source_order: *source_order,
            });
        }
    }

    // border (on table or img)
    if let Some(b) = elem.get_attribute("border") {
        if let Ok(b_val) = b.trim().parse::<f32>() {
            if b_val <= 0.0 {
                for side in &["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::BorderStyle(crate::values::BorderStyle::None), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                for side in &["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::Length(Length::Px(0.0)), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
            } else {
                for side in &["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::BorderStyle(crate::values::BorderStyle::Solid), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                for side in &["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::Length(Length::Px(b_val)), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
            }
        }
    }

    // frameborder (on iframe: "0" or "no" removes border)
    if tag == "iframe" {
        if let Some(fb) = elem.get_attribute("frameborder") {
            let fb_clean = fb.trim().to_ascii_lowercase();
            if fb_clean == "0" || fb_clean == "no" {
                for side in &["border-top-style", "border-right-style", "border-bottom-style", "border-left-style"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::BorderStyle(crate::values::BorderStyle::None), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
                for side in &["border-top-width", "border-right-width", "border-bottom-width", "border-left-width"] {
                    *source_order += 1;
                    matched_decls.push(MatchedDeclaration {
                        declaration: Declaration::new(*side, Value::Length(Length::Px(0.0)), false),
                        specificity: Specificity(0, 0, 0),
                        origin: Origin::PresentationalHint,
                        source_order: *source_order,
                    });
                }
            }
        }
    }

    // For table, cellspacing attribute -> border-spacing
    if tag == "table" {
        if let Some(cs_str) = elem.get_attribute("cellspacing") {
            if let Ok(cs) = cs_str.trim().parse::<f32>() {
                *source_order += 1;
                matched_decls.push(MatchedDeclaration {
                    declaration: Declaration::new("border-spacing", Value::Length(Length::Px(cs)), false),
                    specificity: Specificity(0, 0, 0),
                    origin: Origin::PresentationalHint,
                    source_order: *source_order,
                });
            }
        }
    }

    // For td or th, inherit cellpadding from ancestor table, and valign/align from parent tr
    if tag == "td" || tag == "th" {
        // Inherit valign and align from parent tr if not explicitly declared on cell
        if let Some(parent_id) = doc.get(node_id).and_then(|n| n.parent) {
            if let Some(p_node) = doc.get(parent_id) {
                if let NodeData::Element(p_elem) = &p_node.data {
                    if p_elem.tag_name.eq_ignore_ascii_case("tr") {
                        if elem.get_attribute("valign").is_none() {
                            if let Some(valign) = p_elem.get_attribute("valign") {
                                let va = match valign.trim().to_ascii_lowercase().as_str() {
                                    "top" => Some(crate::values::VerticalAlign::Top),
                                    "middle" => Some(crate::values::VerticalAlign::Middle),
                                    "bottom" => Some(crate::values::VerticalAlign::Bottom),
                                    "baseline" => Some(crate::values::VerticalAlign::Baseline),
                                    _ => None,
                                };
                                if let Some(va_val) = va {
                                    *source_order += 1;
                                    matched_decls.push(MatchedDeclaration {
                                        declaration: Declaration::new("vertical-align", Value::VerticalAlign(va_val), false),
                                        specificity: Specificity(0, 0, 0),
                                        origin: Origin::PresentationalHint,
                                        source_order: *source_order,
                                    });
                                }
                            }
                        }
                        if elem.get_attribute("align").is_none() {
                            if let Some(align) = p_elem.get_attribute("align") {
                                let ta = match align.trim().to_ascii_lowercase().as_str() {
                                    "center" => Some(crate::values::TextAlign::Center),
                                    "right" => Some(crate::values::TextAlign::Right),
                                    "left" => Some(crate::values::TextAlign::Left),
                                    "justify" => Some(crate::values::TextAlign::Justify),
                                    _ => None,
                                };
                                if let Some(ta_val) = ta {
                                    *source_order += 1;
                                    matched_decls.push(MatchedDeclaration {
                                        declaration: Declaration::new("text-align", Value::TextAlign(ta_val), false),
                                        specificity: Specificity(0, 0, 0),
                                        origin: Origin::PresentationalHint,
                                        source_order: *source_order,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Inherit cellpadding from ancestor table
        let mut cur = node_id;
        while let Some(parent_id) = doc.get(cur).and_then(|n| n.parent) {
            if let Some(p_node) = doc.get(parent_id) {
                if let NodeData::Element(p_elem) = &p_node.data {
                    if p_elem.tag_name.eq_ignore_ascii_case("table") {
                        if let Some(cp_str) = p_elem.get_attribute("cellpadding") {
                            if let Ok(cp) = cp_str.trim().parse::<f32>() {
                                for side in &["padding-top", "padding-right", "padding-bottom", "padding-left"] {
                                    *source_order += 1;
                                    matched_decls.push(MatchedDeclaration {
                                        declaration: Declaration::new(*side, Value::Length(Length::Px(cp)), false),
                                        specificity: Specificity(0, 0, 0),
                                        origin: Origin::PresentationalHint,
                                        source_order: *source_order,
                                    });
                                }
                            }
                        }
                        break;
                    }
                }
            }
            cur = parent_id;
        }
    }
}

fn apply_style_rule(
    style_rule: &StyleRule,
    node_id: NodeId,
    doc: &Document,
    origin: Origin,
    source_order: &mut usize,
    out: &mut Vec<MatchedDeclaration>,
) {
    for complex_sel in &style_rule.selectors.selectors {
        if complex_sel.matches(node_id, doc) {
            let specificity = Specificity::of(complex_sel);
            for decl in &style_rule.declarations {
                for expanded in decl.clone().expand_shorthand() {
                    *source_order += 1;
                    out.push(MatchedDeclaration {
                        declaration: expanded,
                        specificity,
                        origin,
                        source_order: *source_order,
                    });
                }
            }
            // Only match once per style rule even if multiple selectors in the rule match
            break;
        }
    }
}

/// Color scheme preference for `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSchemePreference {
    #[default]
    Light,
    Dark,
}

/// Reduced motion preference for `prefers-reduced-motion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReducedMotionPreference {
    #[default]
    NoPreference,
    Reduce,
}

/// Contrast preference for `prefers-contrast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContrastPreference {
    #[default]
    NoPreference,
    More,
    Less,
    Custom,
}

/// Pointer accuracy type for `pointer` and `any-pointer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerType {
    #[default]
    Fine,
    Coarse,
    None,
}

/// Hover capability type for `hover` and `any-hover`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoverType {
    #[default]
    Hover,
    None,
}

/// Represents the browser/client media environment for CSS Media Queries evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEnvironment {
    pub color_scheme: ColorSchemePreference,
    pub reduced_motion: ReducedMotionPreference,
    pub contrast: ContrastPreference,
    pub hover: HoverType,
    pub any_hover: HoverType,
    pub pointer: PointerType,
    pub any_pointer: PointerType,
    pub device_pixel_ratio: f32,
}

impl Default for MediaEnvironment {
    fn default() -> Self {
        Self {
            color_scheme: ColorSchemePreference::Light,
            reduced_motion: ReducedMotionPreference::NoPreference,
            contrast: ContrastPreference::NoPreference,
            hover: HoverType::Hover,
            any_hover: HoverType::Hover,
            pointer: PointerType::Fine,
            any_pointer: PointerType::Fine,
            device_pixel_ratio: 1.0,
        }
    }
}

static GLOBAL_MEDIA_ENV: std::sync::RwLock<Option<MediaEnvironment>> = std::sync::RwLock::new(None);

/// Returns the current global media environment.
pub fn current_media_environment() -> MediaEnvironment {
    if let Ok(guard) = GLOBAL_MEDIA_ENV.read() {
        if let Some(ref env) = *guard {
            return env.clone();
        }
    }
    MediaEnvironment::default()
}

/// Updates the global media environment.
pub fn set_media_environment(env: MediaEnvironment) {
    if let Ok(mut guard) = GLOBAL_MEDIA_ENV.write() {
        *guard = Some(env);
    }
}

/// Sets the color scheme preference (`prefers-color-scheme: dark | light`).
pub fn set_prefers_color_scheme(scheme: ColorSchemePreference) {
    let mut env = current_media_environment();
    env.color_scheme = scheme;
    set_media_environment(env);
}

/// Sets the reduced motion preference (`prefers-reduced-motion: reduce | no-preference`).
pub fn set_prefers_reduced_motion(motion: ReducedMotionPreference) {
    let mut env = current_media_environment();
    env.reduced_motion = motion;
    set_media_environment(env);
}

/// Sets the contrast preference (`prefers-contrast: more | less | no-preference | custom`).
pub fn set_prefers_contrast(contrast: ContrastPreference) {
    let mut env = current_media_environment();
    env.contrast = contrast;
    set_media_environment(env);
}

/// Sets the device pixel ratio / resolution (1.0 = 1dppx / 96dpi).
pub fn set_device_pixel_ratio(dpr: f32) {
    let mut env = current_media_environment();
    env.device_pixel_ratio = dpr;
    set_media_environment(env);
}

/// Sets the pointer device capability (`pointer` and `any-pointer`).
pub fn set_pointer(pointer: PointerType) {
    let mut env = current_media_environment();
    env.pointer = pointer;
    env.any_pointer = pointer;
    set_media_environment(env);
}

/// Sets the hover capability (`hover` and `any-hover`).
pub fn set_hover(hover: HoverType) {
    let mut env = current_media_environment();
    env.hover = hover;
    env.any_hover = hover;
    set_media_environment(env);
}

/// Tests if a CSS media query matches the browser environment.
pub fn matches_media_query(query: &str, viewport_width: f32) -> bool {
    matches_media_query_size(query, viewport_width, 900.0)
}

/// Tests if a CSS media query matches the browser environment with explicit width and height.
pub fn matches_media_query_size(query: &str, viewport_width: f32, viewport_height: f32) -> bool {
    matches_media_query_env(query, viewport_width, viewport_height, &current_media_environment())
}

/// Tests if a CSS media query matches the specified media environment and viewport dimensions.
pub fn matches_media_query_env(
    query: &str,
    viewport_width: f32,
    viewport_height: f32,
    env: &MediaEnvironment,
) -> bool {
    matches_condition_internal(query, viewport_width, viewport_height, env, false)
}

/// Tests if a CSS container query condition matches given container dimensions.
pub fn matches_container_query(query: &str, container_width: f32, container_height: f32) -> bool {
    let env = current_media_environment();
    matches_condition_internal(query, container_width, container_height, &env, true)
}

/// Helper function to find container size for a node using a default or custom RuleIndex.
pub fn find_container_size_for_node(
    node_id: NodeId,
    doc: &Document,
    name_filter: Option<&str>,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(f32, f32)> {
    let index = RuleIndex::new_with_size(viewport_width, viewport_height);
    index.find_container_size_for_node(node_id, doc, name_filter)
}

fn matches_condition_internal(
    query: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    for part in q.split(',') {
        let trimmed_part = part.trim();
        if trimmed_part.contains(" or ") {
            for subpart in trimmed_part.split(" or ") {
                if matches_single_condition(subpart.trim(), width, height, env, is_container) {
                    return true;
                }
            }
        } else if matches_single_condition(trimmed_part, width, height, env, is_container) {
            return true;
        }
    }
    false
}

fn matches_single_condition(
    query: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> bool {
    let mut lower = query.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    if let Some(rest) = lower.strip_prefix("only ") {
        lower = rest.trim().to_string();
    }
    if let Some(rest) = lower.strip_prefix("not ") {
        return !matches_single_condition(rest.trim(), width, height, env, is_container);
    }

    let normalized = lower.replace(")and", ") and ").replace("and(", " and (");
    for condition in normalized.split(" and ") {
        let cond = condition.trim();
        if cond.is_empty() {
            continue;
        }

        if !is_container {
            if cond == "screen" || cond == "all" || cond == "handheld" {
                continue;
            }
            if cond == "print" || cond == "speech" {
                return false;
            }
        }

        let trimmed_cond = cond.trim();
        let inner = if trimmed_cond.starts_with('(') && trimmed_cond.ends_with(')') && trimmed_cond.len() >= 2 {
            &trimmed_cond[1..trimmed_cond.len() - 1]
        } else {
            trimmed_cond
        }.trim();

        // 1. Check if it's a range condition (contains <=, >=, <, >, =)
        if let Some(res) = eval_range_condition(inner, width, height, env, is_container) {
            if !res {
                return false;
            }
            continue;
        }

        // 2. Feature: value condition
        if let Some((name, val_str)) = inner.split_once(':') {
            if !eval_feature_condition(name.trim(), val_str.trim(), width, height, env, is_container) {
                return false;
            }
            continue;
        }

        // 3. Boolean feature query: e.g. (hover), (pointer), (color)
        if !eval_boolean_feature(inner, env) {
            return false;
        }
    }

    true
}

fn eval_boolean_feature(feature: &str, env: &MediaEnvironment) -> bool {
    match feature {
        "hover" => env.hover == HoverType::Hover,
        "any-hover" => env.any_hover == HoverType::Hover,
        "pointer" => env.pointer != PointerType::None,
        "any-pointer" => env.any_pointer != PointerType::None,
        "color" => true,
        "all" | "screen" => true,
        _ => false,
    }
}

fn eval_feature_condition(
    feature: &str,
    val_str: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    _is_container: bool,
) -> bool {
    match feature {
        "min-width" | "min-inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                width >= val - 1e-4
            } else {
                false
            }
        }
        "max-width" | "max-inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                width <= val + 1e-4
            } else {
                false
            }
        }
        "width" | "inline-size" => {
            if let Some(val) = parse_media_length(val_str) {
                (width - val).abs() < 1.0
            } else {
                false
            }
        }
        "min-height" | "min-block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                height >= val - 1e-4
            } else {
                false
            }
        }
        "max-height" | "max-block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                height <= val + 1e-4
            } else {
                false
            }
        }
        "height" | "block-size" => {
            if let Some(val) = parse_media_length(val_str) {
                (height - val).abs() < 1.0
            } else {
                false
            }
        }
        "orientation" => {
            if val_str == "landscape" {
                width >= height
            } else if val_str == "portrait" {
                width < height
            } else {
                false
            }
        }
        "aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).map_or(false, |t| (ratio - t).abs() < 1e-4)
        }
        "min-aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).map_or(false, |t| ratio >= t - 1e-4)
        }
        "max-aspect-ratio" => {
            let ratio = width / height.max(1.0);
            parse_aspect_ratio(val_str).map_or(false, |t| ratio <= t + 1e-4)
        }
        "prefers-color-scheme" => {
            if val_str.contains("dark") {
                env.color_scheme == ColorSchemePreference::Dark
            } else if val_str.contains("light") {
                env.color_scheme == ColorSchemePreference::Light
            } else {
                true
            }
        }
        "prefers-reduced-motion" => {
            if val_str.contains("reduce") {
                env.reduced_motion == ReducedMotionPreference::Reduce
            } else {
                env.reduced_motion == ReducedMotionPreference::NoPreference
            }
        }
        "prefers-contrast" => {
            match val_str {
                "more" => env.contrast == ContrastPreference::More,
                "less" => env.contrast == ContrastPreference::Less,
                "no-preference" => env.contrast == ContrastPreference::NoPreference,
                "custom" => env.contrast == ContrastPreference::Custom,
                _ => false,
            }
        }
        "hover" => {
            match val_str {
                "hover" => env.hover == HoverType::Hover,
                "none" => env.hover == HoverType::None,
                _ => false,
            }
        }
        "any-hover" => {
            match val_str {
                "hover" => env.any_hover == HoverType::Hover,
                "none" => env.any_hover == HoverType::None,
                _ => false,
            }
        }
        "pointer" => {
            match val_str {
                "fine" => env.pointer == PointerType::Fine,
                "coarse" => env.pointer == PointerType::Coarse,
                "none" => env.pointer == PointerType::None,
                _ => false,
            }
        }
        "any-pointer" => {
            match val_str {
                "fine" => env.any_pointer == PointerType::Fine,
                "coarse" => env.any_pointer == PointerType::Coarse,
                "none" => env.any_pointer == PointerType::None,
                _ => false,
            }
        }
        "resolution" | "-webkit-device-pixel-ratio" => {
            parse_resolution(val_str).map_or(false, |t| (env.device_pixel_ratio - t).abs() < 1e-4)
        }
        "min-resolution" | "-webkit-min-device-pixel-ratio" => {
            parse_resolution(val_str).map_or(false, |t| env.device_pixel_ratio >= t - 1e-4)
        }
        "max-resolution" | "-webkit-max-device-pixel-ratio" => {
            parse_resolution(val_str).map_or(false, |t| env.device_pixel_ratio <= t + 1e-4)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Le,
    Ge,
    Lt,
    Gt,
    Eq,
}

impl CmpOp {
    fn eval(self, a: f32, b: f32) -> bool {
        match self {
            CmpOp::Le => a <= b + 1e-4,
            CmpOp::Ge => a >= b - 1e-4,
            CmpOp::Lt => a < b - 1e-4,
            CmpOp::Gt => a > b + 1e-4,
            CmpOp::Eq => (a - b).abs() < 1e-4,
        }
    }
}

fn scan_operators(s: &str) -> Vec<(usize, usize, CmpOp)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut ops = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                ops.push((i, i + 2, CmpOp::Le));
                i += 2;
                continue;
            } else {
                ops.push((i, i + 1, CmpOp::Lt));
                i += 1;
                continue;
            }
        } else if bytes[i] == b'>' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                ops.push((i, i + 2, CmpOp::Ge));
                i += 2;
                continue;
            } else {
                ops.push((i, i + 1, CmpOp::Gt));
                i += 1;
                continue;
            }
        } else if bytes[i] == b'=' {
            ops.push((i, i + 1, CmpOp::Eq));
            i += 1;
            continue;
        }
        i += 1;
    }
    ops
}

fn resolve_range_operand(
    s: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    _is_container: bool,
) -> Option<f32> {
    let s_lower = s.trim().to_ascii_lowercase();
    match s_lower.as_str() {
        "width" | "inline-size" => Some(width),
        "height" | "block-size" => Some(height),
        "aspect-ratio" => Some(width / height.max(1.0)),
        "resolution" | "device-pixel-ratio" | "-webkit-device-pixel-ratio" => Some(env.device_pixel_ratio),
        _ => {
            if let Some(num_str) = s_lower.strip_suffix("vw") {
                if let Ok(val) = num_str.trim().parse::<f32>() {
                    return Some((val / 100.0) * width);
                }
            }
            if let Some(num_str) = s_lower.strip_suffix("vh") {
                if let Ok(val) = num_str.trim().parse::<f32>() {
                    return Some((val / 100.0) * height);
                }
            }
            if s_lower.contains('/') {
                let parts: Vec<&str> = s_lower.split('/').collect();
                if parts.len() == 2 {
                    let num = parts[0].trim().parse::<f32>().ok()?;
                    let den = parts[1].trim().parse::<f32>().ok()?;
                    return Some(num / den.max(1e-5));
                }
            }
            if let Some(len) = parse_media_length(s) {
                return Some(len);
            }
            if let Some(res) = parse_resolution(s) {
                return Some(res);
            }
            s.trim().parse::<f32>().ok()
        }
    }
}

fn eval_range_condition(
    inner: &str,
    width: f32,
    height: f32,
    env: &MediaEnvironment,
    is_container: bool,
) -> Option<bool> {
    let ops = scan_operators(inner);
    if ops.is_empty() {
        return None;
    }
    if ops.len() == 1 {
        let (start, end, op) = ops[0];
        let lhs_str = inner[..start].trim();
        let rhs_str = inner[end..].trim();
        let v1 = resolve_range_operand(lhs_str, width, height, env, is_container)?;
        let v2 = resolve_range_operand(rhs_str, width, height, env, is_container)?;
        Some(op.eval(v1, v2))
    } else if ops.len() == 2 {
        let (start1, end1, op1) = ops[0];
        let (start2, end2, op2) = ops[1];
        let p1 = inner[..start1].trim();
        let p2 = inner[end1..start2].trim();
        let p3 = inner[end2..].trim();
        let v1 = resolve_range_operand(p1, width, height, env, is_container)?;
        let v2 = resolve_range_operand(p2, width, height, env, is_container)?;
        let v3 = resolve_range_operand(p3, width, height, env, is_container)?;
        Some(op1.eval(v1, v2) && op2.eval(v2, v3))
    } else {
        None
    }
}

fn parse_aspect_ratio(s: &str) -> Option<f32> {
    let s = s.trim();
    if s.contains('/') {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 2 {
            let num = parts[0].trim().parse::<f32>().ok()?;
            let den = parts[1].trim().parse::<f32>().ok()?;
            return Some(num / den.max(1e-5));
        }
    }
    s.parse::<f32>().ok()
}

fn parse_resolution(s: &str) -> Option<f32> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(num_str) = s.strip_suffix("dppx") {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix('x') {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix("dpi") {
        num_str.trim().parse::<f32>().ok().map(|dpi| dpi / 96.0)
    } else if let Some(num_str) = s.strip_suffix("dpcm") {
        num_str.trim().parse::<f32>().ok().map(|dpcm| dpcm / 37.79527559)
    } else {
        s.parse::<f32>().ok()
    }
}

fn parse_media_length(s: &str) -> Option<f32> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(inner) = s.strip_prefix("calc(").and_then(|x| x.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split('-').collect();
        if parts.len() == 2 {
            let left = parse_simple_length(parts[0].trim())?;
            let right = parse_simple_length(parts[1].trim())?;
            return Some(left - right);
        }
        let parts_add: Vec<&str> = inner.split('+').collect();
        if parts_add.len() == 2 {
            let left = parse_simple_length(parts_add[0].trim())?;
            let right = parse_simple_length(parts_add[1].trim())?;
            return Some(left + right);
        }
        if let Some(val) = parse_simple_length(inner.trim()) {
            return Some(val);
        }
    }
    parse_simple_length(&s)
}

fn parse_simple_length(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(num_str) = s.strip_suffix("px") {
        num_str.trim().parse::<f32>().ok()
    } else if let Some(num_str) = s.strip_suffix("em") {
        num_str.trim().parse::<f32>().ok().map(|em| em * 16.0)
    } else if let Some(num_str) = s.strip_suffix("rem") {
        num_str.trim().parse::<f32>().ok().map(|rem| rem * 16.0)
    } else if let Some(num_str) = s.strip_suffix("pt") {
        num_str.trim().parse::<f32>().ok().map(|pt| pt * (96.0 / 72.0))
    } else if let Some(num_str) = s.strip_suffix("in") {
        num_str.trim().parse::<f32>().ok().map(|i| i * 96.0)
    } else if let Some(num_str) = s.strip_suffix("cm") {
        num_str.trim().parse::<f32>().ok().map(|cm| cm * (96.0 / 2.54))
    } else if let Some(num_str) = s.strip_suffix("mm") {
        num_str.trim().parse::<f32>().ok().map(|mm| mm * (9.6 / 2.54))
    } else if let Some(num_str) = s.strip_suffix("ch") {
        num_str.trim().parse::<f32>().ok().map(|ch| ch * 8.0)
    } else if let Some(num_str) = s.strip_suffix("ex") {
        num_str.trim().parse::<f32>().ok().map(|ex| ex * 8.0)
    } else {
        s.parse::<f32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Color;
    use mango_html::parse_html;

    #[test]
    fn test_cascade_user_agent_and_author_override() {
        let html = r#"<html><body><h1 class="heading">Mango</h1></body></html>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let h1_id = doc.find_element_by_tag(root, "h1").unwrap();

        // Without author styles, h1 gets UA styles (font-weight: bold, font-size: 32px)
        let default_styles = resolve_cascade(h1_id, &doc, &[]);
        assert_eq!(
            default_styles.get("font-weight"),
            Some(&Value::FontWeight(crate::values::FontWeight::Bold))
        );

        // Author stylesheet overrides color and font-size
        let author_css = r#"
            h1 { color: #ffa136; font-size: 40px; }
            .heading { color: #00ff00; }
        "#;
        let author_sheet = parse_stylesheet(author_css);
        let cascaded = resolve_cascade(h1_id, &doc, &[&author_sheet]);

        // .heading has higher specificity (0, 1, 0) than h1 (0, 0, 1) -> color should be green
        assert_eq!(
            cascaded.get("color"),
            Some(&Value::Color(Color::rgb(0, 255, 0)))
        );
        // font-size: 40px from h1 rule
        assert_eq!(
            cascaded.get("font-size"),
            Some(&Value::Length(crate::values::Length::Px(40.0)))
        );
    }

    #[test]
    fn test_inline_style_overrides_author() {
        let html = r#"<div id="card" style="color: blue;">Hello</div>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let div_id = doc.find_element_by_tag(root, "div").unwrap();

        let author_css = "#card { color: red; }";
        let author_sheet = parse_stylesheet(author_css);

        let cascaded = resolve_cascade(div_id, &doc, &[&author_sheet]);
        // Inline style wins over author ID selector
        assert_eq!(cascaded.get("color"), Some(&Value::Color(Color::BLUE)));
    }

    #[test]
    fn test_presentational_hints() {
        let html = r##"<table id="hnmain" width="85%" bgcolor="#f6f6ef" cellpadding="0"><tr><td bgcolor="ff6600">News</td></tr></table>"##;
        let doc = parse_html(html);
        let root = doc.root();
        let table_id = doc.find_element_by_tag(root, "table").unwrap();
        let td_id = doc.find_element_by_tag(root, "td").unwrap();

        let table_styles = resolve_cascade(table_id, &doc, &[]);
        assert_eq!(table_styles.get("width"), Some(&Value::Length(Length::Percent(85.0))));
        assert_eq!(table_styles.get("background-color"), Some(&Value::Color(Color::rgb(0xf6, 0xf6, 0xef))));

        let td_styles = resolve_cascade(td_id, &doc, &[]);
        assert_eq!(td_styles.get("background-color"), Some(&Value::Color(Color::rgb(0xff, 0x66, 0x00))));
        assert_eq!(td_styles.get("padding-top"), Some(&Value::Length(Length::Px(0.0))));
    }

    #[test]
    fn test_iframe_styles_and_presentational_hints() {
        let html = r#"<iframe id="f1" width="560" height="315" frameborder="0"></iframe>"#;
        let doc = parse_html(html);
        let root = doc.root();
        let iframe_id = doc.find_element_by_tag(root, "iframe").unwrap();

        let styles = resolve_cascade(iframe_id, &doc, &[]);
        // width and height from presentational attributes
        assert_eq!(styles.get("width"), Some(&Value::Length(Length::Px(560.0))));
        assert_eq!(styles.get("height"), Some(&Value::Length(Length::Px(315.0))));
        // frameborder="0" removes borders
        assert_eq!(styles.get("border-top-style"), Some(&Value::BorderStyle(crate::values::BorderStyle::None)));
        assert_eq!(styles.get("border-top-width"), Some(&Value::Length(Length::Px(0.0))));
        // display is inline-block from UA stylesheet
        assert_eq!(styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
    }

    #[test]
    fn test_video_and_audio_styles_and_presentational_hints() {
        let html = r#"
            <video id="v1" width="640" height="360"></video>
            <audio id="a1"></audio>
            <audio id="a2" controls></audio>
        "#;
        let doc = parse_html(html);
        let root = doc.root();

        // 1. Video has default inline-block and presentational width/height
        let v1_id = doc.find_element_by_tag(root, "video").unwrap();
        let v1_styles = resolve_cascade(v1_id, &doc, &[]);
        assert_eq!(v1_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(v1_styles.get("width"), Some(&Value::Length(Length::Px(640.0))));
        assert_eq!(v1_styles.get("height"), Some(&Value::Length(Length::Px(360.0))));

        // 2. Audio without controls is display: none
        let a1_id = doc.find_element_by_id(root, "a1").unwrap();
        let a1_styles = resolve_cascade(a1_id, &doc, &[]);
        assert_eq!(a1_styles.get("display"), Some(&Value::Keyword("none".to_string())));

        // 3. Audio with controls is display: inline-block with 300x36 dimensions
        let a2_id = doc.find_element_by_id(root, "a2").unwrap();
        let a2_styles = resolve_cascade(a2_id, &doc, &[]);
        assert_eq!(a2_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(a2_styles.get("width"), Some(&Value::Length(Length::Px(300.0))));
        assert_eq!(a2_styles.get("height"), Some(&Value::Length(Length::Px(36.0))));
    }

    #[test]
    fn test_canvas_ua_styles_and_presentational_hints() {
        let html = r#"
            <canvas id="c1"></canvas>
            <canvas id="c2" width="500" height="250"></canvas>
        "#;
        let doc = parse_html(html);
        let root = doc.root();

        // 1. Default canvas: inline-block, 300x150
        let c1_id = doc.find_element_by_id(root, "c1").unwrap();
        let c1_styles = resolve_cascade(c1_id, &doc, &[]);
        assert_eq!(c1_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(c1_styles.get("width"), Some(&Value::Length(Length::Px(300.0))));
        assert_eq!(c1_styles.get("height"), Some(&Value::Length(Length::Px(150.0))));

        // 2. Canvas with presentational hints
        let c2_id = doc.find_element_by_id(root, "c2").unwrap();
        let c2_styles = resolve_cascade(c2_id, &doc, &[]);
        assert_eq!(c2_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(c2_styles.get("width"), Some(&Value::Length(Length::Px(500.0))));
        assert_eq!(c2_styles.get("height"), Some(&Value::Length(Length::Px(250.0))));
    }

    #[test]
    fn test_rule_index_bucketing_and_resolution() {
        let css = r#"
            * { margin: 0px; }
            div { color: red; }
            .btn { background-color: blue; }
            #main { font-size: 20px; }
        "#;
        let sheet = parse_stylesheet(css);
        let mut order = 0;
        let index = RuleIndex::from_stylesheets(&[&sheet], Origin::Author, &mut order);

        // Verify bucketing
        assert_eq!(index.universal_rules.len(), 1);
        assert!(index.tag_rules.contains_key("div"));
        assert!(index.class_rules.contains_key("btn"));
        assert!(index.id_rules.contains_key("main"));

        let doc = parse_html(r#"<div id="main" class="btn">Hello</div>"#);
        let div_id = doc.find_element_by_tag(doc.root(), "div").unwrap();
        let styles = resolve_cascade_with_index(div_id, &doc, &index);

        assert_eq!(styles.get("margin-top"), Some(&Value::Length(Length::Px(0.0))));
        assert_eq!(styles.get("color"), Some(&Value::Color(Color::RED)));
        assert_eq!(styles.get("background-color"), Some(&Value::Color(Color::BLUE)));
        assert_eq!(styles.get("font-size"), Some(&Value::Length(Length::Px(20.0))));
    }

    #[test]
    fn test_element_styles() {
        let html = r#"
            <dialog id="d1">Closed</dialog>
            <dialog id="d2" open>Open</dialog>
            <progress id="p1" value="50" max="100"></progress>
            <meter id="m1" value="0.7"></meter>
            <output id="o1">42</output>
            <datalist id="dl1"><option value="A"></option></datalist>
            <ruby id="rb1">漢<rt id="rt1">かん</rt><rp id="rp1">(</rp></ruby>
            <bdi id="bdi1">auto</bdi>
            <bdo id="bdo1" dir="rtl">reversed</bdo>
        "#;
        let doc = parse_html(html);
        let root = doc.root();

        // 1. dialog: closed is display: none, open is display: block
        let d1_id = doc.find_element_by_id(root, "d1").unwrap();
        let d1_styles = resolve_cascade(d1_id, &doc, &[]);
        assert!(matches!(d1_styles.get("display"), Some(Value::Display(crate::values::Display::None))) || d1_styles.get("display") == Some(&Value::Keyword("none".to_string())));

        let d2_id = doc.find_element_by_id(root, "d2").unwrap();
        let d2_styles = resolve_cascade(d2_id, &doc, &[]);
        assert_eq!(d2_styles.get("display"), Some(&Value::Display(crate::values::Display::Block)));

        // 2. progress & meter dimensions
        let p1_id = doc.find_element_by_id(root, "p1").unwrap();
        let p1_styles = resolve_cascade(p1_id, &doc, &[]);
        assert_eq!(p1_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(p1_styles.get("width"), Some(&Value::Length(Length::Px(160.0))));
        assert_eq!(p1_styles.get("height"), Some(&Value::Length(Length::Px(16.0))));

        let m1_id = doc.find_element_by_id(root, "m1").unwrap();
        let m1_styles = resolve_cascade(m1_id, &doc, &[]);
        assert_eq!(m1_styles.get("display"), Some(&Value::Display(crate::values::Display::InlineBlock)));
        assert_eq!(m1_styles.get("width"), Some(&Value::Length(Length::Px(80.0))));
        assert_eq!(m1_styles.get("height"), Some(&Value::Length(Length::Px(16.0))));

        // 3. datalist is display: none
        let dl1_id = doc.find_element_by_id(root, "dl1").unwrap();
        let dl1_styles = resolve_cascade(dl1_id, &doc, &[]);
        assert!(matches!(dl1_styles.get("display"), Some(Value::Display(crate::values::Display::None))) || dl1_styles.get("display") == Some(&Value::Keyword("none".to_string())));

        // 4. ruby: rp is display: none, rt is font-size 50%
        let rt1_id = doc.find_element_by_id(root, "rt1").unwrap();
        let rt1_styles = resolve_cascade(rt1_id, &doc, &[]);
        assert_eq!(rt1_styles.get("font-size"), Some(&Value::Length(Length::Percent(50.0))));

        let rp1_id = doc.find_element_by_id(root, "rp1").unwrap();
        let rp1_styles = resolve_cascade(rp1_id, &doc, &[]);
        assert!(matches!(rp1_styles.get("display"), Some(Value::Display(crate::values::Display::None))) || rp1_styles.get("display") == Some(&Value::Keyword("none".to_string())));

        // 5. bdi & bdo unicode-bidi and direction
        let bdi1_id = doc.find_element_by_id(root, "bdi1").unwrap();
        let bdi1_styles = resolve_cascade(bdi1_id, &doc, &[]);
        assert_eq!(bdi1_styles.get("unicode-bidi"), Some(&Value::UnicodeBidi(crate::values::UnicodeBidi::Isolate)));

        let bdo1_id = doc.find_element_by_id(root, "bdo1").unwrap();
        let bdo1_styles = resolve_cascade(bdo1_id, &doc, &[]);
        assert_eq!(bdo1_styles.get("unicode-bidi"), Some(&Value::UnicodeBidi(crate::values::UnicodeBidi::BidiOverride)));
        assert_eq!(bdo1_styles.get("direction"), Some(&Value::Direction(crate::values::Direction::Rtl)));
    }

    #[test]
    fn test_media_query_calc() {
        assert!(!matches_media_query_size("screen and (max-width:calc(1120px - 1px))", 1280.0, 900.0));
        assert!(matches_media_query_size("screen and (max-width:calc(1120px - 1px))", 1000.0, 900.0));
        assert!(matches_media_query_size("screen and (min-width: 60em)", 1280.0, 900.0));
        assert!(!matches_media_query_size("screen and (min-width: 60em)", 800.0, 900.0));
    }

    #[test]
    fn test_media_and_container_queries() {
        // 1. prefers-color-scheme
        set_prefers_color_scheme(ColorSchemePreference::Dark);
        assert!(matches_media_query_size("(prefers-color-scheme: dark)", 1000.0, 800.0));
        assert!(!matches_media_query_size("(prefers-color-scheme: light)", 1000.0, 800.0));

        set_prefers_color_scheme(ColorSchemePreference::Light);
        assert!(!matches_media_query_size("(prefers-color-scheme: dark)", 1000.0, 800.0));
        assert!(matches_media_query_size("(prefers-color-scheme: light)", 1000.0, 800.0));

        // 2. prefers-reduced-motion
        set_prefers_reduced_motion(ReducedMotionPreference::Reduce);
        assert!(matches_media_query_size("(prefers-reduced-motion: reduce)", 1000.0, 800.0));
        assert!(!matches_media_query_size("(prefers-reduced-motion: no-preference)", 1000.0, 800.0));
        set_prefers_reduced_motion(ReducedMotionPreference::NoPreference);

        // 3. prefers-contrast
        set_prefers_contrast(ContrastPreference::More);
        assert!(matches_media_query_size("(prefers-contrast: more)", 1000.0, 800.0));
        assert!(!matches_media_query_size("(prefers-contrast: less)", 1000.0, 800.0));
        set_prefers_contrast(ContrastPreference::NoPreference);

        // 4. hover and pointer
        set_hover(HoverType::Hover);
        set_pointer(PointerType::Fine);
        assert!(matches_media_query_size("(hover: hover)", 1000.0, 800.0));
        assert!(matches_media_query_size("(hover)", 1000.0, 800.0));
        assert!(matches_media_query_size("(pointer: fine)", 1000.0, 800.0));
        assert!(matches_media_query_size("(pointer)", 1000.0, 800.0));

        // 5. resolution / device-pixel-ratio
        set_device_pixel_ratio(2.0);
        assert!(matches_media_query_size("(min-resolution: 2dppx)", 1000.0, 800.0));
        assert!(matches_media_query_size("(min-resolution: 192dpi)", 1000.0, 800.0));
        assert!(matches_media_query_size("(-webkit-min-device-pixel-ratio: 2)", 1000.0, 800.0));
        assert!(!matches_media_query_size("(min-resolution: 3dppx)", 1000.0, 800.0));
        set_device_pixel_ratio(1.0);

        // 6. Media Queries Level 4 range comparison syntax
        assert!(matches_media_query_size("(width >= 600px)", 800.0, 600.0));
        assert!(!matches_media_query_size("(width >= 900px)", 800.0, 600.0));
        assert!(matches_media_query_size("(600px <= width <= 1000px)", 800.0, 600.0));
        assert!(!matches_media_query_size("(600px <= width <= 750px)", 800.0, 600.0));
        assert!(matches_media_query_size("(aspect-ratio >= 4/3)", 800.0, 600.0));

        // 7. Logical 'or' and 'not'
        assert!(matches_media_query_size("(max-width: 500px) or (min-width: 700px)", 800.0, 600.0));
        assert!(!matches_media_query_size("(max-width: 500px) or (min-width: 900px)", 800.0, 600.0));
        assert!(matches_media_query_size("not (max-width: 500px)", 800.0, 600.0));

        // 8. Container queries matching and cascade resolution
        assert!(matches_container_query("(min-width: 400px)", 500.0, 300.0));
        assert!(!matches_container_query("(min-width: 600px)", 500.0, 300.0));
        assert!(matches_container_query("(inline-size >= 400px)", 500.0, 300.0));
        assert!(matches_container_query("(300px <= width <= 600px)", 500.0, 300.0));

        // 9. Cascade resolution with @container rule
        let html = r#"
            <div id="container" style="container-type: inline-size; width: 500px;">
                <div id="target" class="card">Hello</div>
            </div>
        "#;
        let doc = parse_html(html);
        let root = doc.root();
        let target_id = doc.find_element_by_id(root, "target").unwrap();

        let css = r#"
            .card { color: red; }
            @container (min-width: 400px) {
                .card { color: green; font-size: 20px; }
            }
            @container (min-width: 800px) {
                .card { color: blue; }
            }
        "#;
        let sheet = parse_stylesheet(css);
        let cascaded = resolve_cascade(target_id, &doc, &[&sheet]);

        // Container is 500px, so min-width: 400px matches (color: green), min-width: 800px does NOT match
        assert_eq!(cascaded.get("color"), Some(&Value::Color(Color::rgb(0, 128, 0))));
        assert_eq!(cascaded.get("font-size"), Some(&Value::Length(Length::Px(20.0))));
    }
}


