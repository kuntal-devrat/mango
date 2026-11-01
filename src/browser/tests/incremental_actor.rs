//! Tests for incremental style/layout invalidation and actor frame delivery.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mango_html::parse_html;
use mango_net::CookieJar;

use crate::renderer_actor::{LoadDocumentPayload, RendererActorHandle, RendererResult};

fn test_payload(url: &str, html: &str, width: f32, height: f32) -> LoadDocumentPayload {
    let doc = parse_html(html);
    LoadDocumentPayload {
        url: url.to_string(),
        doc,
        html: html.to_string(),
        stylesheets: Vec::new(),
        preloaded_scripts: Vec::new(),
        viewport_width: width,
        viewport_height: height,
        base_url: mango_net::Url::parse(url).ok(),
        generation: 1,
        local_storage: Arc::new(Mutex::new(HashMap::new())),
        session_storage: Arc::new(Mutex::new(HashMap::new())),
        cookie_jar: Arc::new(Mutex::new(CookieJar::new())),
        cache: None,
    }
}

#[test]
fn test_renderer_actor_incremental_dom_mutation_and_frame_ready() {
    let mut renderer = RendererActorHandle::spawn(401);
    let html = r#"<!doctype html>
    <html>
        <head>
            <title>Incremental Test</title>
            <style>
                #item { color: rgb(0, 0, 0); }
                .urgent { color: rgb(255, 0, 0); }
            </style>
        </head>
        <body>
            <div id="item">Normal text</div>
        </body>
    </html>"#;

    renderer.load_document(test_payload("https://example.com/incr", html, 800.0, 600.0));

    // Wait for initial frame
    let first = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Initial frame ready");
    match first {
        RendererResult::FrameReady(frame) => {
            assert_eq!(frame.title.as_deref(), Some("Incremental Test"));
            assert!(!frame.display_list.is_empty());
        }
        _ => panic!("Expected FrameReady"),
    }

    // Mutate an attribute via execute_script -> triggers incremental style invalidation
    renderer.execute_script(
        "document.getElementById('item').className = 'urgent'; document.title = 'Updated Title';"
            .to_string(),
    );

    // Wait for second frame
    let second = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Updated frame ready");
    match second {
        RendererResult::FrameReady(frame) => {
            assert_eq!(frame.title.as_deref(), Some("Updated Title"));
            assert!(!frame.display_list.is_empty());
        }
        _ => panic!("Expected FrameReady on mutation"),
    }

    renderer.shutdown();
}

#[test]
fn test_renderer_actor_incremental_viewport_resize() {
    let mut renderer = RendererActorHandle::spawn(402);
    let html = r#"<!doctype html>
    <html>
        <head><title>Resize Test</title></head>
        <body><div style="width: 100%; height: 50px;">Full width bar</div></body>
    </html>"#;

    renderer.load_document(test_payload(
        "https://example.com/resize",
        html,
        800.0,
        600.0,
    ));

    let first = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Initial frame");
    assert!(matches!(first, RendererResult::FrameReady(_)));

    // Resize viewport
    renderer.resize(400.0, 300.0);

    let second = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Resized frame");
    match second {
        RendererResult::FrameReady(frame) => {
            assert_eq!(frame.title.as_deref(), Some("Resize Test"));
            assert!(!frame.display_list.is_empty());
        }
        _ => panic!("Expected FrameReady on resize"),
    }

    renderer.shutdown();
}

#[test]
fn test_renderer_actor_incremental_tree_growth() {
    let mut renderer = RendererActorHandle::spawn(403);
    let html = r#"<!doctype html>
    <html>
        <head><title>Growth Test</title></head>
        <body><div id="container">Original child</div></body>
    </html>"#;

    renderer.load_document(test_payload(
        "https://example.com/growth",
        html,
        800.0,
        600.0,
    ));

    let first = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Initial frame");
    assert!(matches!(first, RendererResult::FrameReady(_)));

    // Append child to DOM tree
    renderer.execute_script(
        r#"
        var p = document.createElement("p");
        p.textContent = "New dynamically appended paragraph";
        document.body.appendChild(p);
        document.title = "Growth Done";
    "#
        .to_string(),
    );

    let second = renderer
        .result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Growth frame");
    match second {
        RendererResult::FrameReady(frame) => {
            assert_eq!(frame.title.as_deref(), Some("Growth Done"));
            assert!(!frame.display_list.is_empty());
        }
        _ => panic!("Expected FrameReady on tree growth"),
    }

    renderer.shutdown();
}
