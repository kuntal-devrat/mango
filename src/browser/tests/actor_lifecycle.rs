use super::BrowserChrome;
use crate::renderer_actor::RendererActorHandle;

#[test]
fn test_renderer_actor_shutdown_on_active_tab_close() {
    let mut browser = BrowserChrome::new(800, 600);
    browser.new_tab();
    assert_eq!(browser.tabs.len(), 2);
    assert_eq!(browser.active_tab_idx, 1);

    // Attach renderer actor to active tab 1
    let renderer = RendererActorHandle::spawn(101);
    browser.renderer_actor = Some(renderer);
    assert!(browser.renderer_actor.is_some());

    // Close active tab 1
    browser.close_tab(1);
    assert_eq!(browser.tabs.len(), 1);
    assert_eq!(browser.active_tab_idx, 0);

    // Active renderer actor for closed tab must have been cleanly taken and shut down
    assert!(browser.renderer_actor.is_none());
}

#[test]
fn test_renderer_actor_shutdown_on_background_tab_close() {
    let mut browser = BrowserChrome::new(800, 600);
    browser.new_tab();
    assert_eq!(browser.tabs.len(), 2);

    // Attach renderer actor to tab 1, then switch back to tab 0
    let renderer = RendererActorHandle::spawn(102);
    browser.renderer_actor = Some(renderer);
    browser.switch_tab(0);
    assert_eq!(browser.active_tab_idx, 0);

    // Tab 1 now has its actor in tab.contents
    let tab1_contents = browser.tabs[1].contents.as_ref();
    assert!(tab1_contents.is_some());
    assert!(tab1_contents.unwrap().renderer_actor.is_some());

    // Close background tab 1
    browser.close_tab(1);
    assert_eq!(browser.tabs.len(), 1);
    assert_eq!(browser.active_tab_idx, 0);
}

#[test]
fn test_navigation_actor_clean_shutdown() {
    let loader = mango_net::ResourceLoader::new();
    let mut nav_actor = crate::navigation_actor::NavigationActorHandle::spawn(loader);
    nav_actor.shutdown();
}
