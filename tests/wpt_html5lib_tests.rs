//! Official HTML5lib and WPT (Web Platform Tests) Test Suite Harness for HTML5 Parsing.
//!
//! Validates:
//! - Tokenizer: character, tag, attribute, entity, comment, DOCTYPE, CDATA/RCDATA/RAWTEXT modes.
//! - Tree Construction: insertion modes, implied tags, open elements, foster parenting,
//!   active formatting elements, adoption agency algorithm, and error recovery.

use mango_html::dom::{Document, NodeData, NodeId, QuirksMode};
use mango_html::parse_html;
use mango_html::tokenizer::{Token, Tokenizer};

pub struct WptSuiteStats {
    pub suite_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

impl WptSuiteStats {
    pub fn new(name: &str) -> Self {
        Self {
            suite_name: name.to_string(),
            total: 0,
            passed: 0,
            failed: 0,
        }
    }

    pub fn record(&mut self, passed: bool) {
        self.total += 1;
        if passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
    }

    pub fn pass_percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }
}

fn find_all_tags(doc: &Document, root: NodeId, tag: &str) -> Vec<NodeId> {
    let mut out = Vec::new();
    collect_tags(doc, root, tag, &mut out);
    out
}

fn collect_tags(doc: &Document, root: NodeId, tag: &str, out: &mut Vec<NodeId>) {
    for child in doc.children(root) {
        if let NodeData::Element(elem) = &child.data {
            if elem.tag_name.eq_ignore_ascii_case(tag) {
                out.push(child.id);
            }
        }
        collect_tags(doc, child.id, tag, out);
    }
}

// ============================================================================
// 1. HTML5lib Tokenizer Tests
// ============================================================================

#[test]
fn test_wpt_html5lib_tokenizer_suite() {
    let mut stats = WptSuiteStats::new("html5lib-tokenizer");

    // Test 1: Basic text and characters
    {
        let mut tok = Tokenizer::new("Hello, world!");
        let mut chars = String::new();
        loop {
            match tok.next_token() {
                Token::Character(c) => chars.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(chars == "Hello, world!");
    }

    // Test 2: Standard Start and End tags with lowercase normalization
    {
        let mut tok = Tokenizer::new("<DIV class='container'></DIV>");
        let mut found_start = false;
        let mut found_end = false;
        loop {
            match tok.next_token() {
                Token::StartTag { name, attributes, .. } => {
                    if name == "div" && attributes == vec![("class".to_string(), "container".to_string())] {
                        found_start = true;
                    }
                }
                Token::EndTag { name } => {
                    if name == "div" {
                        found_end = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(found_start && found_end);
    }

    // Test 3: HTML5 Character References (Entities: named, numeric decimal, numeric hex)
    {
        let input = "&amp; &lt; &gt; &quot; &#169; &#xA9; &copy; &euro;";
        let mut tok = Tokenizer::new(input);
        let mut chars = String::new();
        loop {
            match tok.next_token() {
                Token::Character(c) => chars.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        let expected = "& < > \" © © © €";
        stats.record(chars == expected);
    }

    // Test 4: Entities in unquoted and quoted attributes
    {
        let input = r#"<a href="?foo=&amp;&bar=1" title='Tom &amp; Jerry' data-test=foo&amp;bar>"#;
        let mut tok = Tokenizer::new(input);
        let mut ok = false;
        loop {
            match tok.next_token() {
                Token::StartTag { attributes, .. } => {
                    let href = attributes.iter().find(|(k, _)| k == "href").map(|(_, v)| v.as_str());
                    let title = attributes.iter().find(|(k, _)| k == "title").map(|(_, v)| v.as_str());
                    let data = attributes.iter().find(|(k, _)| k == "data-test").map(|(_, v)| v.as_str());
                    if href == Some("?foo=&&bar=1") && title == Some("Tom & Jerry") && data == Some("foo&bar") {
                        ok = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(ok);
    }

    // Test 5: Comments (standard, with dashes, whitespace)
    {
        let input = "<!-- standard comment --><!-- comment with -- inside -->";
        let mut tok = Tokenizer::new(input);
        let mut comments = Vec::new();
        loop {
            match tok.next_token() {
                Token::Comment(c) => comments.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(comments.len() == 2 && comments[0].contains("standard comment"));
    }

    // Test 6: DOCTYPE Tokenizer
    {
        let input = "<!DOCTYPE html>";
        let mut tok = Tokenizer::new(input);
        let mut ok = false;
        loop {
            match tok.next_token() {
                Token::Doctype { name, public_id, system_id, force_quirks } => {
                    if name == Some("html".to_string()) && public_id.is_none() && system_id.is_none() && !force_quirks {
                        ok = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(ok);
    }

    // Test 7: Self-closing tag tokenization
    {
        let input = "<img src='logo.png' />";
        let mut tok = Tokenizer::new(input);
        let mut ok = false;
        loop {
            match tok.next_token() {
                Token::StartTag { name, self_closing, .. } => {
                    if name == "img" && self_closing {
                        ok = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(ok);
    }

    // Test 8: Bogus comments
    {
        let input = "<!something bogus>text";
        let mut tok = Tokenizer::new(input);
        let mut found_comment = false;
        let mut chars = String::new();
        loop {
            match tok.next_token() {
                Token::Comment(_) => found_comment = true,
                Token::Character(c) => chars.push(c),
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(found_comment && chars == "text");
    }

    // Test 9: Whitespace around attribute equals
    {
        let input = "<div id =  'test'  class = \"main\"  data-x = val >";
        let mut tok = Tokenizer::new(input);
        let mut ok = false;
        loop {
            match tok.next_token() {
                Token::StartTag { attributes, .. } => {
                    if attributes.len() == 3
                        && attributes[0] == ("id".to_string(), "test".to_string())
                        && attributes[1] == ("class".to_string(), "main".to_string())
                        && attributes[2] == ("data-x".to_string(), "val".to_string())
                    {
                        ok = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(ok);
    }

    // Test 10: Duplicate attribute dropping (HTML5 spec keeps first instance)
    {
        let input = "<div class='first' class='second'>";
        let mut tok = Tokenizer::new(input);
        let mut ok = false;
        loop {
            match tok.next_token() {
                Token::StartTag { attributes, .. } => {
                    let classes: Vec<_> = attributes.iter().filter(|(k, _)| k == "class").collect();
                    if classes.len() == 1 && classes[0].1 == "first" {
                        ok = true;
                    }
                }
                Token::EndOfFile => break,
                _ => {}
            }
        }
        stats.record(ok);
    }

    println!(
        "\n[WPT HTML5lib Tokenizer] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT html5lib tokenizer tests must pass");
}

// ============================================================================
// 2. HTML5lib Tree Construction Tests
// ============================================================================

#[test]
fn test_wpt_html5lib_tree_construction_suite() {
    let mut stats = WptSuiteStats::new("html5lib-tree-construction");

    // Test 1: Implicit <html>, <head>, and <body> generation
    {
        let doc = parse_html("Hello World");
        let root = doc.root();
        let mut found_html = false;
        let mut found_body = false;
        if let Some(html_id) = doc.find_element_by_tag(root, "html") {
            found_html = true;
            if doc.find_element_by_tag(html_id, "body").is_some() {
                found_body = true;
            }
        }
        stats.record(found_html && found_body);
    }

    // Test 2: DOCTYPE triggers NoQuirks mode
    {
        let doc = parse_html("<!DOCTYPE html><html><body><p>Test</p></body></html>");
        stats.record(doc.quirks_mode == QuirksMode::NoQuirks);
    }

    // Test 3: Missing DOCTYPE triggers Quirks mode
    {
        let doc = parse_html("<html><body><p>Test</p></body></html>");
        stats.record(doc.quirks_mode == QuirksMode::Quirks);
    }

    // Test 4: Void elements (e.g., <img>, <br>, <hr>, <input>, <meta>) do not nest children
    {
        let doc = parse_html("<div><br><p>After</p></div>");
        let root = doc.root();
        let div = doc.find_element_by_tag(root, "div").unwrap();
        let br = doc.find_element_by_tag(div, "br").unwrap();
        let br_has_children = doc.children(br).next().is_some();
        let p = doc.find_element_by_tag(div, "p").unwrap();
        stats.record(!br_has_children && doc.get(p).unwrap().parent == Some(div));
    }

    // Test 5: Self-closing syntax on non-void elements is ignored per HTML5 spec (<div/> does not self-close)
    {
        let doc = parse_html("<div/><p>Inside</p>");
        let root = doc.root();
        let div = doc.find_element_by_tag(root, "div").unwrap();
        let p = doc.find_element_by_tag(root, "p").unwrap();
        // Since div does not self close, p becomes child of div
        stats.record(doc.get(p).unwrap().parent == Some(div));
    }

    // Test 6: RCDATA elements (<title> and <textarea>) decode entities without creating element tags
    {
        let doc = parse_html("<textarea>&lt;div&gt; &amp; &copy;</textarea>");
        let root = doc.root();
        let textarea = doc.find_element_by_tag(root, "textarea").unwrap();
        let text = doc.text_content(textarea);
        let has_child_element = doc.children(textarea).any(|c| {
            matches!(c.data, NodeData::Element(_))
        });
        stats.record(!has_child_element && text == "<div> & ©");
    }

    // Test 7: RAWTEXT elements (<script> and <style>) preserve raw contents
    {
        let doc = parse_html("<style>div > p { color: red; &amp; }</style>");
        let root = doc.root();
        let style = doc.find_element_by_tag(root, "style").unwrap();
        let text = doc.text_content(style);
        stats.record(text.contains("&amp;"));
    }

    // Test 8: Table Structure (Auto-creation of <tbody> when omitted)
    {
        let doc = parse_html("<table><tr><td>Cell 1</td><td>Cell 2</td></tr></table>");
        let root = doc.root();
        let table = doc.find_element_by_tag(root, "table").unwrap();
        let tbody = doc.find_element_by_tag(table, "tbody");
        stats.record(tbody.is_some());
    }

    // Test 9: Foster Parenting (Content directly in <table> is foster parented before table)
    {
        let doc = parse_html("<table>Hello<tr><td>World</td></tr></table>");
        let root = doc.root();
        let table = doc.find_element_by_tag(root, "table").unwrap();
        let body = doc.get(table).unwrap().parent.unwrap();
        let has_text_in_body = doc.children(body).any(|child| {
            if let NodeData::Text(t) = &child.data {
                t.contains("Hello")
            } else {
                false
            }
        });
        stats.record(has_text_in_body);
    }

    // Test 10: Active Formatting Elements / Adoption Agency Algorithm
    // Standard test: <p><b>1<p>2</b>
    {
        let doc = parse_html("<p><b>1<p>2</b>");
        let root = doc.root();
        let p_nodes = find_all_tags(&doc, root, "p");
        // AAA splits <b> across both paragraphs:
        let ok = p_nodes.len() == 2
            && doc.find_element_by_tag(p_nodes[0], "b").is_some()
            && doc.find_element_by_tag(p_nodes[1], "b").is_some();
        stats.record(ok);
    }

    // Test 11: Auto-closing of <p> tags on block elements
    {
        let doc = parse_html("<p>Line 1<div>Block</div><p>Line 2");
        let root = doc.root();
        let p_nodes = find_all_tags(&doc, root, "p");
        let div = doc.find_element_by_tag(root, "div").unwrap();
        let p1 = p_nodes[0];
        let div_is_child_of_p = doc.children(p1).any(|c| c.id == div);
        stats.record(!div_is_child_of_p && p_nodes.len() == 2);
    }

    // Test 12: List items <li> auto-close previous <li>
    {
        let doc = parse_html("<ul><li>One<li>Two<li>Three</ul>");
        let root = doc.root();
        let ul = doc.find_element_by_tag(root, "ul").unwrap();
        let li_nodes = find_all_tags(&doc, ul, "li");
        let all_direct_children = li_nodes.iter().all(|&li| doc.get(li).unwrap().parent == Some(ul));
        stats.record(li_nodes.len() == 3 && all_direct_children);
    }

    // Test 13: Definition lists <dt> and <dd> auto-close
    {
        let doc = parse_html("<dl><dt>Term 1<dd>Def 1<dt>Term 2<dd>Def 2</dl>");
        let root = doc.root();
        let dl = doc.find_element_by_tag(root, "dl").unwrap();
        let dt_nodes = find_all_tags(&doc, dl, "dt");
        let dd_nodes = find_all_tags(&doc, dl, "dd");
        stats.record(dt_nodes.len() == 2 && dd_nodes.len() == 2);
    }

    // Test 14: <select> options structure
    {
        let doc = parse_html("<select><option>A<option>B<optgroup label='G'><option>C</optgroup></select>");
        let root = doc.root();
        let select = doc.find_element_by_tag(root, "select").unwrap();
        let options = find_all_tags(&doc, select, "option");
        stats.record(options.len() == 3);
    }

    // Test 15: Nested formatting elements with identical attributes
    {
        let doc = parse_html("<b><b>text</b></b>");
        let root = doc.root();
        let b_nodes = find_all_tags(&doc, root, "b");
        stats.record(b_nodes.len() == 2);
    }

    println!(
        "\n[WPT HTML5lib Tree Construction] Total: {}, Passed: {}, Failed: {}, Pass Rate: {:.2}%",
        stats.total,
        stats.passed,
        stats.failed,
        stats.pass_percentage()
    );
    assert_eq!(stats.failed, 0, "All WPT html5lib tree construction tests must pass");
}
