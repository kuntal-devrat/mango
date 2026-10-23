//! Headless viewport renderer — the visual regression / parity tool.
//!
//! Renders a URL into a PNG at a fixed viewport size without opening a window.
//! This is the reference renderer used by `tools/visual_diff.py` to compare
//! Mango against Chromium pixel-for-pixel.
//!
//! Usage:
//!   headless <url|file> [output.png] [width] [height] [scroll_y] [--page] [--blur]
//!   headless <url|file> --probe <css-selector> [width] [height]
//!
//! Flags:
//!   --page   render only the page content viewport (no browser chrome), which is
//!            what a Chromium `--screenshot` produces, so the two are comparable.
//!   --blur   clear form focus before painting (matches an unfocused Chromium tab).
//!   --probe  print the geometry and computed style of every element matching a CSS
//!            selector instead of rasterizing. This is the counterpart to Chromium's
//!            `getBoundingClientRect()` + `getComputedStyle()` and is how layout
//!            divergences are diagnosed.

use std::env;
use std::path::Path;

use mango_browser::browser::BrowserChrome;
use mango_browser::chrome_ui::{HEADER_HEIGHT, STATUS_BAR_HEIGHT};
use mango_html::dom::{Document, NodeId};
use mango_layout::BoxType;
use mango_layout::box_tree::LayoutBox;
use mango_platform::input::{KeyState, MouseButton};
use mango_render::painter::paint;

/// Height of the browser chrome measured from the top of the window to the first
/// row of page content: one pixel of separator plus the two chrome bars.
const CONTENT_TOP: u32 = (HEADER_HEIGHT as u32) + 1;

fn main() {
    unsafe {
        if let Ok(lang) = std::env::var("LANG") {
            if !lang.to_lowercase().contains("utf-8") && !lang.to_lowercase().contains("utf8") {
                let normalized = format!("{}.UTF-8", lang.split('.').next().unwrap_or("en_US"));
                std::env::set_var("LANG", &normalized);
                std::env::set_var("LC_ALL", &normalized);
            }
        } else {
            std::env::set_var("LANG", "C.UTF-8");
            std::env::set_var("LC_ALL", "C.UTF-8");
        }
        if std::env::var("LC_CTYPE").is_err() {
            std::env::set_var("LC_CTYPE", "C.UTF-8");
        }
    }

    let _ = env_logger::builder().is_test(false).try_init();

    let args: Vec<String> = env::args().collect();
    let mut positional: Vec<&str> = Vec::new();
    let mut page_only = false;
    let mut full_page = false;
    let mut blur = false;
    let mut probe_selector = None;
    let mut base_url_override: Option<String> = None;
    let mut hover_pos = None;
    let mut click_pos = None;
    let mut virtual_time_budget_ms: u64 = 100;
    let mut wait_sec: u64 = 0;

    let mut remote_debugging_port = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            arg if arg.starts_with("--remote-debugging-port=") => {
                if let Ok(port) = arg["--remote-debugging-port=".len()..].parse::<u16>() {
                    remote_debugging_port = Some(port);
                }
                i += 1;
            }
            "--remote-debugging-port" => {
                if i + 1 < args.len()
                    && let Ok(port) = args[i + 1].parse::<u16>()
                {
                    remote_debugging_port = Some(port);
                    i += 1;
                }
                i += 1;
            }
            "--full-page" | "--full" => {
                full_page = true;
                page_only = true;
                i += 1;
            }
            "--page" => {
                page_only = true;
                i += 1;
            }
            "--blur" | "--unfocused" => {
                blur = true;
                i += 1;
            }
            "--base" => {
                if i + 1 < args.len() {
                    base_url_override = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--probe" => {
                if i + 1 < args.len() {
                    probe_selector = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--hover" => {
                if i + 1 < args.len() {
                    let s = &args[i + 1];
                    let mut parts = s.split(',');
                    if let (Some(x), Some(y)) = (
                        parts.next().and_then(|v| v.parse().ok()),
                        parts.next().and_then(|v| v.parse().ok()),
                    ) {
                        hover_pos = Some((x, y));
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--click" => {
                if i + 1 < args.len() {
                    let s = &args[i + 1];
                    let mut parts = s.split(',');
                    if let (Some(x), Some(y)) = (
                        parts.next().and_then(|v| v.parse().ok()),
                        parts.next().and_then(|v| v.parse().ok()),
                    ) {
                        click_pos = Some((x, y));
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--virtual-time-budget" | "--timeout" => {
                if i + 1 < args.len() {
                    if let Ok(ms) = args[i + 1].parse::<u64>() {
                        virtual_time_budget_ms = ms;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--wait" | "--wait-sec" | "--wait-seconds" => {
                if i + 1 < args.len() {
                    if let Ok(sec) = args[i + 1].parse::<u64>() {
                        wait_sec = sec;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            flag if flag.starts_with("--") => {
                i += 1;
            }
            pos => {
                positional.push(pos);
                i += 1;
            }
        }
    }

    let _cdp_server = if let Some(port) = remote_debugging_port {
        match mango_browser::cdp::CdpServer::start(port) {
            Ok(server) => {
                eprintln!(
                    "DevTools listening on ws://127.0.0.1:{}/devtools/browser",
                    server.port()
                );
                Some(server)
            }
            Err(e) => {
                eprintln!("Failed to start CDP server on port {port}: {e}");
                None
            }
        }
    } else {
        None
    };

    if positional.is_empty() && remote_debugging_port.is_some() {
        eprintln!("Mango Headless running with DevTools active. Press Ctrl+C to stop.");
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    let url = positional
        .first()
        .copied()
        .unwrap_or("https://en.wikipedia.org/wiki/Main_Page");
    let (output_path, width, height, scroll_y) = if probe_selector.is_some() {
        let width: u32 = positional
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1280);
        let height: u32 = positional
            .get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);
        let scroll_y: f32 = positional
            .get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        ("probe.png", width, height, scroll_y)
    } else {
        let output_path = positional.get(1).copied().unwrap_or("wikipedia_render.png");
        let width: u32 = positional
            .get(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1280);
        let height: u32 = positional
            .get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);
        let scroll_y: f32 = positional
            .get(4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        (output_path, width, height, scroll_y)
    };

    // In page-only mode we render at window height = viewport height + chrome, then
    // crop the chrome away, so the page sees exactly `height` CSS pixels of viewport.
    let window_height = if page_only {
        height + CONTENT_TOP + (STATUS_BAR_HEIGHT as u32)
    } else {
        height
    };

    println!("🥭 Mango Headless Renderer");
    println!("Navigating to: {}", url);
    println!(
        "Target resolution: {}x{}{}",
        width,
        height,
        if page_only { " (page viewport)" } else { "" }
    );
    if scroll_y > 0.0 {
        println!("Scroll offset: {}px", scroll_y);
    }

    let mut browser = BrowserChrome::new(width, window_height);
    let file_path = if let Some(stripped) = url.strip_prefix("file://") {
        stripped
    } else {
        url
    };
    let page_url = base_url_override.unwrap_or_else(|| {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            "https://www.youtube.com/".to_string()
        }
    });
    let load_local = |b: &mut BrowserChrome, content: String, target_url: String| {
        b.load_html(content, target_url);
    };
    if let Ok(content) = std::fs::read_to_string(file_path) {
        load_local(&mut browser, content, page_url);
    } else {
        browser.navigate(url);
        browser.wait_for_navigation();
    }
    println!("Browser status: {}", browser.status_text());

    // Virtual time budget / real-time wait: drive animations and JS timers
    let tick_interval_ms = 16.0;
    let mut loaded_any_images = false;
    if wait_sec > 0 {
        let wait_dur = std::time::Duration::from_secs(wait_sec);
        let start = std::time::Instant::now();
        println!(
            "Waiting {}s for rendering, dynamic JS, and network settling...",
            wait_sec
        );
        while start.elapsed() < wait_dur {
            let _ = browser.tick_navigation();
            let _ = browser.tick_animations(tick_interval_ms);
            let _ = browser.tick_js();
            let _ = browser.flush_pending_layout();
            if browser.has_pending_image_fetches() && browser.drain_pending_images() {
                loaded_any_images = true;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    } else {
        let total_ticks =
            ((virtual_time_budget_ms as f32 / tick_interval_ms).ceil() as usize).max(5);
        for _ in 0..total_ticks {
            let _ = browser.tick_animations(tick_interval_ms);
            let _ = browser.tick_js();
            let _ = browser.flush_pending_layout();
            if browser.has_pending_image_fetches() && browser.drain_pending_images() {
                loaded_any_images = true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    // Drain pending deferred/async scripts so dynamic content is fully executed
    let mut drained_scripts = 0;
    while browser.has_pending_scripts() && drained_scripts < 64 {
        drained_scripts += 1;
        browser.tick_js();
        browser.flush_pending_layout();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Allow promises, microtasks, and framework mounts (React/Vue/Svelte) to settle
    for _ in 0..10 {
        let _ = browser.tick_animations(tick_interval_ms);
        let _ = browser.tick_js();
        let _ = browser.flush_pending_layout();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Drain pending images so remote and data URI images are loaded and decoded
    let mut drained_batches = 0;
    while browser.has_pending_image_fetches() && drained_batches < 300 {
        drained_batches += 1;
        if browser.drain_pending_images() {
            loaded_any_images = true;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
    if loaded_any_images {
        browser.relayout();
    }

    if !browser.devtools.console_entries.is_empty() {
        println!(
            "Console messages ({}):",
            browser.devtools.console_entries.len()
        );
        for entry in browser.devtools.console_entries.iter().take(20) {
            println!("  [{:?}] {}", entry.level, entry.message);
        }
    }

    if scroll_y > 0.0 {
        browser.set_scroll_y(scroll_y);
    }

    if blur {
        browser.clear_form_focus();
    }

    if let Some((hx, hy)) = hover_pos {
        let (actual_hx, actual_hy) = if page_only {
            (hx, hy + CONTENT_TOP as f32)
        } else {
            (hx, hy)
        };
        browser.handle_mouse_move(actual_hx, actual_hy);
    }

    if let Some((cx, cy)) = click_pos {
        let (actual_cx, actual_cy) = if page_only {
            (cx, cy + CONTENT_TOP as f32)
        } else {
            (cx, cy)
        };
        browser.handle_mouse_move(actual_cx, actual_cy);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Pressed);
        browser.handle_mouse_click(MouseButton::Left, KeyState::Released);
        for _ in 0..10 {
            let _ = browser.tick_animations(tick_interval_ms);
            let _ = browser.tick_js();
            let _ = browser.flush_pending_layout();
        }
        browser.relayout();
    }

    if let Some(selector) = probe_selector {
        probe(&browser, &selector);
        return;
    }

    let (paint_width, paint_height, dl) = if full_page {
        let doc_height = (browser.scrollable_height().ceil() as u32).max(height);
        println!("Full document height: {}px", doc_height);
        let mut full_dl = mango_render::display_list::DisplayList::new();
        full_dl.push(mango_render::DisplayCommand::FillRect {
            rect: mango_core::Rect::new(0.0, 0.0, width as f32, doc_height as f32),
            color: browser.canvas_bg(),
        });
        if let Some(root) = browser.root_box() {
            let page_dl = mango_layout::display_list::build_display_list(root);
            for cmd in page_dl {
                full_dl.push(cmd);
            }
        }
        (width, doc_height, full_dl)
    } else {
        (
            width,
            window_height,
            browser.build_display_list((width, window_height)),
        )
    };
    println!("Generated {} display list commands", dl.len());
    let mut bold_count = 0;
    let mut reg_count = 0;
    for cmd in dl.iter() {
        if let mango_render::DisplayCommand::DrawText {
            text,
            x,
            y,
            font_size,
            weight,
            color,
            ..
        } = cmd
        {
            if *y < 300.0 {
                println!(
                    "DrawText: {:?} at pos=({:.1}, {:.1}) size={:.1} weight={:?} color={:?}",
                    text, x, y, font_size, weight, color
                );
            }
            if *weight == mango_render::FontWeight::Bold {
                bold_count += 1;
            } else {
                reg_count += 1;
            }
        }
    }
    println!("DrawText stats: bold={}, regular={}", bold_count, reg_count);

    let mut buffer = vec![0u32; (paint_width * paint_height) as usize];
    buffer.fill(browser.canvas_bg().to_rgb_u32());
    paint(&dl, &mut buffer, paint_width, paint_height);

    let (crop_top, crop_height) = if full_page {
        (0, paint_height)
    } else if page_only {
        (CONTENT_TOP, height)
    } else {
        (0, window_height)
    };

    let mut img = image::RgbImage::new(paint_width, crop_height);
    for y in 0..crop_height {
        for x in 0..paint_width {
            let px = buffer[((y + crop_top) * paint_width + x) as usize];
            img.put_pixel(
                x,
                y,
                image::Rgb([
                    ((px >> 16) & 0xFF) as u8,
                    ((px >> 8) & 0xFF) as u8,
                    (px & 0xFF) as u8,
                ]),
            );
        }
    }

    if let Some(parent) = Path::new(output_path).parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(err) = img.save(output_path) {
        img.save_with_format(output_path, image::ImageFormat::Png)
            .unwrap_or_else(|_| {
                panic!("failed to save rendered image to {}: {}", output_path, err)
            });
    }
    println!("Successfully captured screenshot to: {}", output_path);
}

/// Prints layout geometry and key computed styles for every element matching `selector`.
///
/// Output is one line per match, in document order, with page-relative CSS pixel
/// coordinates and the text runs each element laid out. Compare directly against
/// Chromium's `getBoundingClientRect()` / `getComputedStyle()` output.
fn probe(browser: &BrowserChrome, selector: &str) {
    let Some(root) = browser.root_box() else {
        println!("no layout tree (page not loaded)");
        return;
    };

    let Some(selector_list) = mango_css::parser::parse_selectors(selector) else {
        println!("could not parse selector: {selector}");
        return;
    };

    let doc = browser.active_document();
    let mut matches: Vec<NodeId> = Vec::new();
    collect_matches(&doc, doc.root(), &selector_list, &mut matches);
    if let Some(root_node) = doc.find_element_by_id(doc.root(), "root") {
        let serialized = doc.serialize_html(root_node);
        println!("ROOT HTML (len={}): {}", serialized.len(), serialized);
    }
    println!("selector '{selector}' matched {} element(s)", matches.len());
    let mut script_dump = String::new();
    let mut script_matches = Vec::new();
    if let Some(script_sel) = mango_css::parser::parse_selectors("script") {
        collect_matches(&doc, doc.root(), &script_sel, &mut script_matches);
        for (i, nid) in script_matches.iter().enumerate() {
            if let Some(node) = doc.get(*nid)
                && let mango_html::dom::NodeData::Element(ref el) = node.data
            {
                script_dump.push_str(&format!(
                    "\n=== SCRIPT {} attrs={:?} ===\n",
                    i + 1,
                    el.attributes
                ));
                script_dump.push_str(&doc.text_content(*nid));
                script_dump.push('\n');
            }
        }
        let _ = std::fs::write("/tmp/claude_scripts.txt", script_dump);
        println!(
            "Dumped {} scripts to /tmp/claude_scripts.txt",
            script_matches.len()
        );
    }

    let mut index = 0usize;
    walk_boxes(root, 0, &matches, false, &mut |depth, b, is_match| {
        if !is_match {
            return;
        }
        index += 1;
        let border = b.dimensions.border_box();
        let content = b.dimensions.content;
        let indent = "  ".repeat(depth);
        let tag = b.tag_name.as_deref().unwrap_or(match &b.box_type {
            BoxType::TextNode(_) => "#text",
            BoxType::AnonymousBlock => "#anon-block",
            BoxType::InlineNode => "#inline",
            _ => "?",
        });
        let style = b.style.as_ref();
        println!(
            "[{index}] {indent}<{tag}> border=({:.1}, {:.1}, {:.1}x{:.1}) content=({:.1}, {:.1}, {:.1}x{:.1}) pos={:?} display={:?} h={:?} max_h={:?} w={:?} max_w={:?} overflow={:?} float={:?} clear={:?}",
            border.x(),
            border.y(),
            border.width(),
            border.height(),
            content.x(),
            content.y(),
            content.width(),
            content.height(),
            style.map(|s| s.position),
            style.map(|s| s.display),
            style.map(|s| &s.height),
            style.map(|s| &s.max_height),
            style.map(|s| &s.width),
            style.map(|s| &s.max_width),
            style.map(|s| (s.overflow_x, s.overflow_y)),
            style.map(|s| s.float),
            style.map(|s| s.clear),
        );
        if let Some(s) = style {
            println!(
                "      flex_dir: {:?}, box_sizing: {:?}, flex: ({}, {}, {:?}), align_items: {:?}, align_self: {:?}",
                s.flex_direction,
                s.box_sizing,
                s.flex_grow,
                s.flex_shrink,
                s.flex_basis,
                s.align_items,
                s.align_self
            );
            println!(
                "      grid_cols: {:?}, grid_col: ({:?} / {:?})",
                s.grid_template_columns, s.grid_column_start, s.grid_column_end
            );
        }
        println!(
            "      box_type: {:?}, appearance: {:?}, attrs: {:?}",
            b.box_type,
            style.map(|s| s.appearance),
            b.attributes
        );
        if let Some(s) = style {
            println!(
                "      color: {:?}, custom_props_count: {}, bg: img={:?} pos={:?} size={:?} layers={:?}, tf: {:?}, borders: (w_r={}, s_r={:?}, c_r={:?})",
                s.color,
                s.custom_properties.len(),
                s.background_image,
                s.background_position,
                s.background_size,
                s.background_layers,
                s.transform,
                s.border_right_width,
                s.border_right_style,
                s.border_right_color
            );
        }
        for child in &b.children {
            if let BoxType::TextNode(text) = &child.box_type
                && !text.trim().is_empty()
            {
                let run = child.dimensions.content;
                let font_size = child.style.as_ref().map(|s| s.font_size).unwrap_or(0.0);
                let family = child
                    .style
                    .as_ref()
                    .map(|s| mango_render::FontFamily::from_css_name(&s.font_family))
                    .unwrap_or(mango_render::FontFamily::SansSerif);
                let (ascent, descent, gap) = mango_render::font::font_manager().font_metrics(
                    family,
                    mango_render::FontWeight::Regular,
                    font_size,
                );
                let ls = child.style.as_ref().map(|s| s.letter_spacing);
                let ws = child.style.as_ref().map(|s| s.word_spacing);
                let fw = child.style.as_ref().map(|s| s.font_weight);
                let ff = child.style.as_ref().map(|s| s.font_family.clone());
                println!(
                    "      text {:?} baseline_y={:.1} x={:.1} w={:.1} ascent={:.1} descent={:.1} gap={:.1} size={:.1} fam={:?} fw={:?} ls={:?} ws={:?} raw_fam={:?}",
                    truncate(text, 40),
                    run.y(),
                    run.x(),
                    run.width(),
                    ascent,
                    descent,
                    gap,
                    font_size,
                    family,
                    fw,
                    ls,
                    ws,
                    ff,
                );
            }
        }
    });
}

fn collect_matches(
    doc: &Document,
    node_id: NodeId,
    selector: &mango_css::selectors::SelectorList,
    out: &mut Vec<NodeId>,
) {
    if selector.matches(node_id, doc) {
        out.push(node_id);
    }
    for child in doc.children(node_id) {
        collect_matches(doc, child.id, selector, out);
    }
    if let Some(shadow) = doc.get_shadow_root(node_id) {
        collect_matches(doc, shadow, selector, out);
    }
}

fn walk_boxes(
    node: &LayoutBox,
    depth: usize,
    matches: &[NodeId],
    is_inside: bool,
    visit: &mut impl FnMut(usize, &LayoutBox, bool),
) {
    let direct_match = node
        .node_id
        .map(|id| matches.contains(&id))
        .unwrap_or(false);
    let active = direct_match || is_inside;
    visit(depth, node, active);
    for child in &node.children {
        walk_boxes(child, depth + 1, matches, active, visit);
    }
}

fn truncate(text: &str, max: usize) -> String {
    let cleaned = text.replace('\n', "\\n");
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let cut: String = cleaned.chars().take(max).collect();
        format!("{cut}…")
    }
}
