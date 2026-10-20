use super::*;

#[test]
fn test_devtools_toggle_and_tabs() {
    let mut dt = DevTools::new();
    assert!(!dt.is_open);
    dt.toggle();
    assert!(dt.is_open);
    dt.open_tab(DevToolsTab::Console);
    assert_eq!(dt.active_tab, DevToolsTab::Console);
    dt.open_tab(DevToolsTab::Computed);
    assert_eq!(dt.active_tab, DevToolsTab::Computed);
    dt.open_tab(DevToolsTab::Performance);
    assert_eq!(dt.active_tab, DevToolsTab::Performance);
    dt.open_tab(DevToolsTab::Debugger);
    assert_eq!(dt.active_tab, DevToolsTab::Debugger);
}

#[test]
fn test_devtools_console_logging() {
    let mut dt = DevTools::new();
    dt.log(ConsoleLevel::Info, "Testing DevTools console");
    dt.log(ConsoleLevel::Error, "Syntax error line 42");
    assert_eq!(dt.console_entries.len(), 2);
    assert_eq!(dt.console_entries[0].level, ConsoleLevel::Info);
    assert_eq!(dt.console_entries[1].level, ConsoleLevel::Error);
}

#[test]
fn test_devtools_network_recording() {
    let mut dt = DevTools::new();
    dt.record_network(
        "https://example.com/api",
        "GET",
        200,
        "application/json",
        1024,
    );
    assert_eq!(dt.network_entries.len(), 1);
    assert_eq!(dt.network_entries[0].status, 200);
    assert_eq!(dt.network_entries[0].method, "GET");
}

#[test]
fn test_devtools_performance_and_debugger() {
    let mut dt = DevTools::new();
    dt.record_metric("paint_frame", 10.0, 1.5, "paint");
    dt.record_metric("dom_layout", 12.0, 3.2, "layout");
    assert_eq!(dt.performance_metrics.len(), 2);

    let bp_id = dt.add_breakpoint("app.js", 42);
    assert_eq!(bp_id, 1);
    assert_eq!(dt.breakpoints.len(), 1);

    dt.set_call_stack(vec![StackFrame {
        function_name: "renderApp".to_string(),
        script_url: "app.js".to_string(),
        line_number: 42,
        column_number: 8,
    }]);
    assert_eq!(dt.call_stack.len(), 1);
}

#[test]
fn test_devtools_render_panel() {
    let mut dt = DevTools::new();
    dt.open_tab(DevToolsTab::Elements);
    dt.log(ConsoleLevel::Log, "Init complete");

    let doc = mango_html::parse_html("<html><body><div id=\"app\">Hello</div></body></html>");
    let cmds = dt.render_panel(800.0, 600.0, &doc, None);

    assert!(
        !cmds.is_empty(),
        "Should generate display list commands for open DevTools"
    );

    // Test render for Computed, Performance, Debugger tabs
    dt.open_tab(DevToolsTab::Computed);
    let cmds_computed = dt.render_panel(800.0, 600.0, &doc, None);
    assert!(!cmds_computed.is_empty());

    dt.open_tab(DevToolsTab::Performance);
    let cmds_perf = dt.render_panel(800.0, 600.0, &doc, None);
    assert!(!cmds_perf.is_empty());

    dt.open_tab(DevToolsTab::Debugger);
    let cmds_dbg = dt.render_panel(800.0, 600.0, &doc, None);
    assert!(!cmds_dbg.is_empty());
}

#[test]
fn test_cdp_session_dispatching() {
    let mut session = CdpSession::new();
    let doc = mango_html::parse_html("<html><body><div id=\"main\">Mango</div></body></html>");

    // Page.enable
    let resp = session.handle_message(r#"{"id":1,"method":"Page.enable"}"#, Some(&doc), None);
    assert!(resp.contains(r#""id":1"#));
    assert!(session.enabled_domains.contains(&"Page".to_string()));

    // DOM.getDocument
    let doc_resp =
        session.handle_message(r#"{"id":2,"method":"DOM.getDocument"}"#, Some(&doc), None);
    assert!(doc_resp.contains("\"nodeName\":\"#document\""));

    // CSS.getComputedStyleForNode
    let css_resp = session.handle_message(
        r#"{"id":3,"method":"CSS.getComputedStyleForNode","params":{"nodeId":"1"}}"#,
        Some(&doc),
        None,
    );
    assert!(css_resp.contains("computedStyle"));

    // Unknown method returns -32601 error
    let err_resp = session.handle_message(r#"{"id":4,"method":"NonExistent.method"}"#, None, None);
    assert!(err_resp.contains("-32601"));
}
