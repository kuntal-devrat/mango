use mango_core::Color;
use mango_html::parse_html;

use crate::cascade::resolver::{resolve_cascade, resolve_cascade_with_index};
use crate::cascade::rule_index::RuleIndex;
use crate::cascade::types::Origin;
use crate::parser::parse_stylesheet;
use crate::values::{Display, Length, Value};

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
fn test_cdata_stylesheet_parsing() {
    let css = "<![CDATA[ div { background-color: green; } ]]>";
    let tokens = crate::tokenizer::CssTokenizer::tokenize(css);
    println!("Tokens: {:?}", tokens);
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
}

#[test]
fn test_presentational_hints() {
    let html = r##"<table id="hnmain" width="85%" bgcolor="#f6f6ef" cellpadding="0"><tr><td bgcolor="ff6600">News</td></tr></table>"##;
    let doc = parse_html(html);
    let root = doc.root();
    let table_id = doc.find_element_by_tag(root, "table").unwrap();
    let td_id = doc.find_element_by_tag(root, "td").unwrap();

    let table_styles = resolve_cascade(table_id, &doc, &[]);
    assert_eq!(
        table_styles.get("width"),
        Some(&Value::Length(Length::Percent(85.0)))
    );
    assert_eq!(
        table_styles.get("background-color"),
        Some(&Value::Color(Color::rgb(0xf6, 0xf6, 0xef)))
    );

    let td_styles = resolve_cascade(td_id, &doc, &[]);
    assert_eq!(
        td_styles.get("background-color"),
        Some(&Value::Color(Color::rgb(0xff, 0x66, 0x00)))
    );
    assert_eq!(
        td_styles.get("padding-top"),
        Some(&Value::Length(Length::Px(0.0)))
    );
}

#[test]
fn test_iframe_styles_and_presentational_hints() {
    let html = r#"<iframe id="f1" width="560" height="315" frameborder="0"></iframe>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let iframe_id = doc.find_element_by_tag(root, "iframe").unwrap();

    let styles = resolve_cascade(iframe_id, &doc, &[]);
    assert_eq!(styles.get("width"), Some(&Value::Length(Length::Px(560.0))));
    assert_eq!(
        styles.get("height"),
        Some(&Value::Length(Length::Px(315.0)))
    );
    assert_eq!(
        styles.get("border-top-style"),
        Some(&Value::BorderStyle(crate::values::BorderStyle::None))
    );
    assert_eq!(
        styles.get("border-top-width"),
        Some(&Value::Length(Length::Px(0.0)))
    );
    assert_eq!(
        styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
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
    assert_eq!(
        v1_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(
        v1_styles.get("width"),
        Some(&Value::Length(Length::Px(640.0)))
    );
    assert_eq!(
        v1_styles.get("height"),
        Some(&Value::Length(Length::Px(360.0)))
    );

    // 2. Audio without controls is display: none
    let a1_id = doc.find_element_by_id(root, "a1").unwrap();
    let a1_styles = resolve_cascade(a1_id, &doc, &[]);
    assert_eq!(
        a1_styles.get("display"),
        Some(&Value::Display(Display::None))
    );

    // 3. Audio with controls is display: inline-block with 300x36 dimensions
    let a2_id = doc.find_element_by_id(root, "a2").unwrap();
    let a2_styles = resolve_cascade(a2_id, &doc, &[]);
    assert_eq!(
        a2_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(
        a2_styles.get("width"),
        Some(&Value::Length(Length::Px(300.0)))
    );
    assert_eq!(
        a2_styles.get("height"),
        Some(&Value::Length(Length::Px(36.0)))
    );
}

#[test]
fn test_canvas_ua_styles_and_presentational_hints() {
    let html = r#"
        <canvas id="c1"></canvas>
        <canvas id="c2" width="500" height="250"></canvas>
    "#;
    let doc = parse_html(html);
    let root = doc.root();

    // 1. Default canvas: inline-block, width/height auto (intrinsic dimensions handled in layout)
    let c1_id = doc.find_element_by_id(root, "c1").unwrap();
    let c1_styles = resolve_cascade(c1_id, &doc, &[]);
    assert_eq!(
        c1_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(c1_styles.get("width"), None);
    assert_eq!(c1_styles.get("height"), None);

    // 2. Canvas with HTML width/height attributes (coordinate space, not CSS presentational hints)
    let c2_id = doc.find_element_by_id(root, "c2").unwrap();
    let c2_styles = resolve_cascade(c2_id, &doc, &[]);
    assert_eq!(
        c2_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(c2_styles.get("width"), None);
    assert_eq!(c2_styles.get("height"), None);
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

    assert_eq!(
        styles.get("margin-top"),
        Some(&Value::Length(Length::Px(0.0)))
    );
    assert_eq!(styles.get("color"), Some(&Value::Color(Color::RED)));
    assert_eq!(
        styles.get("background-color"),
        Some(&Value::Color(Color::BLUE))
    );
    assert_eq!(
        styles.get("font-size"),
        Some(&Value::Length(Length::Px(20.0)))
    );
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
    assert!(
        matches!(
            d1_styles.get("display"),
            Some(Value::Display(crate::values::Display::None))
        ) || d1_styles.get("display") == Some(&Value::Display(Display::None))
    );

    let d2_id = doc.find_element_by_id(root, "d2").unwrap();
    let d2_styles = resolve_cascade(d2_id, &doc, &[]);
    assert_eq!(
        d2_styles.get("display"),
        Some(&Value::Display(crate::values::Display::Block))
    );

    // 2. progress & meter dimensions
    let p1_id = doc.find_element_by_id(root, "p1").unwrap();
    let p1_styles = resolve_cascade(p1_id, &doc, &[]);
    assert_eq!(
        p1_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(
        p1_styles.get("width"),
        Some(&Value::Length(Length::Px(160.0)))
    );
    assert_eq!(
        p1_styles.get("height"),
        Some(&Value::Length(Length::Px(16.0)))
    );

    let m1_id = doc.find_element_by_id(root, "m1").unwrap();
    let m1_styles = resolve_cascade(m1_id, &doc, &[]);
    assert_eq!(
        m1_styles.get("display"),
        Some(&Value::Display(crate::values::Display::InlineBlock))
    );
    assert_eq!(
        m1_styles.get("width"),
        Some(&Value::Length(Length::Px(80.0)))
    );
    assert_eq!(
        m1_styles.get("height"),
        Some(&Value::Length(Length::Px(16.0)))
    );

    // 3. datalist is display: none
    let dl1_id = doc.find_element_by_id(root, "dl1").unwrap();
    let dl1_styles = resolve_cascade(dl1_id, &doc, &[]);
    assert!(
        matches!(
            dl1_styles.get("display"),
            Some(Value::Display(crate::values::Display::None))
        ) || dl1_styles.get("display") == Some(&Value::Display(Display::None))
    );

    // 4. ruby: rp is display: none, rt is font-size 50%
    let rt1_id = doc.find_element_by_id(root, "rt1").unwrap();
    let rt1_styles = resolve_cascade(rt1_id, &doc, &[]);
    assert_eq!(
        rt1_styles.get("font-size"),
        Some(&Value::Length(Length::Percent(50.0)))
    );

    let rp1_id = doc.find_element_by_id(root, "rp1").unwrap();
    let rp1_styles = resolve_cascade(rp1_id, &doc, &[]);
    assert!(
        matches!(
            rp1_styles.get("display"),
            Some(Value::Display(crate::values::Display::None))
        ) || rp1_styles.get("display") == Some(&Value::Display(Display::None))
    );

    // 5. bdi & bdo unicode-bidi and direction
    let bdi1_id = doc.find_element_by_id(root, "bdi1").unwrap();
    let bdi1_styles = resolve_cascade(bdi1_id, &doc, &[]);
    assert_eq!(
        bdi1_styles.get("unicode-bidi"),
        Some(&Value::UnicodeBidi(crate::values::UnicodeBidi::Isolate))
    );

    let bdo1_id = doc.find_element_by_id(root, "bdo1").unwrap();
    let bdo1_styles = resolve_cascade(bdo1_id, &doc, &[]);
    assert_eq!(
        bdo1_styles.get("unicode-bidi"),
        Some(&Value::UnicodeBidi(
            crate::values::UnicodeBidi::BidiOverride
        ))
    );
    assert_eq!(
        bdo1_styles.get("direction"),
        Some(&Value::Direction(crate::values::Direction::Rtl))
    );
}

#[test]
fn test_multi_selector_max_specificity() {
    let html = r#"<h1 id="hero" class="title">Hello World</h1>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let h1_id = doc.find_element_by_id(root, "hero").unwrap();

    let css = r#"
        h1, #hero {
            color: red;
        }
        .title {
            color: blue;
        }
    "#;
    let sheet = parse_stylesheet(css);
    let cascaded = resolve_cascade(h1_id, &doc, &[&sheet]);
    assert_eq!(
        cascaded.get("color"),
        Some(&Value::Color(Color::RED)),
        "Multi-selector rule must use max specificity (1, 0, 0) over class (0, 1, 0)"
    );
}

#[test]
fn test_layer_cascade_priority() {
    let html = r#"<div id="target" class="item">Test</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_id(root, "target").unwrap();

    // Test 1: Unlayered normal beats layered normal, even if layered has higher specificity
    let css1 = r#"
        @layer base {
            #target { color: blue; }
        }
        .item { color: red; }
    "#;
    let sheet1 = parse_stylesheet(css1);
    let cascaded1 = resolve_cascade(div_id, &doc, &[&sheet1]);
    assert_eq!(
        cascaded1.get("color"),
        Some(&Value::Color(Color::RED)),
        "Unlayered normal rule must override layered normal rule"
    );

    // Test 2: Later layer beats earlier layer
    let css2 = r#"
        @layer first {
            .item { color: red; }
        }
        @layer second {
            .item { color: green; }
        }
    "#;
    let sheet2 = parse_stylesheet(css2);
    let cascaded2 = resolve_cascade(div_id, &doc, &[&sheet2]);
    assert_eq!(
        cascaded2.get("color"),
        Some(&Value::Color(Color::GREEN)),
        "Later declared layer must beat earlier declared layer"
    );

    // Test 3: In !important, layered beats unlayered, and earlier layer beats later layer!
    let css3 = r#"
        @layer first {
            .item { color: red !important; }
        }
        @layer second {
            .item { color: green !important; }
        }
        .item { color: blue !important; }
    "#;
    let sheet3 = parse_stylesheet(css3);
    let cascaded3 = resolve_cascade(div_id, &doc, &[&sheet3]);
    assert_eq!(
        cascaded3.get("color"),
        Some(&Value::Color(Color::RED)),
        "In !important, earlier layer must beat later layer and unlayered"
    );
}

#[test]
fn test_css_nesting() {
    let html = r#"<div id="c1" class="card"><span id="t1" class="title">Hello</span><div id="c2" class="card active"><span id="b1" class="badge">Active</span></div></div>"#;
    let doc = parse_html(html);
    let root = doc.root();

    let css = r#"
        .card {
            background-color: white;
            & .title {
                color: red;
            }
            &.active {
                background-color: green;
            }
            span {
                font-size: 14px;
            }
        }
    "#;
    let sheet = parse_stylesheet(css);
    assert!(!sheet.rules.is_empty(), "Parsed sheet should contain rules");

    let title_id = doc
        .find_element_by_id(root, "t1")
        .expect("Should find title element");
    let active_id = doc
        .find_element_by_id(root, "c2")
        .expect("Should find active card");

    let cascaded_title = resolve_cascade(title_id, &doc, &[&sheet]);
    assert_eq!(
        cascaded_title.get("color"),
        Some(&Value::Color(Color::RED)),
        "Nested selector & .title must resolve color: red"
    );
    assert_eq!(
        cascaded_title.get("font-size"),
        Some(&Value::Length(Length::Px(14.0))),
        "Nested selector span without & must resolve font-size: 14px"
    );

    let cascaded_active = resolve_cascade(active_id, &doc, &[&sheet]);
    assert_eq!(
        cascaded_active.get("background-color"),
        Some(&Value::Color(Color::GREEN)),
        "Nested selector &.active must resolve background-color: green"
    );
}

#[test]
fn test_layer_statement_order() {
    let html = r#"<div id="target" class="item">Test</div>"#;
    let doc = parse_html(html);
    let root = doc.root();
    let div_id = doc.find_element_by_id(root, "target").unwrap();

    let css = r#"
        @layer base, components;
        @layer components {
            .item { color: green; }
        }
        @layer base {
            .item { color: red; }
        }
    "#;
    let sheet = parse_stylesheet(css);
    let cascaded = resolve_cascade(div_id, &doc, &[&sheet]);
    assert_eq!(
        cascaded.get("color"),
        Some(&Value::Color(Color::GREEN)),
        "components layer must beat base layer even though base block appears later in source"
    );
}

#[test]
fn test_scope_and_doughnut_hole() {
    let html = r#"
        <div id="card" class="card">
            <p id="in-scope">In scope</p>
            <div id="fig" class="figure">
                <p id="in-hole">In doughnut hole</p>
            </div>
        </div>
        <p id="outside">Outside card</p>
    "#;
    let doc = parse_html(html);
    let root = doc.root();
    let card_id = doc.find_element_by_id(root, "card").unwrap();
    let in_scope_id = doc.find_element_by_id(root, "in-scope").unwrap();
    let in_hole_id = doc.find_element_by_id(root, "in-hole").unwrap();
    let outside_id = doc.find_element_by_id(root, "outside").unwrap();

    let css = r#"
        @scope (.card) to (.figure) {
            :scope {
                border-top-width: 5px;
            }
            p {
                color: blue;
            }
        }
        p {
            color: black;
        }
    "#;
    let sheet = parse_stylesheet(css);

    // 1. :scope matches .card
    let card_cascaded = resolve_cascade(card_id, &doc, &[&sheet]);
    assert_eq!(
        card_cascaded.get("border-top-width"),
        Some(&Value::Length(Length::Px(5.0))),
        ":scope must match scoping root .card"
    );

    // 2. p inside .card and outside .figure matches color: blue
    let in_scope_cascaded = resolve_cascade(in_scope_id, &doc, &[&sheet]);
    assert_eq!(
        in_scope_cascaded.get("color"),
        Some(&Value::Color(Color::BLUE)),
        "p in scope must resolve color: blue"
    );

    // 3. p inside .figure is in doughnut hole -> does not match scoped rule, gets black
    let in_hole_cascaded = resolve_cascade(in_hole_id, &doc, &[&sheet]);
    assert_eq!(
        in_hole_cascaded.get("color"),
        Some(&Value::Color(Color::BLACK)),
        "p in doughnut hole must NOT match scoped rule"
    );

    // 4. p outside .card does not match scoped rule, gets black
    let outside_cascaded = resolve_cascade(outside_id, &doc, &[&sheet]);
    assert_eq!(
        outside_cascaded.get("color"),
        Some(&Value::Color(Color::BLACK)),
        "p outside scope root must NOT match scoped rule"
    );
}

#[test]
fn test_where_descendant_matching() {
    let css = r#"
        .prc-Button-ButtonBase-9n-Xk :where(.prc-Button-ButtonContent-Iohp5) {
            display: grid;
        }
    "#;
    let sheet = parse_stylesheet(css);
    let doc = parse_html(
        r#"
        <button class="prc-Button-ButtonBase-9n-Xk">
            <span class="prc-Button-ButtonContent-Iohp5"></span>
        </button>
    "#,
    );
    let span = doc.find_element_by_tag(doc.root(), "span").unwrap();
    let styles = resolve_cascade(span, &doc, &[&sheet]);
    assert_eq!(
        styles.get("display"),
        Some(&Value::Display(crate::values::Display::Grid))
    );
}
