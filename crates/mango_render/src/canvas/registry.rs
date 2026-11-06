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
    use std::collections::hash_map::Entry;
    let canvas = match reg.entry(node_id) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let new_canvas = Canvas2D::new(width, height)?;
            entry.insert(new_canvas)
        }
    };
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

/// Copies pixel contents from one canvas to another canvas with transformation and blending.
pub fn copy_canvas_to_canvas(
    src_id: u64,
    dst_id: u64,
    _sx: f32,
    _sy: f32,
    _sw: f32,
    _sh: f32,
    dx: f32,
    dy: f32,
    _dw: f32,
    _dh: f32,
) {
    if src_id == dst_id {
        return;
    }
    let Ok(mut reg) = canvas_registry().write() else {
        return;
    };
    let Some(src_pixmap) = reg.get(&src_id).map(|c| c.pixmap.clone()) else {
        return;
    };
    if let Some(dst_canvas) = reg.get_mut(&dst_id) {
        let paint = tiny_skia::PixmapPaint::default();
        let transform = dst_canvas.state.transform;
        dst_canvas.pixmap.draw_pixmap(
            dx as i32,
            dy as i32,
            src_pixmap.as_ref(),
            &paint,
            transform,
            None,
        );
    }
}

/// Clears all canvases from the registry.
pub fn clear_canvas_registry() {
    if let Ok(mut reg) = canvas_registry().write() {
        reg.clear();
    }
}
