use cssparser::ToCss;
use mango_core::Atom;
use mango_html::dom::{Document, NodeData, NodeId};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::matching::{ElementSelectorFlags, MatchingContext};
use selectors::parser::SelectorImpl;
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;

/// Transparent wrapper around `Atom` implementing Servo `SelectorImpl` bounds.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct CssAtom(pub Atom);

impl Deref for CssAtom {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl AsRef<str> for CssAtom {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for CssAtom {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for CssAtom {
    fn from(s: &str) -> Self {
        Self(Atom::from(s))
    }
}

impl ToCss for CssAtom {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        dest.write_str(self.0.as_str())
    }
}

impl PrecomputedHash for CssAtom {
    fn precomputed_hash(&self) -> u32 {
        self.0.0.precomputed_hash()
    }
}

/// SelectorImpl implementation for Mango browser engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MangoSelectorImpl;

impl SelectorImpl for MangoSelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = CssAtom;
    type Identifier = CssAtom;
    type LocalName = CssAtom;
    type NamespaceUrl = CssAtom;
    type NamespacePrefix = CssAtom;
    type BorrowedNamespaceUrl = str;
    type BorrowedLocalName = str;
    type NonTSPseudoClass = NonTSPseudoClass;
    type PseudoElement = PseudoElement;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonTSPseudoClass(pub String);

impl selectors::parser::NonTSPseudoClass for NonTSPseudoClass {
    fn is_active_or_hover(&self) -> bool {
        self.0 == "active" || self.0 == "hover"
    }

    fn is_user_action_state(&self) -> bool {
        matches!(
            self.0.as_str(),
            "active" | "hover" | "focus" | "focus-visible"
        )
    }
}

impl ToCss for NonTSPseudoClass {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        write!(dest, ":{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PseudoElement(pub String);

impl selectors::parser::PseudoElement for PseudoElement {}

impl ToCss for PseudoElement {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        write!(dest, "::{}", self.0)
    }
}

/// Lightweight DOM element adapter for Servo selector matching.
#[derive(Copy, Clone, Debug)]
pub struct MangoElement<'a> {
    pub node_id: NodeId,
    pub doc: &'a Document,
}

impl<'a> selectors::Element for MangoElement<'a> {
    type Impl = MangoSelectorImpl;

    fn opaque(&self) -> selectors::OpaqueElement {
        let node = self.doc.get(self.node_id).expect("Node must exist");
        selectors::OpaqueElement::new(node)
    }

    fn parent_element(&self) -> Option<Self> {
        crate::selectors::nav::parent_element(self.node_id, self.doc).map(|id| MangoElement {
            node_id: id,
            doc: self.doc,
        })
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        if let Some(parent) = self.doc.get(self.node_id).and_then(|n| n.parent) {
            if let Some(node) = self.doc.get(parent) {
                return matches!(node.data, NodeData::DocumentFragment);
            }
        }
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        self.doc
            .get_enclosing_shadow_root(self.node_id)
            .and_then(|sr| self.doc.get_host_for_shadow_root(sr))
            .map(|id| MangoElement {
                node_id: id,
                doc: self.doc,
            })
    }

    fn assigned_slot(&self) -> Option<Self> {
        let node = self.doc.get(self.node_id)?;
        let parent_id = node.parent?;
        let parent_node = self.doc.get(parent_id)?;
        if let NodeData::Element(parent_elem) = &parent_node.data {
            if parent_elem.tag_name.eq_ignore_ascii_case("slot") {
                return Some(MangoElement {
                    node_id: parent_id,
                    doc: self.doc,
                });
            }
            if let Some(sr_id) = parent_elem.shadow_root {
                let child_slot = if let NodeData::Element(el) = &node.data {
                    el.get_attribute("slot").unwrap_or("")
                } else {
                    ""
                };
                let mut stack = vec![sr_id];
                while let Some(curr) = stack.pop() {
                    if let Some(curr_node) = self.doc.get(curr) {
                        if let NodeData::Element(curr_el) = &curr_node.data {
                            if curr_el.tag_name.eq_ignore_ascii_case("template") {
                                continue;
                            }
                            if curr_el.tag_name.eq_ignore_ascii_case("slot") {
                                let name = curr_el.get_attribute("name").unwrap_or("");
                                if name == child_slot {
                                    return Some(MangoElement {
                                        node_id: curr,
                                        doc: self.doc,
                                    });
                                }
                            }
                        }
                        let mut children: Vec<NodeId> =
                            self.doc.children(curr).map(|c| c.id).collect();
                        children.reverse();
                        stack.extend(children);
                    }
                }
            }
        }
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        crate::selectors::nav::prev_element_sibling(self.node_id, self.doc).map(|id| MangoElement {
            node_id: id,
            doc: self.doc,
        })
    }

    fn next_sibling_element(&self) -> Option<Self> {
        crate::selectors::nav::next_element_sibling(self.node_id, self.doc).map(|id| MangoElement {
            node_id: id,
            doc: self.doc,
        })
    }

    fn first_element_child(&self) -> Option<Self> {
        self.doc.children(self.node_id).find_map(|child| {
            if matches!(child.data, NodeData::Element(_)) {
                Some(MangoElement {
                    node_id: child.id,
                    doc: self.doc,
                })
            } else {
                None
            }
        })
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(&self, local_name: &<Self::Impl as SelectorImpl>::BorrowedLocalName) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        elem.tag_name.eq_ignore_ascii_case(local_name)
            || elem
                .tag_name
                .split_once(':')
                .is_some_and(|(_, local)| local.eq_ignore_ascii_case(local_name))
    }

    fn has_namespace(&self, ns: &<Self::Impl as SelectorImpl>::BorrowedNamespaceUrl) -> bool {
        ns.is_empty()
    }

    fn is_same_type(&self, other: &Self) -> bool {
        let (Some(n1), Some(n2)) = (self.doc.get(self.node_id), self.doc.get(other.node_id)) else {
            return false;
        };
        let (NodeData::Element(e1), NodeData::Element(e2)) = (&n1.data, &n2.data) else {
            return false;
        };
        e1.tag_name == e2.tag_name
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&<Self::Impl as SelectorImpl>::NamespaceUrl>,
        local_name: &<Self::Impl as SelectorImpl>::LocalName,
        operation: &AttrSelectorOperation<&<Self::Impl as SelectorImpl>::AttrValue>,
    ) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        if let Some(val) = elem.get_attribute(local_name.as_ref()) {
            operation.eval_str(val)
        } else if matches!(ns, NamespaceConstraint::Any) {
            for (k, v) in &elem.attributes {
                if k.split_once(':')
                    .is_some_and(|(_, l)| l.eq_ignore_ascii_case(local_name.as_ref()))
                {
                    return operation.eval_str(v);
                }
            }
            false
        } else {
            false
        }
    }

    fn match_non_ts_pseudo_class(
        &self,
        pc: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        crate::selectors::pseudo::matches_pseudo_class(&pc.0, elem, self.node_id, node, self.doc)
    }

    fn match_pseudo_element(
        &self,
        pe: &<Self::Impl as SelectorImpl>::PseudoElement,
        _context: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        let canon = crate::selectors::types::canonical_pseudo(&pe.0);
        if canon == "marker" {
            let Some(node) = self.doc.get(self.node_id) else {
                return false;
            };
            if let NodeData::Element(el) = &node.data {
                return el.tag_name.eq_ignore_ascii_case("li")
                    || el.get_attribute("data-mango-display") == Some("list-item");
            }
        }
        true
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        (elem.tag_name == "a" || elem.tag_name == "area" || elem.tag_name == "link")
            && elem.get_attribute("href").is_some()
    }

    fn is_html_slot_element(&self) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        elem.tag_name.eq_ignore_ascii_case("slot")
    }

    fn has_id(
        &self,
        id: &<Self::Impl as SelectorImpl>::Identifier,
        case_sensitivity: CaseSensitivity,
    ) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        match elem.id() {
            Some(elem_id) => case_sensitivity.eq(elem_id.as_bytes(), id.as_bytes()),
            None => false,
        }
    }

    fn has_class(
        &self,
        name: &<Self::Impl as SelectorImpl>::Identifier,
        case_sensitivity: CaseSensitivity,
    ) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        elem.classes()
            .any(|c| case_sensitivity.eq(c.as_bytes(), name.as_bytes()))
    }

    fn has_custom_state(&self, _name: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        false
    }

    fn imported_part(
        &self,
        _name: &<Self::Impl as SelectorImpl>::Identifier,
    ) -> Option<<Self::Impl as SelectorImpl>::Identifier> {
        None
    }

    fn is_part(&self, name: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        if let Some(part_attr) = elem.get_attribute("part") {
            part_attr
                .split_whitespace()
                .any(|p| p.eq_ignore_ascii_case(name.as_ref()))
        } else {
            false
        }
    }

    fn is_empty(&self) -> bool {
        for child in self.doc.children(self.node_id) {
            match &child.data {
                NodeData::Element(_) => return false,
                NodeData::Text(t) if !t.is_empty() => return false,
                _ => {}
            }
        }
        true
    }

    fn is_root(&self) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        elem.tag_name.eq_ignore_ascii_case("html")
    }

    fn add_element_unique_hashes(&self, filter: &mut BloomFilter) -> bool {
        let Some(node) = self.doc.get(self.node_id) else {
            return false;
        };
        let NodeData::Element(elem) = &node.data else {
            return false;
        };
        let mut added = false;
        let tag = elem.tag_name.as_str();
        if tag.bytes().all(|b| !b.is_ascii_uppercase()) {
            filter.insert_hash(elem.tag_name.0.precomputed_hash());
            added = true;
        }
        if let Some(id) = elem.id() {
            filter.insert_hash(precomputed_hash::PrecomputedHash::precomputed_hash(
                &string_cache::DefaultAtom::from(id),
            ));
            added = true;
        }
        for class in elem.classes() {
            filter.insert_hash(precomputed_hash::PrecomputedHash::precomputed_hash(
                &string_cache::DefaultAtom::from(class),
            ));
            added = true;
        }
        added
    }
}

/// Selector parser using Servo's parser engine.
pub struct MangoSelectorParser;

impl<'i> selectors::parser::Parser<'i> for MangoSelectorParser {
    type Impl = MangoSelectorImpl;
    type Error = selectors::parser::SelectorParseErrorKind;

    fn parse_slotted(&self) -> bool {
        true
    }

    fn parse_part(&self) -> bool {
        true
    }

    fn parse_nth_child_of(&self) -> bool {
        true
    }

    fn parse_is_and_where(&self) -> bool {
        true
    }

    fn parse_has(&self) -> bool {
        true
    }

    fn parse_parent_selector(&self) -> bool {
        true
    }

    fn parse_host(&self) -> bool {
        true
    }

    fn parse_non_ts_pseudo_class(
        &self,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<NonTSPseudoClass, cssparser::ParseError<Self::Error>> {
        Ok(NonTSPseudoClass(name.to_string()))
    }

    fn parse_non_ts_functional_pseudo_class(
        &self,
        name: cssparser::CowRcStr<'i>,
        parser: &mut cssparser::Parser<'i>,
        _after_part: bool,
    ) -> Result<NonTSPseudoClass, cssparser::ParseError<Self::Error>> {
        let mut args = String::new();
        while let Ok(tok) = parser.next() {
            args.push_str(&tok.to_css_string());
        }
        Ok(NonTSPseudoClass(format!("{}({})", name, args)))
    }

    fn parse_pseudo_element(
        &self,
        name: cssparser::CowRcStr<'i>,
    ) -> Result<PseudoElement, cssparser::ParseError<Self::Error>> {
        Ok(PseudoElement(name.to_string()))
    }

    fn parse_functional_pseudo_element(
        &self,
        name: cssparser::CowRcStr<'i>,
        parser: &mut cssparser::Parser<'i>,
    ) -> Result<PseudoElement, cssparser::ParseError<Self::Error>> {
        let mut args = String::new();
        while let Ok(tok) = parser.next() {
            args.push_str(&tok.to_css_string());
        }
        Ok(PseudoElement(format!("{}({})", name, args)))
    }
}

/// Precomputed Servo selector AST and ancestor hashes for fast Bloom-filter matching.
#[derive(Clone, Debug)]
pub struct ServoSelectorData {
    pub selector: selectors::parser::Selector<MangoSelectorImpl>,
    pub ancestor_hashes: selectors::parser::AncestorHashes,
}

impl ServoSelectorData {
    pub fn try_from_selector_str(sel_str: &str) -> Option<Self> {
        let list = parse_selector_list(sel_str).ok()?;
        let first = list.slice().first()?.clone();
        let ancestor_hashes = selectors::parser::AncestorHashes::new(
            &first,
            selectors::matching::QuirksMode::NoQuirks,
        );
        Some(Self {
            selector: first,
            ancestor_hashes,
        })
    }
}

/// Helper function to parse a selector string using Servo's engine into `selectors::SelectorList`.
pub fn parse_selector_list(
    input: &str,
) -> Result<selectors::SelectorList<MangoSelectorImpl>, String> {
    let mut parser = cssparser::Parser::new(input);
    selectors::SelectorList::parse(
        &MangoSelectorParser,
        &mut parser,
        selectors::parser::ParseRelative::No,
    )
    .map_err(|e| format!("{:?}", e))
}

/// Matches a DOM element using Servo's selector matching engine and optional Bloom filter.
pub fn matches_selector_servo(
    selector: &selectors::parser::Selector<MangoSelectorImpl>,
    node_id: NodeId,
    doc: &Document,
    bloom_filter: Option<&BloomFilter>,
) -> bool {
    let elem = MangoElement { node_id, doc };
    let mut caches = selectors::matching::SelectorCaches::default();
    let mut context = MatchingContext::new(
        selectors::matching::MatchingMode::Normal,
        bloom_filter,
        &mut caches,
        selectors::matching::QuirksMode::NoQuirks,
        selectors::matching::NeedsSelectorFlags::No,
        selectors::matching::MatchingForInvalidation::No,
    );
    if let Some(scope_id) = crate::selectors::types::current_scope_root() {
        if let Some(node) = doc.get(scope_id) {
            context.scope_element = Some(selectors::OpaqueElement::new(node));
        }
    }
    if let Some(host_id) = crate::selectors::types::current_host_element() {
        if let Some(node) = doc.get(host_id) {
            context.current_host = Some(selectors::OpaqueElement::new(node));
        }
    }
    selectors::matching::matches_selector(selector, 0, None, &elem, &mut context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use selectors::Element;

    #[test]
    fn test_servo_selector_parsing_and_matching() {
        let mut doc = Document::new();
        let body = doc.create_element("body", vec![]);
        let div = doc.create_element(
            "div",
            vec![
                ("class".into(), "card main".to_string()),
                ("id".into(), "hero".to_string()),
            ],
        );
        let span = doc.create_element("span", vec![("data-active".into(), "true".to_string())]);
        doc.append_child(body, div);
        doc.append_child(div, span);

        let list = parse_selector_list("div.card#hero > span[data-active='true']").unwrap();
        assert_eq!(list.slice().len(), 1);
        let sel = &list.slice()[0];
        assert!(matches_selector_servo(sel, span, &doc, None));
        assert!(!matches_selector_servo(sel, div, &doc, None));

        let fail_list = parse_selector_list("div.nonexistent").unwrap();
        assert!(!matches_selector_servo(
            &fail_list.slice()[0],
            div,
            &doc,
            None
        ));

        // Test BloomFilter fast ancestor rejection:
        let mut filter = BloomFilter::default();
        let body_elem = MangoElement {
            node_id: body,
            doc: &doc,
        };
        let div_elem = MangoElement {
            node_id: div,
            doc: &doc,
        };
        body_elem.add_element_unique_hashes(&mut filter);
        div_elem.add_element_unique_hashes(&mut filter);

        assert!(matches_selector_servo(sel, span, &doc, Some(&filter)));

        // An empty filter should fast-reject selector with ancestor requirements
        let empty_filter = BloomFilter::default();
        let hashes =
            selectors::parser::AncestorHashes::new(sel, selectors::matching::QuirksMode::NoQuirks);
        let mut caches = selectors::matching::SelectorCaches::default();
        let mut ctx = MatchingContext::new(
            selectors::matching::MatchingMode::Normal,
            Some(&empty_filter),
            &mut caches,
            selectors::matching::QuirksMode::NoQuirks,
            selectors::matching::NeedsSelectorFlags::No,
            selectors::matching::MatchingForInvalidation::No,
        );
        let matched = selectors::matching::matches_selector(
            sel,
            0,
            Some(&hashes),
            &MangoElement {
                node_id: span,
                doc: &doc,
            },
            &mut ctx,
        );
        assert!(
            !matched,
            "BloomFilter must fast-reject selector when ancestors are not present in filter"
        );

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<selectors::parser::Selector<MangoSelectorImpl>>();
        assert_send_sync::<selectors::parser::AncestorHashes>();
    }
}
