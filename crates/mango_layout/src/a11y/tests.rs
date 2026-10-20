use super::*;

#[test]
fn test_accessibility_tree_roles_and_accname() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>A11y Test</title></head>
        <body>
            <header>
                <nav aria-label="Main Navigation">
                    <a href="/home">Home</a>
                    <a href="/about">About Us</a>
                </nav>
            </header>
            <main>
                <h1>Page Heading</h1>
                <button aria-label="Close dialog">X</button>
                <img src="pic.jpg" alt="A lovely sunrise" />
                <input type="checkbox" id="tos" checked />
                <label for="tos">Agree to Terms</label>
                <div role="slider" aria-valuenow="50" aria-valuemin="0" aria-valuemax="100"></div>
                <div aria-hidden="true">Hidden text</div>
            </main>
        </body>
        </html>
    "#;

    let doc = mango_html::parse_html(html);
    let a11y_tree = A11yTree::build(&doc, None);

    // Find by roles
    let navs = a11y_tree.find_by_role(&A11yRole::Navigation);
    assert_eq!(navs.len(), 1);
    assert_eq!(navs[0].name, "Main Navigation");

    let links = a11y_tree.find_by_role(&A11yRole::Link);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].name, "Home");
    assert_eq!(links[1].name, "About Us");

    let headings = a11y_tree.find_by_role(&A11yRole::Heading { level: 1 });
    assert_eq!(headings.len(), 1);
    assert_eq!(headings[0].name, "Page Heading");

    let buttons = a11y_tree.find_by_role(&A11yRole::Button);
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].name, "Close dialog");

    let images = a11y_tree.find_by_role(&A11yRole::Image);
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].name, "A lovely sunrise");

    let checkboxes = a11y_tree.find_by_role(&A11yRole::Checkbox);
    assert_eq!(checkboxes.len(), 1);
    assert_eq!(checkboxes[0].name, "Agree to Terms");
    assert_eq!(checkboxes[0].state.checked, Some(CheckedState::True));

    let sliders = a11y_tree.find_by_role(&A11yRole::Slider);
    assert_eq!(sliders.len(), 1);
    assert_eq!(sliders[0].state.value_now, Some(50.0));
    assert_eq!(sliders[0].state.value_min, Some(0.0));
    assert_eq!(sliders[0].state.value_max, Some(100.0));

    // Hidden content should not be present
    let hidden = a11y_tree.find_by_name("Hidden text");
    assert!(
        hidden.is_empty(),
        "aria-hidden elements must not appear in the accessibility tree"
    );

    // Dump tree check
    let dump = a11y_tree.dump_tree();
    assert!(dump.contains("[Navigation] \"Main Navigation\""));
    assert!(dump.contains("[Heading (level 1)] \"Page Heading\""));
    assert!(dump.contains("[Checkbox] \"Agree to Terms\" [checked]"));
}

#[test]
fn test_aria_labelledby_precedence_over_aria_label() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <span id="lbl">From Labelledby</span>
            <button aria-label="From Label" aria-labelledby="lbl">X</button>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let buttons = tree.find_by_role(&A11yRole::Button);
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].name, "From Labelledby");
}

#[test]
fn test_th_scope_row_becomes_row_header() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <table>
                <tr>
                    <th scope="row">Row Header</th>
                    <th scope="col">Column Header</th>
                    <th>Default Header</th>
                </tr>
            </table>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let row_headers = tree.find_by_role(&A11yRole::RowHeader);
    assert_eq!(row_headers.len(), 1);
    assert_eq!(row_headers[0].name, "Row Header");

    let col_headers = tree.find_by_role(&A11yRole::ColumnHeader);
    assert_eq!(col_headers.len(), 2);
}

#[test]
fn test_header_inside_article_is_generic() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <header>Top Banner</header>
            <article>
                <header>Article Header</header>
            </article>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let headers = tree.find_by_role(&A11yRole::Header);
    assert_eq!(headers.len(), 1);
}

#[test]
fn test_input_type_hidden_excluded() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <input type="hidden" name="csrf" value="abc123" />
            <input type="text" placeholder="Visible" />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let inputs = tree.find_by_role(&A11yRole::TextInput);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "Visible");
}

#[test]
fn test_select_multiple_is_listbox() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <select multiple>
                <option>A</option>
                <option>B</option>
            </select>
            <select>
                <option>C</option>
            </select>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let listboxes = tree.find_by_role(&A11yRole::ListBox);
    assert_eq!(listboxes.len(), 1);
    let comboboxes = tree.find_by_role(&A11yRole::ComboBox);
    assert_eq!(comboboxes.len(), 1);
}

#[test]
fn test_presentation_role_strips_semantics() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <table role="presentation">
                <tr><td>Cell</td></tr>
            </table>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let tables = tree.find_by_role(&A11yRole::Table);
    assert!(tables.is_empty());
    let presentations = tree.find_by_role(&A11yRole::Presentation);
    assert!(!presentations.is_empty());
}

#[test]
fn test_fieldset_legend_name() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <fieldset>
                <legend>Personal Info</legend>
                <input type="text" placeholder="Name" />
            </fieldset>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let groups = tree.find_by_role(&A11yRole::Group);
    assert!(!groups.is_empty());
    assert_eq!(groups[0].name, "Personal Info");
}

#[test]
fn test_progress_bar_role() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <progress value="70" max="100">70%</progress>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let bars = tree.find_by_role(&A11yRole::ProgressBar);
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].state.value_now, Some(70.0));
    assert_eq!(bars[0].state.value_max, Some(100.0));
}

#[test]
fn test_details_summary_expanded() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <details open>
                <summary>More Info</summary>
                <p>Details content here.</p>
            </details>
            <details>
                <summary>Closed Section</summary>
                <p>Hidden content.</p>
            </details>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);

    let groups = tree.find_by_role(&A11yRole::Group);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].state.expanded, Some(true));
    assert_eq!(groups[0].name, "More Info");
    assert_eq!(groups[1].state.expanded, Some(false));
}

#[test]
fn test_live_region_properties() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <div role="status" aria-live="polite" aria-atomic="true">Loading...</div>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let statuses = tree.find_by_role(&A11yRole::Status);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].state.live, Some("polite".to_string()));
    assert_eq!(statuses[0].state.atomic, Some(true));
}

#[test]
fn test_aria_describedby() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <span id="desc">Password must be 8+ chars</span>
            <input type="password" aria-label="Password" aria-describedby="desc" />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let inputs = tree.find_by_role(&A11yRole::TextInput);
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].name, "Password");
    assert_eq!(
        inputs[0].description,
        Some("Password must be 8+ chars".to_string())
    );
}

#[test]
fn test_img_empty_alt_is_presentation() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <img src="decorative.png" alt="" />
            <img src="meaningful.png" alt="Chart" />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let images = tree.find_by_role(&A11yRole::Image);
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].name, "Chart");
}

#[test]
fn test_focusable_state() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <button>Click Me</button>
            <div tabindex="0">Focusable Div</div>
            <div>Not Focusable</div>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);

    let buttons = tree.find_by_role(&A11yRole::Button);
    assert_eq!(buttons.len(), 1);
    assert!(buttons[0].state.focusable);

    let generics = tree.find_by_role(&A11yRole::Generic);
    let focusable_generics: Vec<_> = generics.iter().filter(|n| n.state.focusable).collect();
    let non_focusable_generics: Vec<_> = generics.iter().filter(|n| !n.state.focusable).collect();

    assert!(
        !focusable_generics.is_empty(),
        "div with tabindex=0 should be focusable"
    );
    assert!(
        !non_focusable_generics.is_empty(),
        "plain div should not be focusable"
    );
}

#[test]
fn test_table_caption_name() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <table>
                <caption>Monthly Sales</caption>
                <tr><td>100</td></tr>
            </table>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let tables = tree.find_by_role(&A11yRole::Table);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "Monthly Sales");
}

#[test]
fn test_separator_role() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <hr />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let seps = tree.find_by_role(&A11yRole::Separator);
    assert_eq!(seps.len(), 1);
}

#[test]
fn test_form_with_label_is_landmark() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <form aria-label="Search form">
                <input type="search" />
            </form>
            <form>
                <input type="text" />
            </form>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let forms = tree.find_by_role(&A11yRole::Form);
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].name, "Search form");
}

#[test]
fn test_aside_is_complementary() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <aside>Sidebar content</aside>
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let complementary = tree.find_by_role(&A11yRole::Complementary);
    assert_eq!(complementary.len(), 1);
}

#[test]
fn test_input_type_image_is_button() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <input type="image" alt="Submit form" src="btn.png" />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let buttons = tree.find_by_role(&A11yRole::Button);
    assert_eq!(buttons.len(), 1);
    assert_eq!(buttons[0].name, "Submit form");
}

#[test]
fn test_checked_mixed_state() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test</title></head>
        <body>
            <input type="checkbox" aria-checked="mixed" aria-label="Select all" />
        </body>
        </html>
    "#;
    let doc = mango_html::parse_html(html);
    let tree = A11yTree::build(&doc, None);
    let checkboxes = tree.find_by_role(&A11yRole::Checkbox);
    assert_eq!(checkboxes.len(), 1);
    assert_eq!(checkboxes[0].state.checked, Some(CheckedState::Mixed));
}
