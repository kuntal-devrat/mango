//! DOM API bindings that bridge JavaScript to Mango's arena-based DOM.

pub mod helpers;
pub mod types;

pub(crate) use helpers::*;
pub use types::*;

pub const DOM_SHIM: &str = concat!(
    include_str!("js/01_events.js"),
    include_str!("js/02_styles.js"),
    include_str!("js/03_collections.js"),
    include_str!("js/04_canvas.js"),
    include_str!("js/05_mutation.js"),
    include_str!("js/06_prototypes.js"),
    include_str!("js/07_wrap_core.js"),
    include_str!("js/08_wrap_forms.js"),
    include_str!("js/09_wrap_media.js"),
    include_str!("js/10_wrap_misc.js"),
    include_str!("js/11_document.js"),
    include_str!("js/12_components.js"),
    include_str!("js/13_range_selection.js"),
    include_str!("js/14_parsers_and_globals.js"),
);
