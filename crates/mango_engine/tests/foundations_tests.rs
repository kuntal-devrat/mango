use mango_core::{Color, Size};
use mango_engine::Engine;
use mango_html::dom::NodeId;
use mango_layout::StyledNode;

fn find_styled_node<'a>(root: &'a StyledNode, target_id: NodeId) -> Option<&'a StyledNode> {
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

#[test]
fn test_engine_css_cascade_layers_ordering() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    /* Statement syntax declares order: base first, then theme */
                    @layer base, theme;

                    /* Even though theme is written first in rules, base is declared first above */
                    @layer theme {
                        #target { color: #ff0000; }
                    }

                    @layer base {
                        #target { color: #0000ff; }
                    }
                </style>
            </head>
            <body>
                <div id="target">Layered element</div>
            </body>
        </html>
    "#;

    let dl = engine.load(html, "");
    assert!(!dl.is_empty());

    let target_node = engine
        .document()
        .find_element_by_id(engine.document().root(), "target")
        .expect("target element must exist");

    let style_tree = engine.style_tree().expect("style tree must exist");
    let target_style = find_styled_node(&style_tree, target_node)
        .expect("target style must exist");

    // Theme layer was declared after base layer, so theme wins (red: #ff0000)
    assert_eq!(target_style.style.color, Color::rgb(255, 0, 0));
}

#[test]
fn test_engine_css_scoping_and_doughnut_hole() {
    let mut engine = Engine::new();
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    @scope (.card) to (.hole) {
                        p { color: #00ff00; }
                    }
                </style>
            </head>
            <body>
                <div class="card">
                    <p id="in-scope">In scope paragraph</p>
                    <div class="hole">
                        <p id="in-hole">In hole paragraph</p>
                    </div>
                </div>
            </body>
        </html>
    "#;

    let dl = engine.load(html, "");
    assert!(!dl.is_empty());

    let root = engine.document().root();
    let in_scope_node = engine
        .document()
        .find_element_by_id(root, "in-scope")
        .expect("in-scope must exist");
    let in_hole_node = engine
        .document()
        .find_element_by_id(root, "in-hole")
        .expect("in-hole must exist");

    let style_tree = engine.style_tree().expect("style tree must exist");
    let in_scope_style = find_styled_node(&style_tree, in_scope_node)
        .expect("in-scope style must exist");
    let in_hole_style = find_styled_node(&style_tree, in_hole_node)
        .expect("in-hole style must exist");

    // #in-scope matches @scope and gets green (#00ff00)
    assert_eq!(in_scope_style.style.color, Color::rgb(0, 255, 0));

    // #in-hole is inside .hole boundary (doughnut hole limit), so does not get green
    assert_ne!(in_hole_style.style.color, Color::rgb(0, 255, 0));
}

#[test]
fn test_engine_subgrid_layout() {
    let mut engine = Engine::with_viewport(Size::new(800.0, 600.0));
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    .parent-grid {
                        display: grid;
                        grid-template-columns: 200px 300px;
                        width: 500px;
                    }
                    .child-subgrid {
                        display: grid;
                        grid-column: 1 / 3;
                        grid-template-columns: subgrid;
                    }
                    .subgrid-item1 {
                        grid-column: 1;
                        height: 50px;
                    }
                    .subgrid-item2 {
                        grid-column: 2;
                        height: 50px;
                    }
                </style>
            </head>
            <body>
                <div class="parent-grid">
                    <div class="child-subgrid">
                        <div id="sub1" class="subgrid-item1">Col1</div>
                        <div id="sub2" class="subgrid-item2">Col2</div>
                    </div>
                </div>
            </body>
        </html>
    "#;

    let dl = engine.load(html, "");
    assert!(!dl.is_empty());

    let snapshot = engine.layout_snapshot().expect("layout snapshot must exist");
    let sub1_node = engine
        .document()
        .find_element_by_id(engine.document().root(), "sub1")
        .expect("sub1 must exist");
    let sub2_node = engine
        .document()
        .find_element_by_id(engine.document().root(), "sub2")
        .expect("sub2 must exist");

    let sub1_rect = snapshot
        .box_rects()
        .get(&sub1_node.raw())
        .expect("sub1 box rect must exist");
    let sub2_rect = snapshot
        .box_rects()
        .get(&sub2_node.raw())
        .expect("sub2 box rect must exist");

    // sub1 rect: [x, y, width, height] -> width is 200px
    assert_eq!(sub1_rect[2], 200.0);
    // sub2 rect: [x, y, width, height] -> width is 300px
    assert_eq!(sub2_rect[2], 300.0);
}
