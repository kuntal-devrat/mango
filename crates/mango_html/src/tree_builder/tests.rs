use super::*;
use crate::dom::NodeData;

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
    let table_id = table_doc
        .find_element_by_tag(t_root, "table")
        .expect("table found");
    let tbody_id = table_doc
        .find_element_by_tag(table_id, "tbody")
        .expect("implicit tbody found");
    let trs: Vec<_> = table_doc
        .children(tbody_id)
        .filter(|node| {
            if let NodeData::Element(el) = &node.data {
                el.tag_name == "tr"
            } else {
                false
            }
        })
        .collect();
    assert_eq!(trs.len(), 2);

    // Foster parenting: "Foster Text" was moved before <table> in body
    let body_id = table_doc
        .find_element_by_tag(t_root, "body")
        .expect("body found");
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
    let doc3 = parse_html(
        "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\" \"http://www.w3.org/TR/html4/loose.dtd\"><html><body></body></html>",
    );
    assert_eq!(doc3.quirks_mode, QuirksMode::LimitedQuirks);

    // Obsolete permitted legacy-compat -> NoQuirks
    let doc4 =
        parse_html("<!DOCTYPE html SYSTEM \"about:legacy-compat\"><html><body></body></html>");
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
    let u_elem = doc
        .find_element_by_tag(i_node.id, "u")
        .expect("u found in second i");
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
    let p1_b = doc
        .find_element_by_tag(p_elements[0].id, "b")
        .expect("b in p1");
    assert_eq!(doc.text_content(p1_b), "hello");

    // Paragraph 2: active formatting reconstructed <b> with "world"
    let p2_b = doc
        .find_element_by_tag(p_elements[1].id, "b")
        .expect("b in p2");
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
    let iframe_id = doc
        .find_element_by_tag(root, "iframe")
        .expect("iframe element should be found");
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
    let table_id = doc
        .find_element_by_tag(root, "table")
        .expect("table exists");
    let tbody_id = doc
        .find_element_by_tag(table_id, "tbody")
        .expect("implicit tbody inserted");
    let tr_id = doc
        .find_element_by_tag(tbody_id, "tr")
        .expect("tr inserted");
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
    let table_id = doc
        .find_element_by_tag(root, "table")
        .expect("table exists");
    let tbody_id = doc
        .find_element_by_tag(table_id, "tbody")
        .expect("tbody exists");
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
    let select_id = doc
        .find_element_by_tag(root, "select")
        .expect("select exists");
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
    let t_select = table_doc
        .find_element_by_tag(table_doc.root(), "select")
        .expect("select in table");
    assert_eq!(table_doc.text_content(t_select), "Table Opt");
}

#[test]
fn test_in_caption_and_in_column_group() {
    let html = "<table><caption>Table Title</caption><colgroup><col width=\"50\"><col width=\"100\"></colgroup><tr><td>Data</td></tr></table>";
    let doc = parse_html(html);
    let root = doc.root();
    let table_id = doc
        .find_element_by_tag(root, "table")
        .expect("table exists");

    let caption_id = doc
        .find_element_by_tag(table_id, "caption")
        .expect("caption exists");
    assert_eq!(doc.text_content(caption_id), "Table Title");

    let colgroup_id = doc
        .find_element_by_tag(table_id, "colgroup")
        .expect("colgroup exists");
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
    let t_id = doc
        .find_element_by_tag(root, "template")
        .expect("template element exists");

    // Direct text_content on template should be empty per WHATWG §4.12.3
    assert_eq!(doc.text_content(t_id), "");

    // template_contents should be a DocumentFragment containing div and p
    let frag_id = doc
        .template_contents(t_id)
        .expect("template_contents DocumentFragment exists");
    let frag_node = doc.get(frag_id).expect("fragment node exists");
    assert_eq!(frag_node.data, NodeData::DocumentFragment);

    let div_id = doc
        .find_element_by_tag(frag_id, "div")
        .expect("div inside fragment");
    assert_eq!(doc.text_content(div_id), "Hello inside template");

    let p_id = doc
        .find_element_by_tag(frag_id, "p")
        .expect("p inside fragment");
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
    let outer_t = doc
        .find_element_by_tag(root, "template")
        .expect("outer template exists");
    let outer_frag = doc
        .template_contents(outer_t)
        .expect("outer template contents");
    let outer_div = doc
        .find_element_by_tag(outer_frag, "div")
        .expect("outer div");

    let inner_t = doc
        .find_element_by_tag(outer_div, "template")
        .expect("inner template");
    let inner_frag = doc
        .template_contents(inner_t)
        .expect("inner template contents");
    let inner_span = doc
        .find_element_by_tag(inner_frag, "span")
        .expect("inner span");
    assert_eq!(doc.text_content(inner_span), "Nested");
}

#[test]
fn test_in_frameset_and_after_frameset() {
    let html = r#"<!DOCTYPE html><html><head><title>Frames</title></head><frameset cols="50%,50%"><frame src="a.html"><frame src="b.html"><noframes><p>No frames</p></noframes></frameset></html>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let html_id = doc
        .find_element_by_tag(root, "html")
        .expect("html element exists");
    let frameset_id = doc
        .find_element_by_tag(html_id, "frameset")
        .expect("frameset exists");
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
    let table_id = doc
        .find_element_by_tag(body_id, "table")
        .expect("table exists");

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
    assert!(
        div_pos < table_pos,
        "Misplaced div must appear BEFORE table in body due to foster parenting"
    );

    // Cell content remains in table
    let td_id = doc
        .find_element_by_tag(table_id, "td")
        .expect("td in table");
    assert_eq!(doc.text_content(td_id), "Cell Content");

    // Misplaced text also foster parented
    let text_html = "<table>Misplaced Text<tr><td>Cell</td></tr></table>";
    let text_doc = parse_html(text_html);
    let text_body = text_doc
        .find_element_by_tag(text_doc.root(), "body")
        .expect("body exists");
    let text_table = text_doc
        .find_element_by_tag(text_body, "table")
        .expect("table exists");
    let body_text = text_doc.text_content(text_body);
    assert!(body_text.contains("Misplaced Text"));
    let table_text = text_doc.text_content(text_table);
    assert!(
        !table_text.contains("Misplaced Text"),
        "Table must not contain misplaced text"
    );
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
    let ul_id = list_doc
        .find_element_by_tag(list_doc.root(), "ul")
        .expect("ul exists");
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
        assert_eq!(
            el.get_attribute("viewBox"),
            Some("0 0 100 100"),
            "viewBox attribute casing must be preserved"
        );
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
    assert_eq!(
        svg_children.len(),
        3,
        "SVG should contain 3 top-level elements: circle, rect, linearGradient"
    );
    assert_eq!(svg_children[0].0, "circle");
    assert_eq!(svg_children[1].0, "rect");
    assert_eq!(
        svg_children[2].0, "linearGradient",
        "linearGradient tag name casing must be preserved"
    );

    // <p>After SVG</p> is sibling of <svg> in <body>
    let p_id = doc
        .find_element_by_tag(body_id, "p")
        .expect("p exists in body");
    assert_eq!(doc.text_content(p_id), "After SVG");
}

#[test]
fn test_dialog_and_ruby_tree_building() {
    let html = "<p>Intro<dialog open>Dialog text</dialog><p>Outro</p>";
    let doc = parse_html(html);
    let root = doc.root();
    let body_id = doc.find_element_by_tag(root, "body").expect("body exists");
    let dialog_id = doc
        .find_element_by_tag(body_id, "dialog")
        .expect("dialog in body");
    assert_eq!(doc.text_content(dialog_id), "Dialog text");

    let ruby_html = "<ruby>漢<rp>(<rt>かん<rp>)<rt>kan</ruby>";
    let rdoc = parse_html(ruby_html);
    let rroot = rdoc.root();
    let rbody_id = rdoc
        .find_element_by_tag(rroot, "body")
        .expect("body exists");
    let ruby_id = rdoc
        .find_element_by_tag(rbody_id, "ruby")
        .expect("ruby in body");
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
    let select_id = doc
        .find_element_by_tag(body_id, "select")
        .expect("select in body");

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
    assert_eq!(
        select_children.len(),
        4,
        "Select should have 4 children: option, option, optgroup, optgroup"
    );
    assert_eq!(select_children[0].0, "option");
    assert_eq!(select_children[1].0, "option");
    assert_eq!(select_children[2].0, "optgroup");
    assert_eq!(select_children[3].0, "optgroup");

    // Group A should have 2 options
    let group_a_children: Vec<_> = doc
        .children(select_children[2].1)
        .filter(|n| matches!(&n.data, NodeData::Element(_)))
        .collect();
    assert_eq!(
        group_a_children.len(),
        2,
        "Group A should contain 2 options"
    );
}

#[test]
fn test_meta_charset_detection() {
    let html =
        r#"<!DOCTYPE html><html><head><meta charset="windows-1252"></head><body></body></html>"#;
    let doc = parse_html(html);
    assert_eq!(doc.character_set, "windows-1252");

    let html_utf8 =
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"></head><body></body></html>"#;
    let doc2 = parse_html(html_utf8);
    assert_eq!(doc2.character_set, "UTF-8");

    let html_latin1 =
        r#"<!DOCTYPE html><html><head><meta charset="iso-8859-1"></head><body></body></html>"#;
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

#[test]
fn test_declarative_shadow_dom_parsing() {
    let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <my-card id="card">
                        <template shadowrootmode="open" shadowrootclonable>
                            <style>:host { display: block; }</style>
                            <div class="card-inner">
                                <h2>Shadow Title</h2>
                                <slot name="desc"></slot>
                            </div>
                        </template>
                        <p slot="desc" id="slotted-desc">Light DOM slotted content</p>
                    </my-card>
                </body>
            </html>
        "#;
    let doc = parse_html(html);
    let root = doc.root();
    let body = doc
        .find_element_by_tag(root, "body")
        .expect("body element exists");
    let card = doc
        .find_element_by_id(body, "card")
        .expect("card element exists");

    // 1. Verify shadow root was attached to host
    let sr_id = doc
        .get_shadow_root(card)
        .expect("shadow root attached to host");
    assert_eq!(doc.get_shadow_root_mode(card), Some("open"));

    // 2. Verify shadow root contains the template contents
    let inner_div = doc
        .find_element_by_tag(sr_id, "div")
        .expect("card-inner div inside shadow root");
    let title = doc
        .find_element_by_tag(inner_div, "h2")
        .expect("h2 in shadow root");
    assert_eq!(doc.text_content(title), "Shadow Title");

    // 3. Verify template element was detached from light DOM (not present in card's light children)
    assert!(doc.find_element_by_tag(card, "template").is_none());

    // 4. Verify light DOM slotted content remains intact in host
    let desc = doc
        .find_element_by_id(card, "slotted-desc")
        .expect("slotted-desc in light DOM");
    assert_eq!(doc.text_content(desc), "Light DOM slotted content");
}
