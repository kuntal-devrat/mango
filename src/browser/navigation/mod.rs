//! URL navigation, document loading, subresource pipelines, and page lifecycle.
//!
//! Modular architecture:
//! - `pipeline`: Background HTML parsing, stylesheet prefetching, and script harvesting.
//! - `resources`: Web font registration, image harvesting, and title extraction.
//! - `history`: History traversal (back/forward), URL navigation, and HTML document loading.
//! - `lifecycle`: Navigation actor polling, progress ticking, and prepared document application.

pub mod history;
pub mod lifecycle;
pub mod pipeline;
pub mod resources;
pub mod traversal;

pub use pipeline::{
    prepare_document_pipeline, prepare_document_pipeline_cancellable,
    prepare_document_pipeline_from_doc,
};
pub use resources::{
    collect_box_background_images, collect_image_sources, extract_title, load_web_fonts,
};
