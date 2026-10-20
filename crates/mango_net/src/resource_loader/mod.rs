//! Resource loader: coordinates document fetching, linked stylesheets, and remote images.

pub mod data_uri;
pub mod loader;
pub mod preload;

#[cfg(test)]
mod tests;

pub use data_uri::{
    MAX_DATA_URI_BYTES, decode_base64_bytes, decode_data_uri_bytes, decode_percent_bytes,
};
pub use loader::{FetchedDocument, ResourceLoader};
pub use preload::{
    FetchPriority, PreloadItem, PreloadKind, PreloadScanResult, scan_html_for_preloads,
};
