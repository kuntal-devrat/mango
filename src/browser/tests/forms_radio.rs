//! Tests for radio button grouping, form ownership, and unchecking behavior (BUG-004).

use super::super::forms::find_element_by_id;
use super::super::*;
use super::attr;
use mango_layout::FormControlHit;

#[test]
fn test_radio_buttons_in_same_form_different_fieldsets() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
        <html>
        <body>
            <form id="myForm">
                <fieldset>
                    <legend>Group A</legend>
                    <input id="r1" type="radio" name="plan" value="free" checked="true">
                </fieldset>
                <fieldset>
                    <legend>Group B</legend>
                    <input id="r2" type="radio" name="plan" value="pro">
                </fieldset>
            </form>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/test_radio_form".to_string(),
    );

    let (r1_id, r2_id) = {
        let doc = browser.js_runtime.as_ref().unwrap().document_ref();
        (
            find_element_by_id(&doc, "r1").expect("r1 exists"),
            find_element_by_id(&doc, "r2").expect("r2 exists"),
        )
    };

    assert_eq!(attr(&browser, r1_id, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r2_id, "checked").as_deref(), None);

    // Click r2
    browser.handle_form_control_click(FormControlHit {
        node_id: Some(r2_id),
        tag_name: "input".to_string(),
        form_type: "radio".to_string(),
        name: "plan".to_string(),
        value: "pro".to_string(),
        checked: false,
        click_offset_x: 0.0,
    });

    // r1 must be unchecked and r2 must be checked
    assert_eq!(attr(&browser, r1_id, "checked").as_deref(), None);
    assert_eq!(attr(&browser, r2_id, "checked").as_deref(), Some("true"));
}

#[test]
fn test_radio_buttons_without_form_document_scope() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
        <html>
        <body>
            <fieldset>
                <input id="doc_r1" type="radio" name="global_choice" value="1" checked="true">
            </fieldset>
            <div>
                <input id="doc_r2" type="radio" name="global_choice" value="2">
            </div>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/test_radio_doc".to_string(),
    );

    let (r1_id, r2_id) = {
        let doc = browser.js_runtime.as_ref().unwrap().document_ref();
        (
            find_element_by_id(&doc, "doc_r1").expect("doc_r1 exists"),
            find_element_by_id(&doc, "doc_r2").expect("doc_r2 exists"),
        )
    };

    assert_eq!(attr(&browser, r1_id, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r2_id, "checked").as_deref(), None);

    // Click doc_r2
    browser.handle_form_control_click(FormControlHit {
        node_id: Some(r2_id),
        tag_name: "input".to_string(),
        form_type: "radio".to_string(),
        name: "global_choice".to_string(),
        value: "2".to_string(),
        checked: false,
        click_offset_x: 0.0,
    });

    assert_eq!(attr(&browser, r1_id, "checked").as_deref(), None);
    assert_eq!(attr(&browser, r2_id, "checked").as_deref(), Some("true"));
}

#[test]
fn test_radio_buttons_in_different_forms_do_not_interfere() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
        <html>
        <body>
            <form id="formA">
                <input id="rA1" type="radio" name="same_name" value="a1" checked="true">
                <input id="rA2" type="radio" name="same_name" value="a2">
            </form>
            <form id="formB">
                <input id="rB1" type="radio" name="same_name" value="b1" checked="true">
                <input id="rB2" type="radio" name="same_name" value="b2">
            </form>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/test_diff_forms".to_string(),
    );

    let (ra1, ra2, rb1, rb2) = {
        let doc = browser.js_runtime.as_ref().unwrap().document_ref();
        (
            find_element_by_id(&doc, "rA1").expect("rA1 exists"),
            find_element_by_id(&doc, "rA2").expect("rA2 exists"),
            find_element_by_id(&doc, "rB1").expect("rB1 exists"),
            find_element_by_id(&doc, "rB2").expect("rB2 exists"),
        )
    };

    assert_eq!(attr(&browser, ra1, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, ra2, "checked").as_deref(), None);
    assert_eq!(attr(&browser, rb1, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, rb2, "checked").as_deref(), None);

    // Click rA2 in Form A
    browser.handle_form_control_click(FormControlHit {
        node_id: Some(ra2),
        tag_name: "input".to_string(),
        form_type: "radio".to_string(),
        name: "same_name".to_string(),
        value: "a2".to_string(),
        checked: false,
        click_offset_x: 0.0,
    });

    // In Form A: rA1 is unchecked, rA2 is checked
    assert_eq!(attr(&browser, ra1, "checked").as_deref(), None);
    assert_eq!(attr(&browser, ra2, "checked").as_deref(), Some("true"));

    // In Form B: rb1 MUST STILL BE CHECKED! Form A click must not affect Form B!
    assert_eq!(attr(&browser, rb1, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, rb2, "checked").as_deref(), None);
}

#[test]
fn test_radio_buttons_form_vs_document_scope_isolation() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
        <html>
        <body>
            <form id="formScope">
                <input id="rInForm" type="radio" name="opt" value="form" checked="true">
            </form>
            <div>
                <input id="rOutside" type="radio" name="opt" value="outside">
            </div>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/test_form_vs_doc".to_string(),
    );

    let (r_form, r_out) = {
        let doc = browser.js_runtime.as_ref().unwrap().document_ref();
        (
            find_element_by_id(&doc, "rInForm").expect("rInForm exists"),
            find_element_by_id(&doc, "rOutside").expect("rOutside exists"),
        )
    };

    assert_eq!(attr(&browser, r_form, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r_out, "checked").as_deref(), None);

    // Click rOutside
    browser.handle_form_control_click(FormControlHit {
        node_id: Some(r_out),
        tag_name: "input".to_string(),
        form_type: "radio".to_string(),
        name: "opt".to_string(),
        value: "outside".to_string(),
        checked: false,
        click_offset_x: 0.0,
    });

    // rOutside is now checked, but rInForm must remain checked because it belongs to formScope!
    assert_eq!(attr(&browser, r_out, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r_form, "checked").as_deref(), Some("true"));
}

#[test]
fn test_radio_button_explicit_form_attribute() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
        <html>
        <body>
            <form id="ownerForm">
                <input id="rInside" type="radio" name="plan" value="monthly" checked="true">
            </form>
            <div>
                <!-- Located outside the <form>, but explicitly owned by ownerForm -->
                <input id="rLinked" type="radio" name="plan" value="yearly" form="ownerForm">
            </div>
        </body>
        </html>
    "#;
    browser.load_html(
        html.to_string(),
        "http://localhost/test_form_attr".to_string(),
    );

    let (r_in, r_linked) = {
        let doc = browser.js_runtime.as_ref().unwrap().document_ref();
        (
            find_element_by_id(&doc, "rInside").expect("rInside exists"),
            find_element_by_id(&doc, "rLinked").expect("rLinked exists"),
        )
    };

    assert_eq!(attr(&browser, r_in, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r_linked, "checked").as_deref(), None);

    // Click rLinked (which targets ownerForm)
    browser.handle_form_control_click(FormControlHit {
        node_id: Some(r_linked),
        tag_name: "input".to_string(),
        form_type: "radio".to_string(),
        name: "plan".to_string(),
        value: "yearly".to_string(),
        checked: false,
        click_offset_x: 0.0,
    });

    // rLinked is now checked, and rInside was unchecked because they share the same form owner!
    assert_eq!(attr(&browser, r_linked, "checked").as_deref(), Some("true"));
    assert_eq!(attr(&browser, r_in, "checked").as_deref(), None);
}
