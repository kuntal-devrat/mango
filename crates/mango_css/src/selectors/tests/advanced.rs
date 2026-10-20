use mango_html::dom::{Document, NodeData, NodeId};
use mango_html::parse_html;

use crate::selectors::types::SimpleSelector;
use crate::selectors::validation::element_is_valid;

#[test]
fn test_form_validity_pseudo_classes() {
    let html = r#"
        <form>
            <input id="req-empty" type="text" required />
            <input id="req-filled" type="text" required value="hello" />
            <input id="opt" type="text" />
            <input id="bad-email" type="email" value="not-an-email" />
            <input id="good-email" type="email" value="user@example.com" />
            <input id="num" type="number" value="12" />
            <input id="bad-num" type="number" value="abc" />
            <input id="ro" type="text" readonly />
            <input id="ph" type="text" placeholder="Type here" />
            <input id="hidden-field" type="hidden" required />
        </form>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let by_id = |id: &str| doc.find_element_by_id(root, id).unwrap();

    let required = SimpleSelector::PseudoClass("required".to_string());
    let optional = SimpleSelector::PseudoClass("optional".to_string());
    let valid = SimpleSelector::PseudoClass("valid".to_string());
    let invalid = SimpleSelector::PseudoClass("invalid".to_string());
    let read_only = SimpleSelector::PseudoClass("read-only".to_string());
    let ph_shown = SimpleSelector::PseudoClass("placeholder-shown".to_string());

    assert!(required.matches(by_id("req-empty"), &doc));
    assert!(optional.matches(by_id("opt"), &doc));
    assert!(!required.matches(by_id("opt"), &doc));

    assert!(
        !valid.matches(by_id("req-empty"), &doc),
        "empty required field is invalid"
    );
    assert!(invalid.matches(by_id("req-empty"), &doc));
    assert!(valid.matches(by_id("req-filled"), &doc));
    assert!(!invalid.matches(by_id("req-filled"), &doc));

    assert!(!valid.matches(by_id("bad-email"), &doc));
    assert!(valid.matches(by_id("good-email"), &doc));
    assert!(valid.matches(by_id("num"), &doc));
    assert!(!valid.matches(by_id("bad-num"), &doc));

    // Hidden inputs never participate in constraint validation
    assert!(valid.matches(by_id("hidden-field"), &doc));

    assert!(read_only.matches(by_id("ro"), &doc));
    assert!(!read_only.matches(by_id("req-filled"), &doc));

    assert!(
        ph_shown.matches(by_id("ph"), &doc),
        "placeholder with no value"
    );
    assert!(
        !ph_shown.matches(by_id("req-filled"), &doc),
        "placeholder is hidden once a value is set"
    );
}

#[test]
fn test_focus_within_ancestor_matching() {
    let html = r#"<div id="wrap"><p id="mid"><input id="inner"/></p></div>"#;
    let mut doc = parse_html(html);
    let root = doc.root();
    let wrap = doc.find_element_by_id(root, "wrap").unwrap();
    let inner = doc.find_element_by_id(root, "inner").unwrap();

    let focus_within = SimpleSelector::PseudoClass("focus-within".to_string());
    assert!(!focus_within.matches(wrap, &doc));

    if let Some(node) = doc.get_mut(inner)
        && let NodeData::Element(el) = &mut node.data
    {
        el.attributes
            .push(("data-mango-focused".to_string(), "true".to_string()));
    }
    assert!(
        focus_within.matches(wrap, &doc),
        "ancestors match :focus-within"
    );
}

#[test]
fn test_has_pseudo_class_matching() {
    fn find_by_class(node_id: NodeId, class_name: &str, doc: &Document) -> Option<NodeId> {
        for child in doc.children(node_id) {
            if let NodeData::Element(el) = &child.data
                && el.has_class(class_name)
            {
                return Some(child.id);
            }
            if let Some(found) = find_by_class(child.id, class_name, doc) {
                return Some(found);
            }
        }
        None
    }

    let html = r#"
        <div id="container">
            <article class="with-img">
                <h2>Card 1</h2>
                <img src="photo.jpg" alt="test" />
            </article>
            <article class="no-img">
                <h2>Card 2</h2>
                <p>Just text</p>
            </article>
            <section class="parent">
                <div class="direct-child">
                    <span class="nested">Deep</span>
                </div>
            </section>
        </div>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let with_img_id = find_by_class(root, "with-img", &doc).unwrap();
    let no_img_id = find_by_class(root, "no-img", &doc).unwrap();
    let section_id = find_by_class(root, "parent", &doc).unwrap();

    // article:has(img)
    let has_img = SimpleSelector::PseudoClass("has(img)".to_string());
    assert!(has_img.matches(with_img_id, &doc));
    assert!(!has_img.matches(no_img_id, &doc));

    // section:has(> .direct-child) - direct child combinator
    let has_direct = SimpleSelector::PseudoClass("has(> .direct-child)".to_string());
    assert!(has_direct.matches(section_id, &doc));

    // section:has(> .nested) - should be false because .nested is not a direct child
    let has_not_direct = SimpleSelector::PseudoClass("has(> .nested)".to_string());
    assert!(!has_not_direct.matches(section_id, &doc));

    // section:has(.nested) - descendant combinator matches
    let has_descendant = SimpleSelector::PseudoClass("has(.nested)".to_string());
    assert!(has_descendant.matches(section_id, &doc));
}

#[test]
fn test_form_element_validation() {
    let html = r#"
        <form id="f">
            <input id="num-ok" type="number" min="10" max="50" step="5" value="25" />
            <input id="num-bad-min" type="number" min="10" max="50" value="5" />
            <input id="num-bad-step" type="number" min="10" max="50" step="5" value="23" />
            <input id="pat-ok" type="text" pattern="[A-Z]{3}-[0-9]{3}" value="ABC-123" />
            <input id="pat-bad" type="text" pattern="[A-Z]{3}-[0-9]{3}" value="abc-123" />
            <input id="date-ok" type="date" min="2026-01-01" max="2026-12-31" value="2026-06-15" />
            <input id="date-bad" type="date" min="2026-01-01" max="2026-12-31" value="2027-01-01" />
        </form>
    "#;
    let doc = parse_html(html);

    fn is_valid_by_id(id: &str, doc: &Document) -> bool {
        let root = doc.root();
        let mut stack = vec![root];
        while let Some(nid) = stack.pop() {
            if let Some(node) = doc.get(nid) {
                if let NodeData::Element(el) = &node.data
                    && el.get_attribute("id") == Some(id)
                {
                    return element_is_valid(nid, el);
                }
                for c in doc.children(nid) {
                    stack.push(c.id);
                }
            }
        }
        panic!("Element #{} not found", id);
    }

    assert!(is_valid_by_id("num-ok", &doc));
    assert!(!is_valid_by_id("num-bad-min", &doc));
    assert!(!is_valid_by_id("num-bad-step", &doc));
    assert!(is_valid_by_id("pat-ok", &doc));
    assert!(!is_valid_by_id("pat-bad", &doc));
    assert!(is_valid_by_id("date-ok", &doc));
    assert!(!is_valid_by_id("date-bad", &doc));
}

#[test]
fn test_selectors_level_4() {
    let html = r#"
        <div id="wrapper">
            <!-- Target test element -->
            <section id="section-target" data-mango-target="true">
                <h1 id="heading">Title</h1>
                <p id="first-p" class="intro">Intro paragraph</p>
                <span id="sibling-span">Span text</span>
            </section>

            <!-- List for nth-child(An+B of S), nth-of-type, etc. -->
            <ul id="list">
                <li id="li-1" class="item active">Item 1</li>
                <li id="li-2" class="item">Item 2</li>
                <li id="li-3" class="separator">Separator</li>
                <li id="li-4" class="item active">Item 4</li>
                <li id="li-5" class="item">Item 5</li>
            </ul>

            <!-- Only child and type tests -->
            <div id="single-parent">
                <p id="only-p">Only child and only of type</p>
            </div>
            <div id="multi-type-parent">
                <span id="span-type-1">Span 1</span>
                <span id="span-type-2">Span 2</span>
                <h2 id="only-h2">Only H2</h2>
            </div>

            <!-- Empty test elements -->
            <div id="truly-empty"></div>
            <div id="comment-empty"><!-- a comment should be ignored --></div>
            <div id="whitespace-empty">
                
            </div>
            <div id="not-empty">Text inside</div>

            <!-- Form controls for validity, enabled/disabled, checked, indeterminate, placeholder-shown, read-only/write -->
            <form id="test-form">
                <input id="inp-enabled" type="text" placeholder="Enter name" value="" />
                <input id="inp-disabled" type="text" disabled value="disabled" />
                <input id="inp-required" type="text" required value="" />
                <input id="inp-optional" type="text" value="optional text" />
                <input id="chk-checked" type="checkbox" checked />
                <input id="chk-unchecked" type="checkbox" />
                <input id="chk-indet" type="checkbox" data-mango-indeterminate="true" />
                <progress id="prog-indet"></progress>
                <progress id="prog-determinate" value="50" max="100"></progress>
                <textarea id="ta-editable" placeholder="Type here"></textarea>
                <textarea id="ta-readonly" readonly>Read-only text</textarea>
            </form>

            <div id="editable-div" contenteditable="true">Editable content</div>
            <p id="readonly-p">Plain paragraph</p>

            <!-- Focus elements -->
            <div id="focus-container">
                <input id="inp-focus-visible" type="text" data-mango-focus-visible="true" />
            </div>
        </div>
    "#;

    let mut doc = parse_html(html);
    let root = doc.root();
    fn collect_ids(
        doc: &Document,
        node_id: NodeId,
        map: &mut std::collections::HashMap<String, NodeId>,
    ) {
        for child in doc.children(node_id) {
            if let mango_html::NodeData::Element(elem) = &child.data
                && let Some(id) = elem.id()
            {
                map.insert(id.to_string(), child.id);
            }
            collect_ids(doc, child.id, map);
        }
    }
    let mut el_map = std::collections::HashMap::new();
    collect_ids(&doc, root, &mut el_map);
    let el = |id: &str| *el_map.get(id).unwrap();

    // 1. :is() and :where()
    let is_sel = crate::parser::parse_selectors(":is(h1, h2, #nonexistent)").unwrap();
    assert!(is_sel.matches(el("heading"), &doc));
    assert!(is_sel.matches(el("only-h2"), &doc));
    assert!(!is_sel.matches(el("first-p"), &doc));

    let where_sel = crate::parser::parse_selectors(":where(p.intro, span#sibling-span)").unwrap();
    assert!(where_sel.matches(el("first-p"), &doc));
    assert!(where_sel.matches(el("sibling-span"), &doc));
    assert!(!where_sel.matches(el("heading"), &doc));

    // 2. :not() with complex selectors
    let not_complex = crate::parser::parse_selectors(":not(#section-target > p.intro)").unwrap();
    assert!(!not_complex.matches(el("first-p"), &doc));
    assert!(not_complex.matches(el("heading"), &doc));

    let not_multi = crate::parser::parse_selectors(":not(.active, .separator)").unwrap();
    assert!(!not_multi.matches(el("li-1"), &doc));
    assert!(!not_multi.matches(el("li-3"), &doc));
    assert!(not_multi.matches(el("li-2"), &doc));

    // 3. :has() parent and sibling selector
    let has_next_sib = crate::parser::parse_selectors("h1:has(+ p)").unwrap();
    assert!(has_next_sib.matches(el("heading"), &doc));

    let has_sub_sib = crate::parser::parse_selectors("h1:has(~ span)").unwrap();
    assert!(has_sub_sib.matches(el("heading"), &doc));

    let has_no_match = crate::parser::parse_selectors("p:has(+ h1)").unwrap();
    assert!(!has_no_match.matches(el("first-p"), &doc));

    let has_child = crate::parser::parse_selectors("section:has(> h1#heading)").unwrap();
    assert!(has_child.matches(el("section-target"), &doc));

    let has_descendant = crate::parser::parse_selectors("section:has(span#sibling-span)").unwrap();
    assert!(has_descendant.matches(el("section-target"), &doc));

    // 4. :nth-child(An+B [of S]) and :nth-last-child(An+B [of S])
    let nth_of_active = crate::parser::parse_selectors("li:nth-child(2 of .active)").unwrap();
    assert!(!nth_of_active.matches(el("li-1"), &doc));
    assert!(!nth_of_active.matches(el("li-2"), &doc));
    assert!(nth_of_active.matches(el("li-4"), &doc));

    let nth_last_of = crate::parser::parse_selectors("li:nth-last-child(1 of .active)").unwrap();
    assert!(nth_last_of.matches(el("li-4"), &doc));
    assert!(!nth_last_of.matches(el("li-1"), &doc));

    let nth_of_type = crate::parser::parse_selectors("span:nth-of-type(2)").unwrap();
    assert!(nth_of_type.matches(el("span-type-2"), &doc));
    assert!(!nth_of_type.matches(el("span-type-1"), &doc));

    let nth_last_of_type = crate::parser::parse_selectors("span:nth-last-of-type(1)").unwrap();
    assert!(nth_last_of_type.matches(el("span-type-2"), &doc));

    // 5. :first-of-type, :last-of-type, :only-of-type, :only-child
    let first_of_type = SimpleSelector::PseudoClass("first-of-type".to_string());
    assert!(first_of_type.matches(el("span-type-1"), &doc));
    assert!(!first_of_type.matches(el("span-type-2"), &doc));

    let last_of_type = SimpleSelector::PseudoClass("last-of-type".to_string());
    assert!(last_of_type.matches(el("span-type-2"), &doc));

    let only_of_type = SimpleSelector::PseudoClass("only-of-type".to_string());
    assert!(only_of_type.matches(el("only-h2"), &doc));
    assert!(!only_of_type.matches(el("span-type-1"), &doc));

    let only_child = SimpleSelector::PseudoClass("only-child".to_string());
    assert!(only_child.matches(el("only-p"), &doc));
    assert!(!only_child.matches(el("only-h2"), &doc));

    // 6. :empty pseudo-class
    let empty_sel = SimpleSelector::PseudoClass("empty".to_string());
    assert!(empty_sel.matches(el("truly-empty"), &doc));
    assert!(
        empty_sel.matches(el("comment-empty"), &doc),
        "comments must be ignored by :empty"
    );
    assert!(
        empty_sel.matches(el("whitespace-empty"), &doc),
        "whitespace must be ignored by :empty in Selectors 4"
    );
    assert!(!empty_sel.matches(el("not-empty"), &doc));

    // 7. :target pseudo-class
    let target_sel = SimpleSelector::PseudoClass("target".to_string());
    assert!(target_sel.matches(el("section-target"), &doc));
    doc.set_target_id(Some("heading".to_string()));
    assert!(
        target_sel.matches(el("heading"), &doc),
        ":target matches document.target_id"
    );
    assert!(!target_sel.matches(el("first-p"), &doc));

    // 8. :focus-visible and :focus-within
    let focus_vis = SimpleSelector::PseudoClass("focus-visible".to_string());
    let focus_within = SimpleSelector::PseudoClass("focus-within".to_string());
    assert!(focus_vis.matches(el("inp-focus-visible"), &doc));
    assert!(
        focus_within.matches(el("focus-container"), &doc),
        "ancestor matches :focus-within"
    );
    assert!(!focus_within.matches(el("section-target"), &doc));

    // 9. :placeholder-shown
    let ph_shown = SimpleSelector::PseudoClass("placeholder-shown".to_string());
    assert!(ph_shown.matches(el("inp-enabled"), &doc));
    assert!(ph_shown.matches(el("ta-editable"), &doc));
    assert!(!ph_shown.matches(el("inp-optional"), &doc));

    // 10. :enabled, :disabled, :checked, :indeterminate
    let enabled_sel = SimpleSelector::PseudoClass("enabled".to_string());
    let disabled_sel = SimpleSelector::PseudoClass("disabled".to_string());
    let checked_sel = SimpleSelector::PseudoClass("checked".to_string());
    let indet_sel = SimpleSelector::PseudoClass("indeterminate".to_string());

    assert!(enabled_sel.matches(el("inp-enabled"), &doc));
    assert!(!enabled_sel.matches(el("inp-disabled"), &doc));
    assert!(
        !enabled_sel.matches(el("wrapper"), &doc),
        "div cannot match :enabled"
    );

    assert!(disabled_sel.matches(el("inp-disabled"), &doc));
    assert!(!disabled_sel.matches(el("inp-enabled"), &doc));

    assert!(checked_sel.matches(el("chk-checked"), &doc));
    assert!(!checked_sel.matches(el("chk-unchecked"), &doc));

    assert!(indet_sel.matches(el("chk-indet"), &doc));
    assert!(
        indet_sel.matches(el("prog-indet"), &doc),
        "<progress> with no value matches :indeterminate"
    );
    assert!(!indet_sel.matches(el("prog-determinate"), &doc));

    // 11. :required, :optional, :valid, :invalid
    let req_sel = SimpleSelector::PseudoClass("required".to_string());
    let opt_sel = SimpleSelector::PseudoClass("optional".to_string());
    let valid_sel = SimpleSelector::PseudoClass("valid".to_string());
    let invalid_sel = SimpleSelector::PseudoClass("invalid".to_string());

    assert!(req_sel.matches(el("inp-required"), &doc));
    assert!(!req_sel.matches(el("inp-optional"), &doc));
    assert!(
        !req_sel.matches(el("wrapper"), &doc),
        "div cannot match :required"
    );

    assert!(opt_sel.matches(el("inp-optional"), &doc));
    assert!(!opt_sel.matches(el("inp-required"), &doc));
    assert!(
        !opt_sel.matches(el("wrapper"), &doc),
        "div cannot match :optional"
    );

    assert!(
        invalid_sel.matches(el("inp-required"), &doc),
        "empty required field is :invalid"
    );
    assert!(!valid_sel.matches(el("inp-required"), &doc));
    assert!(valid_sel.matches(el("inp-optional"), &doc));

    // 12. :read-only, :read-write
    let rw_sel = SimpleSelector::PseudoClass("read-write".to_string());
    let ro_sel = SimpleSelector::PseudoClass("read-only".to_string());

    assert!(rw_sel.matches(el("inp-enabled"), &doc));
    assert!(rw_sel.matches(el("ta-editable"), &doc));
    assert!(
        rw_sel.matches(el("editable-div"), &doc),
        "contenteditable elements match :read-write"
    );

    assert!(!rw_sel.matches(el("inp-disabled"), &doc));
    assert!(!rw_sel.matches(el("ta-readonly"), &doc));
    assert!(
        !rw_sel.matches(el("readonly-p"), &doc),
        "normal paragraphs match :read-only"
    );

    assert!(ro_sel.matches(el("readonly-p"), &doc));
    assert!(ro_sel.matches(el("inp-disabled"), &doc));
    assert!(ro_sel.matches(el("ta-readonly"), &doc));
    assert!(!ro_sel.matches(el("inp-enabled"), &doc));

    // 13. ::placeholder pseudo-element
    let ph_pseudo = crate::parser::parse_selectors("input::placeholder")
        .unwrap()
        .selectors[0]
        .clone();
    assert!(ph_pseudo.matches_pseudo_element(el("inp-enabled"), &doc, "placeholder"));
    assert!(ph_pseudo.matches_pseudo_element(el("inp-enabled"), &doc, "-webkit-input-placeholder"));

    // 14. ::selection pseudo-element
    let sel_pseudo = crate::parser::parse_selectors("p::selection")
        .unwrap()
        .selectors[0]
        .clone();
    assert!(sel_pseudo.matches_pseudo_element(el("first-p"), &doc, "selection"));
    assert!(sel_pseudo.matches_pseudo_element(el("first-p"), &doc, "-moz-selection"));
    assert!(!sel_pseudo.matches_pseudo_element(el("heading"), &doc, "selection"));

    // 15. ::marker pseudo-element
    let marker_pseudo = crate::parser::parse_selectors("li::marker")
        .unwrap()
        .selectors[0]
        .clone();
    assert!(marker_pseudo.matches_pseudo_element(el("li-1"), &doc, "marker"));
    assert!(
        !marker_pseudo.matches_pseudo_element(el("first-p"), &doc, "marker"),
        "p does not have ::marker"
    );
}

#[test]
fn test_shadow_dom_host_and_slotted_selectors() {
    let html = r#"
        <div id="theme-root" class="theme-dark">
            <custom-card id="host1" class="highlight" active="true">
                <span id="slotted-span" slot="badge">Hello</span>
                <p id="slotted-p">Content</p>
            </custom-card>
        </div>
    "#;
    let mut doc = mango_html::parse_html(html);
    let host_id = doc.find_element_by_id(doc.root(), "host1").unwrap();
    let sr_id = doc.attach_shadow(host_id).unwrap();

    let slot1 = doc.create_element("slot", vec![("name".to_string(), "badge".to_string())]);
    doc.append_child(sr_id, slot1);
    let span_id = doc.find_element_by_id(doc.root(), "slotted-span").unwrap();
    doc.append_child(slot1, span_id);

    crate::selectors::types::set_current_host_element(Some(host_id));

    // 1. :host
    let host_sel = crate::parser::parse_selectors(":host").unwrap();
    assert!(
        host_sel.matches(host_id, &doc),
        ":host must match host element"
    );

    // 2. :host(.highlight)
    let host_class_sel = crate::parser::parse_selectors(":host(.highlight)").unwrap();
    assert!(
        host_class_sel.matches(host_id, &doc),
        ":host(.highlight) must match"
    );

    // 3. :host([active])
    let host_attr_sel = crate::parser::parse_selectors(":host([active])").unwrap();
    assert!(
        host_attr_sel.matches(host_id, &doc),
        ":host([active]) must match"
    );

    // 4. :host(.nonexistent)
    let host_none_sel = crate::parser::parse_selectors(":host(.nonexistent)").unwrap();
    assert!(
        !host_none_sel.matches(host_id, &doc),
        ":host(.nonexistent) must not match"
    );

    // 5. :host-context(.theme-dark)
    let host_ctx_sel = crate::parser::parse_selectors(":host-context(.theme-dark)").unwrap();
    assert!(
        host_ctx_sel.matches(host_id, &doc),
        ":host-context(.theme-dark) must match ancestor"
    );

    // 6. ::slotted(span)
    let slotted_sel = crate::parser::parse_selectors("::slotted(span)").unwrap();
    assert!(
        slotted_sel.matches(span_id, &doc),
        "::slotted(span) must match slotted span"
    );

    crate::selectors::types::set_current_host_element(None);
}
