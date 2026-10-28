//! Table formatting context: column sizing, row heights, grid positioning,
//! collapsed border resolution, caption positioning, and fixed/auto table algorithms.
//!
//! Implements CSS 2.1 §17 (Tables) with:
//! - Auto and Fixed table layout algorithms (`table-layout: auto | fixed`)
//! - 2D grid matrix mapping for full `colspan` / `rowspan` support
//! - Multi-row height distribution for spanning cells
//! - Collapsed border model (`border-collapse: collapse`) with CSS conflict resolution
//! - `<caption>` positioning (`caption-side: top | bottom`)
//! - `<colgroup>` and `<col>` span and width distribution
//! - Recursive nested table intrinsic width measurement
//! - Table row and cell percentage height resolution with vertical surplus distribution

pub mod cells;
pub mod collapse;
pub mod columns;
pub mod container;
pub mod intrinsic;
pub mod standalone;
pub mod types;

pub use container::layout_table;
pub(crate) use container::layout_table_contents;
pub use intrinsic::{measure_box_content_intrinsic_widths, measure_box_intrinsic_widths};
