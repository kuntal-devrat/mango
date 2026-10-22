//! Non-DOM Web API bindings for the Mango JS runtime.

pub mod crypto;
pub mod storage;
pub mod types;

pub(crate) use crypto::*;
pub use storage::*;
pub use types::*;

pub const WEB_APIS_SHIM_TEMPLATE: &str = concat!(
    include_str!("js/01_window_player.js"),
    include_str!("js/02_environment.js"),
    include_str!("js/03_timers_media_css.js"),
    include_str!("js/04_encoding_crypto_data.js"),
    include_str!("js/05_streams.js"),
    include_str!("js/06_fetch.js"),
    include_str!("js/07_xhr_websocket.js"),
    include_str!("js/08_dom_nodes.js"),
    include_str!("js/09_dom_elements.js"),
    include_str!("js/10_dom_svg_html.js"),
    include_str!("js/11_dom_other.js"),
    include_str!("js/12_observers_events.js"),
    include_str!("js/13_components_workers.js"),
    include_str!("js/14_workers_channels_locks.js"),
    include_str!("js/15_indexeddb_core.js"),
    include_str!("js/16_indexeddb_ops.js"),
    include_str!("js/17_intl.js"),
    include_str!("js/18_audio_canvas.js"),
    include_str!("js/19_webcodecs.js"),
    include_str!("js/20_opfs_transport_coi.js"),
    include_str!("js/21_test_driver.js"),
    include_str!("js/22_cssom_rules.js"),
    include_str!("js/23_cssom_parser.js"),
);

pub fn build_web_apis_shim(page_url: &str) -> String {
    let safe_url = page_url.replace('\\', "\\\\").replace('"', "\\\"");
    WEB_APIS_SHIM_TEMPLATE.replace("__MANGO_PAGE_URL__", &safe_url)
}
