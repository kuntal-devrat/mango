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
use mango_render::painter::paint;

/// Height of the browser chrome measured from the top of the window to the first
/// row of page content: one pixel of separator plus the two chrome bars.
const CONTENT_TOP: u32 = (HEADER_HEIGHT as u32) + 1;

fn main() {
    let _ = env_logger::builder().is_test(false).try_init();

    let args: Vec<String> = env::args().collect();
    let mut positional: Vec<&str> = Vec::new();
    let mut page_only = false;
    let mut blur = false;
    let mut probe_selector = None;
    let mut hover_pos = None;
    let mut virtual_time_budget_ms: u64 = 100;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--page" => {
                page_only = true;
                i += 1;
            }
            "--blur" | "--unfocused" => {
                blur = true;
                i += 1;
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
            flag if flag.starts_with("--") => {
                i += 1;
            }
            pos => {
                positional.push(pos);
                i += 1;
            }
        }
    }

    let url = positional
        .first()
        .copied()
        .unwrap_or("https://en.wikipedia.org/wiki/Main_Page");
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
    let trimmed_file = url
        .trim_start_matches("file:///")
        .trim_start_matches("file://");
    if let Ok(content) = std::fs::read_to_string(trimmed_file) {
        browser.load_html(content, url.to_string());
    } else if let Ok(content) = std::fs::read_to_string(url) {
        browser.load_html(content, url.to_string());
    } else {
        browser.navigate(url);
    }

    // Virtual time budget: drive animations and JS timers
    let tick_interval_ms = 16.0;
    let total_ticks = ((virtual_time_budget_ms as f32 / tick_interval_ms).ceil() as usize).max(5);
    for _ in 0..total_ticks {
        let _ = browser.tick_animations(tick_interval_ms);
        let _ = browser.tick_js();
        if browser.has_pending_image_fetches() {
            browser.drain_pending_images();
        }
    }

    // Drain pending images so remote and data URI images are loaded and decoded
    let mut drained_batches = 0;
    while browser.has_pending_image_fetches() && drained_batches < 40 {
        drained_batches += 1;
        browser.drain_pending_images();
    }

    if scroll_y > 0.0 {
        browser.set_scroll_y(scroll_y);
    }

    if blur {
        browser.clear_form_focus();
    }

    if let Some((hx, hy)) = hover_pos {
        browser.handle_mouse_move(hx, hy);
    }

    if let Some(selector) = probe_selector {
        probe(&browser, &selector);
        return;
    }

    let dl = browser.build_display_list((width, window_height));
    println!("Generated {} display list commands", dl.len());
    let mut bold_count = 0;
    let mut reg_count = 0;
    for cmd in dl.iter() {
        if let mango_render::DisplayCommand::DrawText {
            text,
            weight,
            family,
            font_size,
            ..
        } = cmd
        {
            if *weight == mango_render::FontWeight::Bold {
                bold_count += 1;
                if bold_count <= 10 {
                    let fm = mango_render::font::font_manager();
                    let f_bold =
                        fm.select_font(*family, mango_render::FontWeight::Bold) as *const _;
                    let f_reg =
                        fm.select_font(*family, mango_render::FontWeight::Regular) as *const _;
                    let same_font = f_bold == f_reg;
                    let desc = fm.debug_family_name(*family);
                    println!(
                        "BOLD TEXT [family={}, size={}, same_as_reg={}]: {:?}",
                        desc, font_size, same_font, text
                    );
                }
            } else {
                reg_count += 1;
            }
        }
    }
    println!("DrawText stats: bold={}, regular={}", bold_count, reg_count);

    let mut buffer = vec![0u32; (width * window_height) as usize];
    buffer.fill(mango_core::Color::rgb(255, 255, 255).to_rgb_u32());
    paint(&dl, &mut buffer, width, window_height);

    let (crop_top, crop_height) = if page_only {
        (CONTENT_TOP, height)
    } else {
        (0, window_height)
    };

    let mut img = image::RgbImage::new(width, crop_height);
    for y in 0..crop_height {
        for x in 0..width {
            let px = buffer[((y + crop_top) * width + x) as usize];
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
    println!("selector '{selector}' matched {} element(s)", matches.len());

    let mut index = 0usize;
    walk_boxes(root, 0, &matches, &mut |depth, b, is_match| {
        if !is_match {
            return;
        }
        index += 1;
        let border = b.dimensions.border_box();
        let content = b.dimensions.content;
        let indent = "  ".repeat(depth);
        let tag = b.tag_name.as_deref().unwrap_or(match &b.box_type {
            BoxType::TextNode(_) => "#text",
            _ => "?",
        });
        let style = b.style.as_ref();
        println!(
            "[{index}] {indent}<{tag}> border=({:.1}, {:.1}, {:.1}x{:.1}) content=({:.1}, {:.1}, {:.1}x{:.1}) pos={:?} display={:?} h={:?} w={:?} max_w={:?}",
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
            style.map(|s| &s.width),
            style.map(|s| &s.max_width),
        );
        println!("      attrs: {:?}", b.attributes);
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
                println!(
                    "      text {:?} baseline_y={:.1} x={:.1} w={:.1} ascent={:.1} descent={:.1} gap={:.1}",
                    truncate(text, 40),
                    run.y(),
                    run.x(),
                    run.width(),
                    ascent,
                    descent,
                    gap,
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
    visit: &mut impl FnMut(usize, &LayoutBox, bool),
) {
    let is_match = node
        .node_id
        .map(|id| matches.contains(&id))
        .unwrap_or(false);
    visit(depth, node, is_match);
    for child in &node.children {
        walk_boxes(child, depth + 1, matches, visit);
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
