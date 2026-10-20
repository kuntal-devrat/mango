//! Page layout, styling, display list construction, and rendering.
//!
//! Modular architecture:
//! - `layout`: Viewport layout, geometry calculation, canvas background, and zoom controls.
//! - `page`: Page content display list translation, viewport clipping, and search highlights.
//! - `chrome`: Chrome display list construction (scrollbar, tab strip, toolbar, omnibox, status bar, devtools, find-in-page).
//! - `layers`: Layer tree management, tile rasterization, and damage invalidation.
//! - `overlays`: Popup menus, select dropdowns, pickers (color, date, time, file), validation bubble.

pub mod chrome;
pub mod layers;
pub mod layout;
pub mod overlays;
pub mod page;

pub use layout::canvas_background_color;
