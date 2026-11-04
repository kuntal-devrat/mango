//! Integration tests for Desktop Browser Experience (Path 3) and Multi-Process Isolation (Path 4).

use std::fs;

use mango_browser::browser::BrowserChrome;
use mango_browser::browser::bookmarks::BookmarkStore;
use mango_browser::browser::downloads::DownloadManager;
use mango_browser::browser::history::HistoryStore;
use mango_browser::browser::omnibox::{OmniboxCompleter, OmniboxKind, is_likely_url};
use mango_browser::devtools::elements::build_inspector_tree;
use mango_html::dom::Document;
use mango_ipc::{IpcEndpoint, IpcMessage, ProcessType};

#[test]
fn test_history_persistence_and_scoring() {
    let temp_dir = std::env::temp_dir().join(format!("mango_hist_test_{}", std::process::id()));
    let file = temp_dir.join("history.json");

    let mut store = HistoryStore::new();
    store.record_visit("https://crates.io", "Rust Package Registry");
    store.record_visit("https://crates.io", "Rust Package Registry");
    store.record_typed_visit("https://crates.io", "Rust Package Registry");
    store.record_visit("https://docs.rs", "Rust Documentation");

    assert_eq!(store.len(), 2);
    let crates_entry = store.get("https://crates.io").expect("found crates.io");
    assert_eq!(crates_entry.visit_count, 3);
    assert_eq!(crates_entry.typed_count, 1);

    store.save_to_file(&file).expect("save succeeds");
    let loaded = HistoryStore::load_from_file(&file).expect("load succeeds");
    assert_eq!(loaded.len(), 2);

    let search_results = loaded.search("rust", 5);
    assert_eq!(search_results.len(), 2);
    assert_eq!(search_results[0].url, "https://crates.io");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bookmarks_netscape_format_roundtrip() {
    let mut store = BookmarkStore::new();
    let dev_folder = store.add_folder(Some(1), "Development");
    store.add_bookmark(Some(dev_folder), "GitHub", "https://github.com");
    store.add_bookmark(Some(dev_folder), "Rust", "https://www.rust-lang.org");
    store.add_bookmark(Some(1), "DuckDuckGo", "https://duckduckgo.com");

    let html = store.export_netscape_html();
    assert!(html.contains("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
    assert!(html.contains("<H3 ADD_DATE=\""));
    assert!(html.contains("Development</H3>"));
    assert!(html.contains("HREF=\"https://github.com\""));

    let mut restored = BookmarkStore::new();
    let count = restored.import_netscape_html(&html);
    assert_eq!(count, 3);
    assert!(restored.find_by_url("https://github.com").is_some());
    assert!(restored.find_by_url("https://www.rust-lang.org").is_some());
    assert!(restored.find_by_url("https://duckduckgo.com").is_some());
}

#[test]
fn test_omnibox_unified_classification() {
    let mut history = HistoryStore::new();
    history.record_visit("https://news.ycombinator.com", "Hacker News");

    let mut bookmarks = BookmarkStore::new();
    bookmarks.add_bookmark(Some(1), "Google", "https://google.com");

    // 1. Direct URL check
    assert!(is_likely_url("google.com"));
    assert!(is_likely_url("localhost:3000"));
    assert!(!is_likely_url("search for rust"));

    // 2. Query classification
    let direct_matches = OmniboxCompleter::complete("google.com", &history, &bookmarks, None, 5);
    assert_eq!(direct_matches[0].kind, OmniboxKind::DirectUrl);
    assert_eq!(direct_matches[0].url, "https://google.com");

    // 3. Search query fallback
    let search_matches =
        OmniboxCompleter::complete("mango browser rust", &history, &bookmarks, None, 5);
    assert_eq!(search_matches[0].kind, OmniboxKind::Search);
    assert!(search_matches[0].url.contains("duckduckgo.com/?q="));
}

#[test]
fn test_download_manager_pipeline() {
    let mut mgr = DownloadManager::new();
    let temp_dir = std::env::temp_dir();

    let id = mgr.start_download(
        "https://example.com/installer.tar.gz",
        "installer.tar.gz",
        &temp_dir,
        Some(2048),
    );

    mgr.on_data_received(id, 1024);
    assert_eq!(mgr.get(id).unwrap().progress_percentage(), Some(50.0));

    assert!(mgr.pause(id));
    assert_eq!(
        mgr.get(id).unwrap().state,
        mango_browser::browser::downloads::DownloadState::Paused
    );

    assert!(mgr.resume(id));
    mgr.on_data_received(id, 1024);
    assert_eq!(
        mgr.get(id).unwrap().state,
        mango_browser::browser::downloads::DownloadState::Completed
    );
}

#[test]
fn test_devtools_inspector_and_console() {
    let mut doc = Document::new();
    let html = doc.create_element("html", Vec::new());
    let body = doc.create_element("body", Vec::new());
    let btn = doc.create_element("button", vec![("id".into(), "submit-btn".to_string())]);
    let txt = doc.create_text("Submit Form");

    doc.append_child(btn, txt);
    doc.append_child(body, btn);
    doc.append_child(html, body);

    let tree = build_inspector_tree(&doc, html).expect("inspector tree built");
    assert_eq!(tree.tag_name, "html");
    assert_eq!(
        tree.children[0].children[0].format_tag(),
        "<button id=\"submit-btn\">"
    );

    let mut browser = BrowserChrome::new(800, 600);
    let (entry, res) = browser.eval_devtools_console("40 + 2");
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), "42");
    assert_eq!(entry.level, mango_browser::devtools::ConsoleLevel::Log);
}

#[test]
fn test_multi_process_ipc_message_exchange() {
    let (ep_browser, ep_renderer) = IpcEndpoint::create_pair();

    let init_msg = IpcMessage::Init {
        process_id: 42,
        process_type: ProcessType::Renderer,
        site: "https://example.com".to_string(),
    };

    ep_browser.send(&init_msg).expect("browser sends init");
    let received = ep_renderer
        .try_recv()
        .expect("try_recv succeeds")
        .expect("renderer receives init");

    match received {
        IpcMessage::Init {
            process_id,
            process_type,
            site,
        } => {
            assert_eq!(process_id, 42);
            assert_eq!(process_type, ProcessType::Renderer);
            assert_eq!(site, "https://example.com");
        }
        other => panic!("Unexpected IPC message: {:?}", other),
    }

    let ack_msg = IpcMessage::InitAck { process_id: 42 };
    ep_renderer.send(&ack_msg).expect("renderer sends ack");
    let ack_received = ep_browser
        .try_recv()
        .expect("try_recv succeeds")
        .expect("browser receives ack");
    assert_eq!(ack_received, IpcMessage::InitAck { process_id: 42 });
}
