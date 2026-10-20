use mango_css::values::Display;
use mango_engine::Engine;
use mango_html::dom::{NodeData, NodeId};
use mango_layout::StyledNode;

fn find_styled_node(root: &StyledNode, target_id: NodeId) -> Option<&StyledNode> {
    if root.node_id == Some(target_id) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(found) = find_styled_node(child, target_id) {
            return Some(found);
        }
    }
    None
}

// ── 3.1: HTML5 Popover & Top-Layer Architecture ──────────────────────────────

#[test]
fn test_popover_ua_styles_and_top_layer_stacking() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    #normal-high-z {
                        position: absolute;
                        z-index: 999999;
                    }
                </style>
            </head>
            <body>
                <div id="normal-high-z">High Z Box</div>
                <div id="popover-closed" popover="auto">Popover Closed</div>
                <div id="popover-open" popover="auto" data-mango-popover-open="true">Popover Open</div>
            </body>
        </html>
    "#;

    let _dl = engine.load(html, "http://localhost/");
    let doc = engine.document();
    let style_tree = engine.style_tree().expect("style tree exists");

    // Closed popover should be excluded from layout style_tree because UA stylesheet gives it display: none
    let closed_node = doc
        .find_element_by_id(doc.root(), "popover-closed")
        .unwrap();
    assert!(
        find_styled_node(style_tree, closed_node).is_none(),
        "closed popover with display: none must generate no styled node"
    );

    // Open popover matches :popover-open and becomes display: block, thus present in style_tree
    let open_node = doc.find_element_by_id(doc.root(), "popover-open").unwrap();
    let open_style =
        find_styled_node(style_tree, open_node).expect("open popover must generate styled node");
    assert_eq!(open_style.style.display, Display::Block);
}

// ── 3.2: Declarative Shadow DOM (DSD) Parsing ─────────────────────────────────

#[test]
fn test_declarative_shadow_dom_tree_building() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <my-element id="open-host">
                    <template shadowrootmode="open" shadowrootclonable shadowrootdelegatesfocus>
                        <h1 id="shadow-title">Shadow Header</h1>
                        <p>Shadow Paragraph</p>
                    </template>
                    <div id="light-child">Light Child</div>
                </my-element>

                <my-element id="closed-host">
                    <template shadowrootmode="closed">
                        <span id="closed-text">Closed Shadow Text</span>
                    </template>
                </my-element>
            </body>
        </html>
    "#;

    engine.load(html, "http://localhost/");
    let doc = engine.document();

    // 1. Verify open host
    let open_host_id = doc.find_element_by_id(doc.root(), "open-host").unwrap();
    assert_eq!(doc.get_shadow_root_mode(open_host_id), Some("open"));

    // Template element MUST be detached from host light DOM children
    let mut light_tags = Vec::new();
    for kid in doc.children(open_host_id) {
        if let NodeData::Element(el) = &kid.data {
            light_tags.push(el.tag_name.clone());
        }
    }
    // Only "div" (light-child) should remain in light children; "template" is detached
    assert_eq!(light_tags, vec!["div"]);

    // Verify shadow root contains the template contents
    let sr_id = doc
        .get_shadow_root(open_host_id)
        .expect("open shadow root should exist");
    let mut sr_tags = Vec::new();
    for kid in doc.children(sr_id) {
        if let NodeData::Element(el) = &kid.data {
            sr_tags.push(el.tag_name.clone());
        }
    }
    assert_eq!(sr_tags, vec!["h1", "p"]);

    // 2. Verify closed host
    let closed_host_id = doc.find_element_by_id(doc.root(), "closed-host").unwrap();
    assert_eq!(doc.get_shadow_root_mode(closed_host_id), Some("closed"));
    let closed_sr_id = doc
        .get_shadow_root(closed_host_id)
        .expect("closed shadow root should exist");
    let mut closed_sr_tags = Vec::new();
    for kid in doc.children(closed_sr_id) {
        if let NodeData::Element(el) = &kid.data {
            closed_sr_tags.push(el.tag_name.clone());
        }
    }
    assert_eq!(closed_sr_tags, vec!["span"]);
}
