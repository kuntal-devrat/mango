use mango_html::dom::NodeData;
use mango_html::parse_html;

use crate::selectors::nth::matches_an_plus_b;
use crate::selectors::types::{Combinator, ComplexSelector, CompoundSelector, SimpleSelector};

#[test]
fn test_selector_matching_basic() {
    let html = r#"<div class="card" id="main"><p class="text">Hello</p></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let div_id = doc.find_element_by_tag(root, "div").unwrap();
    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    // Type selector
    assert!(SimpleSelector::Type("div".to_string()).matches(div_id, &doc));
    assert!(!SimpleSelector::Type("span".to_string()).matches(div_id, &doc));

    // Class selector
    assert!(SimpleSelector::Class("card".to_string()).matches(div_id, &doc));
    assert!(SimpleSelector::Class("text".to_string()).matches(p_id, &doc));

    // ID selector
    assert!(SimpleSelector::Id("main".to_string()).matches(div_id, &doc));

    // Compound selector: div.card#main
    let compound = CompoundSelector::new(vec![
        SimpleSelector::Type("div".to_string()),
        SimpleSelector::Class("card".to_string()),
        SimpleSelector::Id("main".to_string()),
    ]);
    assert!(compound.matches(div_id, &doc));
}

#[test]
fn test_complex_selector_child_and_descendant() {
    let html = r#"<div class="wrapper"><div class="inner"><p>Text</p></div></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let p_id = doc.find_element_by_tag(root, "p").unwrap();

    // div.inner > p
    let child_sel = ComplexSelector::new(
        CompoundSelector::new(vec![
            SimpleSelector::Type("div".to_string()),
            SimpleSelector::Class("inner".to_string()),
        ]),
        vec![(
            Combinator::Child,
            CompoundSelector::new(vec![SimpleSelector::Type("p".to_string())]),
        )],
    );
    assert!(child_sel.matches(p_id, &doc));

    // div.wrapper p (descendant)
    let desc_sel = ComplexSelector::new(
        CompoundSelector::new(vec![
            SimpleSelector::Type("div".to_string()),
            SimpleSelector::Class("wrapper".to_string()),
        ]),
        vec![(
            Combinator::Descendant,
            CompoundSelector::new(vec![SimpleSelector::Type("p".to_string())]),
        )],
    );
    assert!(desc_sel.matches(p_id, &doc));
}

#[test]
fn test_compound_class_descendant() {
    let html =
        r#"<div class="mw-parser-output"><div class="hlist inline" id="target"></div></div>"#;
    let doc = mango_html::parse_html(html);
    let target_id = doc.find_element_by_id(doc.root(), "target").unwrap();
    let sel = crate::parser::parse_selectors(".mw-parser-output .hlist.inline").unwrap();
    assert!(
        sel.matches(target_id, &doc),
        ".mw-parser-output .hlist.inline must match <div class='hlist inline'>"
    );
}

#[test]
fn test_an_plus_b_matching() {
    assert!(matches_an_plus_b("odd", 1));
    assert!(!matches_an_plus_b("odd", 2));
    assert!(matches_an_plus_b("even", 2));
    assert!(!matches_an_plus_b("even", 3));
    assert!(matches_an_plus_b("2n+1", 1));
    assert!(!matches_an_plus_b("2n+1", 2));
    assert!(matches_an_plus_b("2n+1", 3));
    assert!(matches_an_plus_b("2n", 2));
    assert!(!matches_an_plus_b("2n", 3));
    assert!(matches_an_plus_b("2n", 4));
    assert!(matches_an_plus_b("-n+3", 1));
    assert!(matches_an_plus_b("-n+3", 2));
    assert!(matches_an_plus_b("-n+3", 3));
    assert!(!matches_an_plus_b("-n+3", 4));
    assert!(matches_an_plus_b("3", 3));
    assert!(!matches_an_plus_b("3", 2));
}

#[test]
fn test_attribute_operators_dashmatch_empty_and_case_insensitive() {
    let html = r#"
        <div id="en" lang="en"></div>
        <div id="en-us" lang="en-US"></div>
        <div id="english" lang="english"></div>
        <input id="chk" type="checkbox">
    "#;
    let doc = mango_html::tree_builder::parse_html(html);
    let root = doc.root();
    let el = |id: &str| doc.find_element_by_id(root, id).unwrap();

    // DashMatch: [lang|="en"]
    let dash_sel = crate::parser::parse_selectors("[lang|=\"en\"]").unwrap();
    assert!(
        dash_sel.matches(el("en"), &doc),
        "[lang|='en'] matches 'en'"
    );
    assert!(
        dash_sel.matches(el("en-us"), &doc),
        "[lang|='en'] matches 'en-US'"
    );
    assert!(
        !dash_sel.matches(el("english"), &doc),
        "[lang|='en'] does not match 'english'"
    );

    // Empty string matches should return false
    let empty_prefix = crate::parser::parse_selectors("[lang^=\"\"]").unwrap();
    assert!(
        !empty_prefix.matches(el("en"), &doc),
        "[attr^=''] must never match"
    );
    let empty_suffix = crate::parser::parse_selectors("[lang$=\"\"]").unwrap();
    assert!(
        !empty_suffix.matches(el("en"), &doc),
        "[attr$=''] must never match"
    );
    let empty_sub = crate::parser::parse_selectors("[lang*=\"\"]").unwrap();
    assert!(
        !empty_sub.matches(el("en"), &doc),
        "[attr*=''] must never match"
    );

    // Case-insensitive attribute match: [type="CHECKBOX" i]
    let case_sel = crate::parser::parse_selectors("[type=\"CHECKBOX\" i]").unwrap();
    assert!(
        case_sel.matches(el("chk"), &doc),
        "[type='CHECKBOX' i] matches 'checkbox'"
    );
}

#[test]
fn test_pseudo_class_link_and_visited() {
    let html = r#"<p><a href="https://example.com">Link</a><a>Not a link</a></p>"#;
    let doc = parse_html(html);
    let root = doc.root();

    // Find the p element
    let p_id = doc.find_element_by_tag(root, "p").unwrap();
    let p_children = doc.children(p_id);

    let mut link_with_href = None;
    let mut link_without_href = None;
    for child in p_children {
        if let NodeData::Element(el) = &child.data
            && el.tag_name == "a"
        {
            if el.get_attribute("href").is_some() {
                link_with_href = Some(child.id);
            } else {
                link_without_href = Some(child.id);
            }
        }
    }

    let link_sel = SimpleSelector::PseudoClass("link".to_string());
    assert!(link_sel.matches(link_with_href.unwrap(), &doc));
    assert!(!link_sel.matches(link_without_href.unwrap(), &doc));
}

#[test]
fn test_pseudo_element_selector_isolation() {
    let html = r#"<div class="clearfix">Content</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let pseudo_sel = ComplexSelector::new(
        CompoundSelector::new(vec![
            SimpleSelector::Class("clearfix".to_string()),
            SimpleSelector::PseudoElement("after".to_string()),
        ]),
        vec![],
    );

    // Crucial test: .clearfix::after MUST NOT match the div itself!
    assert!(!pseudo_sel.matches(div_id, &doc));

    // But it MUST match when querying the pseudo-element!
    assert!(pseudo_sel.matches_pseudo_element(div_id, &doc, "after"));
    assert!(!pseudo_sel.matches_pseudo_element(div_id, &doc, "before"));
}

#[test]
fn test_type_and_functional_pseudo_classes() {
    let html = r#"
        <div>
            <h1>Title</h1>
            <p class="first">First para</p>
            <p class="second">Second para</p>
            <p class="third">Third para</p>
            <span>Footer</span>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_tag(root, "div").unwrap();

    let mut p_ids = Vec::new();
    let mut span_id = None;
    for child in doc.children(div_id) {
        if let NodeData::Element(el) = &child.data {
            if el.tag_name == "p" {
                p_ids.push(child.id);
            } else if el.tag_name == "span" {
                span_id = Some(child.id);
            }
        }
    }

    // first-of-type
    let first_of_type = SimpleSelector::PseudoClass("first-of-type".to_string());
    assert!(first_of_type.matches(p_ids[0], &doc));
    assert!(!first_of_type.matches(p_ids[1], &doc));

    // last-of-type
    let last_of_type = SimpleSelector::PseudoClass("last-of-type".to_string());
    assert!(last_of_type.matches(p_ids[2], &doc));
    assert!(!last_of_type.matches(p_ids[1], &doc));

    // only-of-type
    let only_of_type = SimpleSelector::PseudoClass("only-of-type".to_string());
    assert!(only_of_type.matches(span_id.unwrap(), &doc));
    assert!(!only_of_type.matches(p_ids[0], &doc));

    // nth-of-type(2)
    let nth_of_type_2 = SimpleSelector::PseudoClass("nth-of-type(2)".to_string());
    assert!(nth_of_type_2.matches(p_ids[1], &doc));
    assert!(!nth_of_type_2.matches(p_ids[0], &doc));

    // is(span, h2)
    let is_sel = SimpleSelector::PseudoClass("is(span, h2)".to_string());
    assert!(is_sel.matches(span_id.unwrap(), &doc));
    assert!(!is_sel.matches(p_ids[0], &doc));

    // where(.second, span)
    let where_sel = SimpleSelector::PseudoClass("where(.second, span)".to_string());
    assert!(where_sel.matches(p_ids[1], &doc));
    assert!(where_sel.matches(span_id.unwrap(), &doc));
    assert!(!where_sel.matches(p_ids[0], &doc));

    // not(.first)
    let not_sel = SimpleSelector::PseudoClass("not(.first)".to_string());
    assert!(!not_sel.matches(p_ids[0], &doc));
    assert!(not_sel.matches(p_ids[1], &doc));
}

#[test]
fn test_has_subsequent_sibling() {
    let html = r#"
        <div>
            <h1 id="title">Title</h1>
            <p id="p1">Intro</p>
            <span id="target">Span</span>
        </div>
    "#;
    let doc = mango_html::parse_html(html);
    let root = doc.root();
    let h1_id = doc.find_element_by_tag(root, "h1").unwrap();
    let p1_id = doc.find_element_by_tag(root, "p").unwrap();

    // h1 has subsequent sibling span (not immediate!)
    let sel_sub = crate::parser::parse_selectors("h1:has(~ span)").unwrap();
    assert!(sel_sub.matches(h1_id, &doc), "h1 must match :has(~ span)");

    // h1 does NOT have immediate sibling span (+ span is false because p is immediate)
    let sel_next = crate::parser::parse_selectors("h1:has(+ span)").unwrap();
    assert!(
        !sel_next.matches(h1_id, &doc),
        "h1 must not match :has(+ span)"
    );

    // p1 has immediate sibling span (+ span is true)
    let p1_next = crate::parser::parse_selectors("p:has(+ span)").unwrap();
    assert!(p1_next.matches(p1_id, &doc), "p must match :has(+ span)");
}
