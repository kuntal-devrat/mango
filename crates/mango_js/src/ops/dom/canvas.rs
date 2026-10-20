use deno_core::{OpState, op2};

use crate::dom_bindings::DomDirtyFlag;

// ── Canvas 2D Ops ────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_canvas_resize(state: &mut OpState, node_id: u32, width: u32, height: u32) {
    mango_render::resize_canvas(node_id as usize, width, height);
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_set_fill_style(
    _state: &mut OpState,
    node_id: u32,
    #[string] color_str: String,
) {
    let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_fill_style(color);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_stroke_style(
    _state: &mut OpState,
    node_id: u32,
    #[string] color_str: String,
) {
    let color = mango_render::color::parse_color(&color_str).unwrap_or(mango_core::Color::BLACK);
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_stroke_style(color);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_width(_state: &mut OpState, node_id: u32, width: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_width(width);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_cap(_state: &mut OpState, node_id: u32, #[string] cap: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_cap(&cap);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_line_join(_state: &mut OpState, node_id: u32, #[string] join: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_line_join(&join);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_miter_limit(_state: &mut OpState, node_id: u32, limit: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_miter_limit(limit);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_global_alpha(_state: &mut OpState, node_id: u32, alpha: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_global_alpha(alpha);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_font(_state: &mut OpState, node_id: u32, #[string] font: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_font(&font);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_text_align(_state: &mut OpState, node_id: u32, #[string] align: String) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_text_align(&align);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_set_text_baseline(
    _state: &mut OpState,
    node_id: u32,
    #[string] baseline: String,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.set_text_baseline(&baseline);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_fill_rect(
    state: &mut OpState,
    node_id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke_rect(
    state: &mut OpState,
    node_id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_clear_rect(
    state: &mut OpState,
    node_id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.clear_rect(x, y, w, h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_begin_path(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.begin_path();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_close_path(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.close_path();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_move_to(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.move_to(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_line_to(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.line_to(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_arc(
    _state: &mut OpState,
    node_id: u32,
    x: f32,
    y: f32,
    r: f32,
    sa: f32,
    ea: f32,
    ccw: bool,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.arc(x, y, r, sa, ea, ccw);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_fill(state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill();
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke(state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke();
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_fill_text(
    state: &mut OpState,
    node_id: u32,
    #[string] text: String,
    x: f32,
    y: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.fill_text(&text, x, y, None);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_stroke_text(
    state: &mut OpState,
    node_id: u32,
    #[string] text: String,
    x: f32,
    y: f32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.stroke_text(&text, x, y, None);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2(fast)]
pub fn op_mango_canvas_measure_text(
    _state: &mut OpState,
    node_id: u32,
    #[string] text: String,
) -> f32 {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| c.measure_text(&text))
        .unwrap_or(0.0)
}

#[op2(fast)]
pub fn op_mango_canvas_save(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.save();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_restore(_state: &mut OpState, node_id: u32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.restore();
    });
}

#[op2(fast)]
pub fn op_mango_canvas_translate(_state: &mut OpState, node_id: u32, x: f32, y: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.translate(x, y);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_rotate(_state: &mut OpState, node_id: u32, angle: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.rotate(angle);
    });
}

#[op2(fast)]
pub fn op_mango_canvas_scale(_state: &mut OpState, node_id: u32, sx: f32, sy: f32) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.scale(sx, sy);
    });
}

#[op2]
#[serde]
pub fn op_mango_canvas_get_image_data(
    _state: &mut OpState,
    node_id: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Vec<u8> {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.get_image_data(x as i32, y as i32, w, h)
    })
    .unwrap_or_else(|| vec![0u8; (w * h * 4) as usize])
}

#[op2]
pub fn op_mango_canvas_put_image_data(
    state: &mut OpState,
    node_id: u32,
    #[serde] data: Vec<u8>,
    dx: i32,
    dy: i32,
    dirty_x: i32,
    dirty_y: i32,
    dirty_w: u32,
    dirty_h: u32,
    src_width: u32,
) {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| {
        c.put_image_data_with_stride(&data, src_width, dx, dy, dirty_x, dirty_y, dirty_w, dirty_h);
    });
    if let Some(dirty) = state.try_borrow::<DomDirtyFlag>() {
        *dirty.borrow_mut() = true;
    }
}

#[op2]
#[string]
pub fn op_mango_canvas_to_data_url(_state: &mut OpState, node_id: u32) -> String {
    mango_render::with_canvas_mut(node_id as usize, 300, 150, |c| c.to_data_url("image/png"))
        .unwrap_or_default()
}
