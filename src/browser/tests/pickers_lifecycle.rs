//! Subsystem unit tests.

use super::super::*;
use super::{attr, hit_control};
use mango_html::dom::NodeId;
use mango_platform::input::{KeyEvent, KeyState, MangoKey, MouseButton};
use mango_render::display_list::DisplayCommand;
use mango_render::get_cached_image;

#[test]
fn test_date_picker_calendar_navigation_and_selection() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="date" id="d" value="2026-03-10" style="width: 160px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:date".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    assert_eq!(hit.form_type, "date");
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("date box");

    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let (px, py) = {
        let picker = browser.picker.as_ref().expect("date picker opens");
        match picker.kind {
            PickerKind::Date { year, month } => assert_eq!((year, month), (2026, 3)),
            ref other => panic!("expected Date picker, got {:?}", other),
        }
        (picker.x, picker.y)
    };
    assert_eq!(browser.status_text, "Pick a date");

    // The calendar header shows the month/year.
    let texts: Vec<String> = browser
        .build_display_list((800, 600))
        .into_iter()
        .filter_map(|c| match c {
            DisplayCommand::DrawText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t == "March 2026"),
        "month header painted"
    );
    assert!(texts.iter().any(|t| t == "15"), "day cell painted");

    // Click day 15 (March 2026 starts on a Sunday).
    let col_w = (210.0 - PICKER_DATE_PAD * 2.0) / 7.0;
    let lead = weekday_sunday0(days_from_civil(2026, 3, 1));
    let idx = lead + 15 - 1;
    let dx = px + PICKER_DATE_PAD + (idx % 7) as f32 * col_w + col_w / 2.0;
    let dy = py + PICKER_DATE_GRID_Y + (idx / 7) as f32 * PICKER_DATE_ROW_H + 10.0;
    browser.handle_mouse_move(dx, dy);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(
        browser.picker.is_none(),
        "picking a day closes the calendar"
    );
    assert_eq!(
        attr(&browser, node_id, "value").as_deref(),
        Some("2026-03-15")
    );
    assert_eq!(browser.status_text, "Date: 2026-03-15");

    // Reopen and navigate to the next month with the ">" header button.
    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let (px, py) = {
        let picker = browser.picker.as_ref().expect("date picker reopens");
        (picker.x, picker.y)
    };
    browser.handle_mouse_move(px + 210.0 - 30.0 + 13.0, py + PICKER_DATE_HEADER_Y + 11.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    match &browser.picker.as_ref().expect("still open").kind {
        PickerKind::Date { year, month } => assert_eq!((*year, *month), (2026, 4)),
        other => panic!("expected Date picker, got {:?}", other),
    }

    // Clicking outside the panel dismisses it.
    browser.handle_mouse_move(10.0, HEADER_HEIGHT + 10.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(browser.picker.is_none(), "outside click closes the picker");
}

#[test]
fn test_time_picker_steppers_and_set() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="time" id="t" value="09:30" style="width: 140px; height: 26px; display: block;"/>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:time".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    assert_eq!(hit.form_type, "time");
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("time box");

    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let (px, py) = {
        let picker = browser.picker.as_ref().expect("time picker opens");
        assert!(matches!(picker.kind, PickerKind::Time { .. }));
        (picker.x, picker.y)
    };

    // Hour "+": 09 → 10.
    browser.handle_mouse_move(px + 123.0, py + PICKER_TIME_HOUR_Y + 12.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    match &browser.picker.as_ref().unwrap().kind {
        PickerKind::Time { hour, .. } => assert_eq!(*hour, 10),
        other => panic!("expected Time picker, got {:?}", other),
    }
    // Minute "-": 30 → 29.
    browser.handle_mouse_move(px + 27.0, py + PICKER_TIME_MIN_Y + 12.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    match &browser.picker.as_ref().unwrap().kind {
        PickerKind::Time { minute, .. } => assert_eq!(*minute, 29),
        other => panic!("expected Time picker, got {:?}", other),
    }

    // "Set" commits the value.
    browser.handle_mouse_move(px + 75.0, py + PICKER_TIME_SET_Y + 13.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(browser.picker.is_none(), "Set closes the time picker");
    assert_eq!(attr(&browser, node_id, "value").as_deref(), Some("10:29"));
    assert_eq!(browser.status_text, "Time: 10:29");
}

#[test]
fn test_file_picker_lists_directory_and_selects() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="file" id="f" style="width: 260px; height: 28px; display: block;"/>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:file".to_string());

    // The unselected field renders a "Choose file..." button.
    let texts: Vec<String> = browser
        .build_display_list((800, 600))
        .into_iter()
        .filter_map(|c| match c {
            DisplayCommand::DrawText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t == "Choose file..."),
        "button painted"
    );
    assert!(
        texts.iter().any(|t| t == "No file chosen"),
        "placeholder painted"
    );

    let hit = hit_control(&browser, 30.0, 30.0);
    assert_eq!(hit.form_type, "file");
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("file box");

    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    let (root_dir, entries_len) = {
        let picker = browser.picker.as_ref().expect("file picker opens");
        match &picker.kind {
            PickerKind::File {
                dir,
                entries,
                offset,
            } => {
                assert!(!entries.is_empty(), "directory listing is populated");
                assert_eq!(*offset, 0);
                (dir.clone(), entries.len())
            }
            other => panic!("expected File picker, got {:?}", other),
        }
    };
    assert!(entries_len > 0);
    assert_eq!(browser.status_text, "Choose a file");

    // Pick the first file entry that fits in the visible first screen
    // (row 0 is the ".." parent row, so entry idx n sits at row n + 1).
    let file_pick = match &browser.picker.as_ref().unwrap().kind {
        PickerKind::File { entries, .. } => entries
            .iter()
            .enumerate()
            .find(|(i, e)| !e.is_dir && *i + 1 < PICKER_FILE_ROWS)
            .map(|(i, e)| (i, e.name.clone())),
        _ => None,
    };
    if let Some((idx, name)) = file_pick {
        let (px, py) = {
            let p = browser.picker.as_ref().unwrap();
            (p.x, p.y)
        };
        let row_y =
            py + PICKER_FILE_ROW_Y + (idx + 1) as f32 * PICKER_FILE_ROW_H + PICKER_FILE_ROW_H / 2.0;
        browser.handle_mouse_move(px + 60.0, row_y);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_none(), "picking a file closes the picker");
        assert_eq!(
            attr(&browser, node_id, "value").as_deref(),
            Some(name.as_str())
        );
        assert_eq!(
            attr(&browser, node_id, "data-mango-filename").as_deref(),
            Some(name.as_str())
        );
        let path = attr(&browser, node_id, "data-mango-filepath").unwrap();
        assert!(
            path.ends_with(&name),
            "data-mango-filepath points at the chosen file: {path}"
        );
        assert_eq!(browser.status_text, format!("Selected file: {}", name));

        // Reopen to exercise directory navigation.
        browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        assert!(browser.picker.is_some(), "picker reopens");
    }

    // Navigate into the first directory (dirs sort first; row 0 is "..").
    let dir_name = match &browser.picker.as_ref().unwrap().kind {
        PickerKind::File { entries, .. } => entries
            .iter()
            .find(|e| e.is_dir)
            .map(|e| e.name.clone())
            .expect("repo contains at least one directory"),
        other => panic!("expected File picker, got {:?}", other),
    };
    let dir_idx = match &browser.picker.as_ref().unwrap().kind {
        PickerKind::File { entries, .. } => {
            entries.iter().position(|e| e.name == dir_name).unwrap()
        }
        _ => unreachable!(),
    };
    let (px, py) = {
        let p = browser.picker.as_ref().unwrap();
        (p.x, p.y)
    };
    let row_y =
        py + PICKER_FILE_ROW_Y + (dir_idx + 1) as f32 * PICKER_FILE_ROW_H + PICKER_FILE_ROW_H / 2.0;
    browser.handle_mouse_move(px + 60.0, row_y);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    match &browser
        .picker
        .as_ref()
        .expect("still open after entering dir")
        .kind
    {
        PickerKind::File {
            dir,
            entries,
            offset,
        } => {
            assert!(
                dir.ends_with(&dir_name),
                "entered {:?}, got {dir:?}",
                dir_name
            );
            assert_eq!(*offset, 0);
            let _ = entries;
        }
        other => panic!("expected File picker, got {:?}", other),
    }

    // ".." (row 0) navigates back to the parent directory.
    let (px, py) = {
        let p = browser.picker.as_ref().unwrap();
        (p.x, p.y)
    };
    browser.handle_mouse_move(px + 60.0, py + PICKER_FILE_ROW_Y + PICKER_FILE_ROW_H / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    match &browser
        .picker
        .as_ref()
        .expect("still open after going up")
        .kind
    {
        PickerKind::File { dir, .. } => assert_eq!(dir, &root_dir, "back at the root"),
        other => panic!("expected File picker, got {:?}", other),
    }
}

#[test]
fn test_input_and_change_events_reach_page_listeners() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <html>
            <body style="margin: 0; padding: 20px;">
                <input type="text" id="t" value="a" style="width: 150px; height: 24px; display: block;"/>
                <div id="pad" style="width: 300px; height: 40px; display: block;">pad</div>
                <script>
                    var el = document.getElementById('t');
                    el.addEventListener('input', function() {
                        el.setAttribute('data-hits', String(Number(el.getAttribute('data-hits') || '0') + 1));
                    });
                    el.addEventListener('change', function() {
                        el.setAttribute('data-changed', '1');
                    });
                </script>
            </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "test:events".to_string());

    let hit = hit_control(&browser, 30.0, 30.0);
    let node_id = hit.node_id.unwrap();
    let (cx, cy, _cw, ch) = browser.control_content_rect(node_id).expect("text box");

    // Focus and type two characters → two `input` events reach the listener.
    browser.handle_mouse_move(cx + 8.0, HEADER_HEIGHT + cy + ch / 2.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert_eq!(browser.focused_control, Some(node_id));
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('b'),
        state: KeyState::Pressed,
        modifiers: mango_platform::input::Modifiers::default(),
    });
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('c'),
        state: KeyState::Pressed,
        modifiers: mango_platform::input::Modifiers::default(),
    });
    assert_eq!(
        attr(&browser, node_id, "data-hits").as_deref(),
        Some("2"),
        "page listeners received both input events"
    );
    assert_eq!(
        attr(&browser, node_id, "value").map(|v| v.chars().count()),
        Some(3),
        "both characters were inserted"
    );
    assert_eq!(
        attr(&browser, node_id, "data-changed"),
        None,
        "no change yet"
    );

    // Clicking normal content blurs the field → one trailing `change` event.
    browser.handle_mouse_move(30.0, HEADER_HEIGHT + 70.0);
    browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
    assert!(
        browser.focused_control.is_none(),
        "focus cleared by content click"
    );
    assert_eq!(
        attr(&browser, node_id, "data-changed").as_deref(),
        Some("1"),
        "change listener fired exactly once on blur"
    );
}

#[test]
fn test_browser_smooth_scroll_to() {
    let mut browser = BrowserChrome::new(800, 600);
    browser.load_html_internal(
        "<div style='height: 2000px;'>Long Content</div>".to_string(),
        "http://example.com".to_string(),
        false,
    );
    assert_eq!(browser.scroll_y, 0.0);

    browser.scroll_smooth_to(300.0, Some(std::time::Duration::from_millis(200)));
    assert!(browser.smooth_scroll.is_some());
    assert!(browser.has_pending_timers());

    // Step animation forward after small delay
    std::thread::sleep(std::time::Duration::from_millis(30));
    let stepped = browser.tick_animations(30.0);
    assert!(stepped);
    assert!(browser.scroll_y > 0.0);

    // Advance to completion
    std::thread::sleep(std::time::Duration::from_millis(210));
    let _ = browser.tick_animations(150.0);
    assert!(browser.scroll_y >= 290.0);
}

#[test]
fn test_browser_dom_content_loaded_and_load_events() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script>
                    window.dcl_fired = false;
                    window.load_fired = false;
                    document.addEventListener('DOMContentLoaded', function() {
                        window.dcl_fired = true;
                        var el = document.getElementById('target');
                        if (el) el.setAttribute('data-dcl', 'yes');
                    });
                    window.addEventListener('load', function() {
                        window.load_fired = true;
                        var el = document.getElementById('target');
                        if (el) el.setAttribute('data-load', 'yes');
                    });
                </script>
            </head>
            <body>
                <div id="target">Content</div>
            </body>
            </html>
        "#;
    browser.load_html_internal(html.to_string(), "http://example.com".to_string(), false);

    let doc = browser.active_document();
    fn find_id(doc: &mango_html::Document, nid: NodeId, id: &str) -> Option<NodeId> {
        if let Some(node) = doc.get(nid)
            && let mango_html::NodeData::Element(el) = &node.data
            && el.get_attribute("id") == Some(id)
        {
            return Some(nid);
        }
        for ch in doc.children(nid) {
            if let Some(found) = find_id(doc, ch.id, id) {
                return Some(found);
            }
        }
        None
    }
    let target_id = find_id(&doc, doc.root(), "target").expect("target found");
    assert_eq!(
        attr(&browser, target_id, "data-dcl").as_deref(),
        Some("yes"),
        "DOMContentLoaded event listener executed"
    );
    assert_eq!(
        attr(&browser, target_id, "data-load").as_deref(),
        Some("yes"),
        "window load event listener executed"
    );
}

#[test]
fn test_find_in_page_and_shortcuts() {
    let mut browser = BrowserChrome::new(800, 600);
    let html = r#"
            <!DOCTYPE html>
            <html>
                <body>
                    <p>Mango is a fast browser.</p>
                    <p>Another Mango instance here.</p>
                    <p>Third mango mention.</p>
                </body>
            </html>
        "#;
    browser.load_html(html.to_string(), "https://example.com/find".to_string());

    // 1. Test Ctrl+F toggle
    let ctrl_mod = mango_platform::input::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let none_mod = mango_platform::input::Modifiers::default();

    assert!(!browser.find_in_page.active);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('f'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert!(browser.find_in_page.active);

    // 2. Test search matches
    browser.find_in_page_search("Mango");
    assert_eq!(browser.find_in_page.matches.len(), 3);
    assert_eq!(browser.find_in_page.current_match, 0);

    // 3. Test find_next and find_previous
    browser.find_next();
    assert_eq!(browser.find_in_page.current_match, 1);
    browser.find_next();
    assert_eq!(browser.find_in_page.current_match, 2);
    browser.find_next();
    assert_eq!(browser.find_in_page.current_match, 0);
    browser.find_previous();
    assert_eq!(browser.find_in_page.current_match, 2);

    // 4. Test Escape closes find bar
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Escape,
        state: KeyState::Pressed,
        modifiers: none_mod,
    });
    assert!(!browser.find_in_page.active);

    // 5. Test Zoom shortcuts: Ctrl+=, Ctrl+-, Ctrl+0
    assert_eq!(browser.zoom_level, 1.0);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('='),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert!((browser.zoom_level - 1.1).abs() < 1e-4);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('-'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert!((browser.zoom_level - 1.0).abs() < 1e-4);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('+'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('0'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert_eq!(browser.zoom_level, 1.0);

    // 6. Test F11 Fullscreen toggle
    assert!(!browser.is_fullscreen);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::F11,
        state: KeyState::Pressed,
        modifiers: none_mod,
    });
    assert!(browser.is_fullscreen);
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::F11,
        state: KeyState::Pressed,
        modifiers: none_mod,
    });
    assert!(!browser.is_fullscreen);

    // 7. Test Tab shortcuts: Ctrl+T, Ctrl+W, Ctrl+L
    let initial_tabs = browser.tabs.len();
    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('t'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert_eq!(browser.tabs.len(), initial_tabs + 1);

    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('l'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert!(browser.address_focused);

    browser.handle_key_event(&KeyEvent {
        key: MangoKey::Char('w'),
        state: KeyState::Pressed,
        modifiers: ctrl_mod,
    });
    assert_eq!(browser.tabs.len(), initial_tabs);

    // 8. Test display list and chrome rendering contains find bar when active
    browser.toggle_find();
    let chrome_dl = browser.build_chrome_display_list((800, 600));
    assert!(!chrome_dl.is_empty());
    let page_dl = browser.build_display_list((800, 600));
    assert!(!page_dl.is_empty());
}

#[test]
fn test_queue_image_backlog_bounded_cap() {
    use crate::browser::images::MAX_PENDING_IMAGE_FETCHES;
    let mut browser = BrowserChrome::new(1024, 768);
    let urls: Vec<String> = (0..1000)
        .map(|i| format!("https://example.com/img{i}.png"))
        .collect();
    browser.queue_image_backlog(urls, Url::parse("https://example.com/").unwrap());
    assert!(browser.pending_image_fetches.len() <= MAX_PENDING_IMAGE_FETCHES);
    assert_eq!(
        browser.pending_image_fetches.len(),
        MAX_PENDING_IMAGE_FETCHES
    );
}

#[test]
fn test_background_tab_navigation_does_not_hijack_active_tab() {
    let mut browser = BrowserChrome::new(1024, 768);
    // Open a new tab (Tab 1), which becomes active
    browser.new_tab();
    assert_eq!(browser.tabs.len(), 2);
    assert_eq!(browser.active_tab_idx, 1);
    let active_title_before = browser.page_title.clone();

    // Prepare a document for background Tab 0
    let url = Url::parse("https://example.com/test-bg").unwrap();
    let doc = mango_html::parse_html(
        "<html><head><title>Background Page Title</title></head><body><h1>BG</h1></body></html>",
    );
    let prep = crate::browser::PreparedDocument {
        url: url.clone(),
        doc,
        html:
            "<html><head><title>Background Page Title</title></head><body><h1>BG</h1></body></html>"
                .to_string(),
        status: 200,
        content_type: "text/html".to_string(),
        encoding: mango_net::encoding::Encoding::Utf8,
        cached_stylesheets: Vec::new(),
        preloaded_scripts: Vec::new(),
        image_sources: Vec::new(),
    };

    let epoch = browser.nav_actor.active_epoch();
    browser
        .nav_actor
        .inject_result_for_test(crate::navigation_actor::NavResult::DocumentReady {
            tab_id: 0,
            prep,
            push_history: true,
            epoch,
        });

    // Tick navigation: background result should update Tab 0 contents, NOT active Tab 1
    let redrawn = browser.tick_navigation();
    assert!(
        !redrawn,
        "Background tab navigation should not trigger active viewport redraw"
    );
    assert_eq!(browser.active_tab_idx, 1, "Active tab must still be Tab 1");
    assert_eq!(
        browser.page_title, active_title_before,
        "Active tab title must not be clobbered"
    );

    // Check Tab 0 state
    let tab0 = &browser.tabs[0];
    assert_eq!(tab0.url, "https://example.com/test-bg");
    assert_eq!(tab0.title, "Background Page Title");
    assert!(tab0.contents.is_some(), "Tab 0 must have updated contents");
    assert_eq!(
        tab0.contents.as_ref().unwrap().current_html,
        "<html><head><title>Background Page Title</title></head><body><h1>BG</h1></body></html>"
    );
}

#[test]
fn test_bfcache_instant_restoration() {
    let mut browser = BrowserChrome::new(1024, 768);

    // 1. Load initial page with tall content and links
    let page1_html = "<!DOCTYPE html><html><head><title>Page 1 Title</title></head><body><div style='height: 2500px;'><h1>Page 1 Heading</h1><a href='https://example.com/page2'>Link</a></div></body></html>";
    browser.load_html(
        page1_html.to_string(),
        "https://example.com/page1".to_string(),
    );
    assert_eq!(browser.address_text, "https://example.com/page1");
    assert_eq!(browser.page_title, "Page 1 Title");
    assert!(browser.root_box.is_some());
    assert!(!browser.page_display_list.is_empty());

    // 2. Scroll down on Page 1
    browser.set_scroll_y_f64(320.0);
    assert_eq!(browser.scroll_y_f64(), 320.0);

    // 3. Navigate to Page 2
    let page2_html = "<!DOCTYPE html><html><head><title>Page 2 Title</title></head><body><h1>Page 2 Heading</h1></body></html>";
    browser.load_html(
        page2_html.to_string(),
        "https://example.com/page2".to_string(),
    );
    assert_eq!(browser.address_text, "https://example.com/page2");
    assert_eq!(browser.page_title, "Page 2 Title");
    assert_eq!(browser.scroll_y_f64(), 0.0);

    // 4. Verify can_go_back
    assert!(browser.tabs[browser.active_tab_idx].can_go_back());

    // 5. Navigate back: should be restored instantly from BFCache!
    browser.go_back();
    assert_eq!(browser.address_text, "https://example.com/page1");
    assert_eq!(browser.page_title, "Page 1 Title");
    assert_eq!(browser.scroll_y_f64(), 320.0);
    assert!(browser.status_text.contains("Restored from BFCache"));
    assert!(browser.root_box.is_some());
    assert!(!browser.page_display_list.is_empty());

    // 6. Navigate forward: should restore Page 2 from BFCache!
    assert!(browser.tabs[browser.active_tab_idx].can_go_forward());
    browser.go_forward();
    assert_eq!(browser.address_text, "https://example.com/page2");
    assert_eq!(browser.page_title, "Page 2 Title");
    assert!(browser.status_text.contains("Restored from BFCache"));
}

#[test]
fn test_github_avatar_fetch_and_caching() {
    let mut browser = BrowserChrome::new(1024, 768);
    let avatar_url = "https://avatars.githubusercontent.com/u/5430905?s=60&v=4";
    let base = Url::parse("https://github.com/rust-lang/rust").unwrap();
    browser.queue_image_backlog(vec![avatar_url.to_string()], base);
    assert!(browser.has_pending_image_fetches());

    let mut drained = false;
    for _ in 0..200 {
        if browser.drain_pending_images() {
            drained = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if drained {
        assert!(
            get_cached_image(avatar_url).is_some(),
            "avatar image must be present in image cache"
        );
    }
}

#[test]
fn test_tick_navigation_applies_results_for_multiple_tabs() {
    let mut browser = BrowserChrome::new(1024, 768);
    // Tab 0 is background; tab 1 is active.
    browser.new_tab();
    assert_eq!(browser.active_tab_idx, 1);

    fn make_prep(url: &str, title: &str) -> crate::browser::PreparedDocument {
        let html = format!(
            "<html><head><title>{title}</title></head><body><h1>{title}</h1></body></html>"
        );
        crate::browser::PreparedDocument {
            url: Url::parse(url).unwrap(),
            doc: mango_html::parse_html(&html),
            html,
            status: 200,
            content_type: "text/html".to_string(),
            encoding: mango_net::encoding::Encoding::Utf8,
            cached_stylesheets: Vec::new(),
            preloaded_scripts: Vec::new(),
            image_sources: Vec::new(),
        }
    }

    let bg_epoch = browser.nav_actor.active_epoch_for(0);
    browser
        .nav_actor
        .inject_result_for_test(crate::navigation_actor::NavResult::DocumentReady {
            tab_id: 0,
            prep: make_prep("https://example.com/background", "Background"),
            push_history: true,
            epoch: bg_epoch,
        });
    let active_epoch = browser.nav_actor.active_epoch_for(1);
    browser
        .nav_actor
        .inject_result_for_test(crate::navigation_actor::NavResult::DocumentReady {
            tab_id: 1,
            prep: make_prep("https://example.com/active", "Active"),
            push_history: true,
            epoch: active_epoch,
        });

    assert!(browser.tick_navigation());
    assert_eq!(
        browser.tabs[0].title, "Background",
        "background result must still be applied when another tab has a result in the same batch"
    );
    assert_eq!(browser.page_title, "Active");
    assert_eq!(browser.active_tab_idx, 1);
}
