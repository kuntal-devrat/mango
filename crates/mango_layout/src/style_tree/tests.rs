use super::*;
use counters::format_counter_value;
use mango_core::Color;
use mango_css::values::Display;
use mango_html::parse_html;

#[test]
fn test_build_style_tree_filters_head_and_style() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <title>Test</title>
                <style>h1 { color: #ff0000; }</style>
            </head>
            <body>
                <h1>Hello Mango</h1>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");

    // The root should be <html>, child should be <body>, child of body is <h1>
    assert_eq!(style_tree.style.display, Display::Block);

    // Verify embedded <style> applied to <h1>
    let body = &style_tree.children[0];
    let h1 = &body.children[0];
    assert_eq!(h1.style.color, Color::RED);

    // Verify <h1> has text child "Hello Mango"
    assert_eq!(h1.children.len(), 1);
    assert_eq!(h1.children[0].text.as_deref(), Some("Hello Mango"));
}

#[test]
fn test_collapse_whitespace() {
    assert_eq!(collapse_whitespace("hello   world\n\t!"), "hello world !");
    assert_eq!(collapse_whitespace("   "), " ");
}

#[test]
fn test_pseudo_element_synthesis() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    .quote::before { content: "“"; color: #0000ff; }
                    .quote::after { content: "”"; color: #ff0000; }
                </style>
            </head>
            <body>
                <p class="quote">Hello World</p>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
    let body = &style_tree.children[0];
    let p = &body.children[0];

    assert_eq!(p.children.len(), 3);
    assert_eq!(p.children[0].text.as_deref(), Some("“"));
    assert_eq!(p.children[0].style.color, Color::BLUE);
    assert_eq!(p.children[1].text.as_deref(), Some("Hello World"));
    assert_eq!(p.children[2].text.as_deref(), Some("”"));
    assert_eq!(p.children[2].style.color, Color::RED);
}

#[test]
fn test_placeholder_and_marker_styling() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    input::placeholder { color: #ff0000; }
                    li::marker { color: #00ff00; content: "» "; }
                </style>
            </head>
            <body>
                <input type="text" placeholder="Search..." />
                <ul>
                    <li>Item 1</li>
                </ul>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
    let body = &style_tree.children[0];
    let input = &body.children[0];
    let ul = &body.children[1];
    let li = &ul.children[0];

    let ph_color = input
        .attributes
        .iter()
        .find(|(k, _)| k == "_mango_placeholder_color");
    assert!(ph_color.is_some(), "Expected placeholder color attribute");
    assert!(ph_color.unwrap().1.contains("255,0,0"));

    let marker_color = li
        .attributes
        .iter()
        .find(|(k, _)| k == "_mango_marker_color");
    assert!(marker_color.is_some(), "Expected marker color attribute");
    assert!(marker_color.unwrap().1.contains("0,255,0"));

    let marker_content = li
        .attributes
        .iter()
        .find(|(k, _)| k == "_mango_marker_content");
    assert_eq!(marker_content.map(|(_, v)| v.as_str()), Some("» "));
}

#[test]
fn test_template_suppression_and_shadow_dom_slot_projection() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <div id="host"><span>Light text</span></div>
                <template id="tmpl"><span>Hidden template text</span></template>
            </body>
        </html>
    "#;
    let mut doc = parse_html(html);

    // 1. Verify template element is suppressed from the style tree
    let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
    let body = &style_tree.children[0];
    assert_eq!(
        body.children.len(),
        1,
        "Template element should not produce a styled node"
    );
    assert_eq!(
        body.children[0]
            .attributes
            .iter()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| v.as_str()),
        Some("host")
    );

    // 2. Attach a shadow root containing a slot to the host element
    let host_id = doc
        .find_element_by_id(doc.root(), "host")
        .expect("host element exists");
    let shadow_root = doc.attach_shadow(host_id).expect("shadow root attached");

    // Add a slot inside the shadow root
    let slot = doc.create_element("slot", vec![]);
    doc.append_child(shadow_root, slot);

    // Build the style tree again — light DOM children should now be projected through the slot in shadow DOM!
    let tree_with_shadow = build_style_tree(&doc, &[]).expect("style tree exists");
    let body_shadow = &tree_with_shadow.children[0];
    let host_node = &body_shadow.children[0];
    assert_eq!(host_node.children.len(), 1);
    assert_eq!(
        host_node.children[0].children[0].text.as_deref(),
        Some("Light text")
    );
}

#[test]
fn test_shadow_dom_v1_scoped_styles_and_slot_projection() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    #host { color: #000000; }
                    .outside { color: #0000ff; }
                </style>
            </head>
            <body>
                <div id="host" class="active">
                    <span class="slotted-item">Slotted Content</span>
                </div>
                <div class="outside">Outside text</div>
            </body>
        </html>
    "#;
    let mut doc = parse_html(html);
    let host_id = doc
        .find_element_by_id(doc.root(), "host")
        .expect("host exists");
    let shadow_root = doc.attach_shadow(host_id).expect("shadow root attached");

    // Style element inside shadow root
    let style_elem = doc.create_element("style", vec![]);
    let css_text = doc.create_text(
        r#"
        :host { display: block; border-top: 2px solid black; }
        :host(.active) { background-color: rgb(255, 0, 0); }
        .inside { color: rgb(0, 255, 0); }
        ::slotted(.slotted-item) { font-weight: 700; color: rgb(0, 128, 0); }
        "#,
    );
    doc.append_child(style_elem, css_text);
    doc.append_child(shadow_root, style_elem);

    // Div inside shadow root
    let inside_div = doc.create_element("div", vec![("class".to_string(), "inside".to_string())]);
    let inside_text = doc.create_text("Inside Shadow");
    doc.append_child(inside_div, inside_text);
    doc.append_child(shadow_root, inside_div);

    // Slot inside shadow root
    let slot = doc.create_element("slot", vec![]);
    doc.append_child(shadow_root, slot);

    let tree = build_style_tree(&doc, &[]).expect("styled tree");
    let body = &tree.children[0];
    let host = &body.children[0];
    let outside = &body.children[1];

    // 1. Verify host styles from :host and :host(.active)
    assert_eq!(host.style.display, Display::Block);
    assert_eq!(host.style.border_top_width, 2.0);
    assert_eq!(host.style.background_color, Color::rgba(255, 0, 0, 255));

    // 2. Verify scoped styles do not leak to outside elements
    assert_eq!(outside.style.color, Color::rgba(0, 0, 255, 255));

    // 3. Verify flat tree projection: host has inside_div and slotted span
    assert_eq!(host.children.len(), 2);
    let first_child = &host.children[0];
    assert_eq!(first_child.tag_name.as_deref(), Some("div"));
    assert_eq!(first_child.style.color, Color::rgba(0, 255, 0, 255));

    let second_child = &host.children[1];
    assert_eq!(second_child.tag_name.as_deref(), Some("span"));
    assert_eq!(
        second_child.style.font_weight,
        mango_css::FontWeight::Numeric(700)
    );
    assert_eq!(second_child.style.color, Color::rgba(0, 128, 0, 255));
}

#[test]
fn test_incremental_style_invalidation() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    .active { color: #ff0000; }
                    .hidden { display: none; }
                    .muted { opacity: 0.5; }
                </style>
            </head>
            <body>
                <div id="c1" class="muted">Card 1</div>
                <div id="c2">Card 2</div>
                <div id="c3">Card 3</div>
            </body>
        </html>
    "#;
    let mut doc = parse_html(html);
    let mut style_tree = build_style_tree(&doc, &[]).expect("style tree exists");

    let c2_id = doc
        .find_element_by_id(doc.root(), "c2")
        .expect("c2 element exists");

    // Mutate only c2 by adding the .active class
    doc.add_class(c2_id, "active");
    assert!(doc.has_dirty_nodes());
    assert_eq!(doc.dirty_nodes().len(), 1);

    // Run incremental invalidation
    let mut invalidator = StyleInvalidator::new();
    invalidator.mark_all_dirty(doc.take_dirty_nodes());

    let recalc_count = invalidator.invalidate_and_update(&mut style_tree, &doc, &[]);
    assert_eq!(recalc_count, 2);

    let body = &style_tree.children[0];
    let c2_styled = &body.children[1];
    assert_eq!(c2_styled.style.color, Color::rgb(255, 0, 0));
}

#[test]
fn test_generated_content_and_counters() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <head>
                <style>
                    body {
                        counter-reset: chapter 0;
                    }
                    .chap {
                        counter-increment: chapter 1;
                    }
                    .chap::before {
                        content: "Chapter " counter(chapter, upper-roman) ": " attr(data-title) " ";
                    }
                    .quote {
                        quotes: "«" "»" "“" "”";
                    }
                    .quote::before {
                        content: open-quote;
                    }
                    .quote::after {
                        content: close-quote;
                    }
                    .icon::after {
                        content: url("icon.png");
                    }
                </style>
            </head>
            <body>
                <div id="c1" class="chap" data-title="Introduction">Content 1</div>
                <div id="c2" class="chap" data-title="Deep Dive">
                    <span id="q1" class="quote">Quote 1</span>
                </div>
                <div id="btn" class="icon">Button</div>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree root exists");
    let body = &style_tree.children[0];

    let c1 = &body.children[0];
    assert_eq!(c1.children.len(), 2);
    assert_eq!(
        c1.children[0].text.as_deref(),
        Some("Chapter I: Introduction ")
    );
    assert_eq!(c1.children[1].text.as_deref(), Some("Content 1"));

    let c2 = &body.children[1];
    assert_eq!(
        c2.children[0].text.as_deref(),
        Some("Chapter II: Deep Dive ")
    );

    let q1 = &c2.children[1];
    assert_eq!(q1.children.len(), 3);
    assert_eq!(q1.children[0].text.as_deref(), Some("«"));
    assert_eq!(q1.children[1].text.as_deref(), Some("Quote 1"));
    assert_eq!(q1.children[2].text.as_deref(), Some("»"));

    let btn = &body.children[2];
    assert_eq!(btn.children.len(), 2);
    assert_eq!(btn.children[0].text.as_deref(), Some("Button"));
    let img_node = &btn.children[1];
    assert_eq!(img_node.tag_name.as_deref(), Some("img"));
    assert!(
        img_node
            .attributes
            .iter()
            .any(|(k, v)| k == "src" && v == "icon.png")
    );
}

#[test]
fn test_whitespace_preserved_between_inline_elements() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <p><span>Hello</span> <span>World</span></p>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree exists");
    let body = &style_tree.children[0];
    let p = &body.children[0];

    assert_eq!(
        p.children.len(),
        3,
        "Inter-inline whitespace must be preserved"
    );
    assert_eq!(p.children[0].children[0].text.as_deref(), Some("Hello"));
    assert_eq!(p.children[1].text.as_deref(), Some(" "));
    assert_eq!(p.children[2].children[0].text.as_deref(), Some("World"));
}

#[test]
fn test_details_multiple_summaries_closed() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <details>
                    <summary>Summary 1</summary>
                    <summary>Summary 2</summary>
                    <p>Hidden body</p>
                </details>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let style_tree = build_style_tree(&doc, &[]).expect("style tree exists");
    let body = &style_tree.children[0];
    let details = &body.children[0];

    assert_eq!(details.children.len(), 1);
    assert_eq!(
        details.children[0].children[0].text.as_deref(),
        Some("Summary 1")
    );
}

#[test]
fn test_format_counter_leading_zero_and_none() {
    assert_eq!(format_counter_value(5, "decimal-leading-zero"), "05");
    assert_eq!(format_counter_value(12, "decimal-leading-zero"), "12");
    assert_eq!(format_counter_value(5, "none"), "");
}

#[test]
fn test_inline_element_with_block_children_promoted_to_block() {
    let html = r#"
        <main id="container">
            <custom-tag>
                <div class="inner-block">Inside custom tag</div>
            </custom-tag>
        </main>
    "#;
    let doc = parse_html(html);
    let styled = build_style_tree(&doc, &[]).expect("styled root");
    let mut box_root = crate::box_tree::build_box_tree(&styled);

    fn find_by_tag<'a>(
        b: &'a crate::box_tree::LayoutBox,
        tag: &str,
    ) -> Option<&'a crate::box_tree::LayoutBox> {
        if b.tag_name.as_deref() == Some(tag) {
            return Some(b);
        }
        for child in &b.children {
            if let Some(found) = find_by_tag(child, tag) {
                return Some(found);
            }
        }
        None
    }

    let custom_box = find_by_tag(&box_root, "custom-tag").expect("custom-tag box exists");
    assert_eq!(custom_box.box_type, crate::box_model::BoxType::BlockNode);
    assert_eq!(custom_box.children.len(), 1);

    let cb = crate::dimensions::Dimensions::new(mango_core::Rect::new(0.0, 0.0, 800.0, 600.0));
    let mut float_ctx = crate::float::FloatContext::new();
    crate::layout_block(&mut box_root, &cb, &mut float_ctx);

    let custom_after = find_by_tag(&box_root, "custom-tag").expect("custom-tag box after layout");
    assert_eq!(custom_after.children.len(), 1);
    assert!(custom_after.dimensions.content.size.height > 0.0);
}
