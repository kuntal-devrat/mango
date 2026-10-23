//! Global thread-safe canvas registry for DOM binding and layout blitting.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::context::Canvas2D;

static CANVAS_REGISTRY: OnceLock<RwLock<HashMap<u64, Canvas2D>>> = OnceLock::new();

fn canvas_registry() -> &'static RwLock<HashMap<u64, Canvas2D>> {
    CANVAS_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Executes a closure with mutable access to a canvas, creating it if it doesn't exist.
pub fn with_canvas_mut<F, R>(node_id: u64, width: u32, height: u32, f: F) -> Option<R>
where
    F: FnOnce(&mut Canvas2D) -> R,
{
    let mut reg = canvas_registry().write().ok()?;
    let canvas = reg.entry(node_id).or_insert_with(|| {
        Canvas2D::new(width, height).expect("Failed to initialize canvas pixmap")
    });
    Some(f(canvas))
}

/// Resizes a canvas in the registry, creating it with the new size if not present.
pub fn resize_canvas(node_id: u64, width: u32, height: u32) {
    if let Ok(mut reg) = canvas_registry().write() {
        if let Some(canvas) = reg.get_mut(&node_id) {
            canvas.resize(width, height);
        } else if let Some(canvas) = Canvas2D::new(width, height) {
            reg.insert(node_id, canvas);
        }
    }
}

/// Retrieves canvas pixel data in `0xAARRGGBB` format and dimensions for display list rendering.
pub fn get_canvas_pixels(node_id: u64) -> Option<(u32, u32, Vec<u32>)> {
    let reg = canvas_registry().read().ok()?;
    let canvas = reg.get(&node_id)?;
    Some((canvas.width, canvas.height, canvas.get_pixels()))
}

/// Retrieves canvas dimensions.
pub fn get_canvas_dimensions(node_id: u64) -> Option<(u32, u32)> {
    let reg = canvas_registry().read().ok()?;
    let canvas = reg.get(&node_id)?;
    Some((canvas.width, canvas.height))
}

/// Removes a canvas from the registry when its DOM node is recycled.
pub fn remove_canvas(node_id: u64) {
    if let Ok(mut reg) = canvas_registry().write() {
        reg.remove(&node_id);
    }
}

/// Removes every canvas backing store owned by one document.
pub fn remove_canvases_for_document(document_id: u64) {
    if let Ok(mut reg) = canvas_registry().write() {
        reg.retain(|key, _| (key >> 32) != document_id);
    }
}

/// Clears all canvases from the registry.
pub fn clear_canvas_registry() {
    if let Ok(mut reg) = canvas_registry().write() {
        reg.clear();
    }
}
