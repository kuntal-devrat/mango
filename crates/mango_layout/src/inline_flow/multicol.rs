use mango_core::Point;

use crate::box_tree::LayoutBox;

pub(crate) fn fragment_multicol_inline_boxes(
    positioned_boxes: Vec<LayoutBox>,
    resolved_cols: usize,
    target_col_h: f32,
    current_y: f32,
    container_x: f32,
    container_y: f32,
    layout_width: f32,
    gap: f32,
    is_container_rtl: bool,
) -> Vec<LayoutBox> {
    let col_h = if target_col_h > 0.0 {
        target_col_h
    } else {
        let total_content_h = current_y - container_y;
        (total_content_h / resolved_cols as f32).max(20.0)
    };

    let mut fragmented_boxes = Vec::new();
    for mut b in positioned_boxes {
        let rel_top = (b.dimensions.content.y() - container_y).max(0.0);
        let rel_bottom = (b.dimensions.content.bottom() - container_y).max(0.0);
        let start_col = (rel_top / col_h).floor() as usize;
        let end_col = if rel_bottom > rel_top {
            ((rel_bottom - 0.001) / col_h).floor() as usize
        } else {
            start_col
        };
        let start_col = start_col.min(resolved_cols - 1);
        let end_col = end_col.min(resolved_cols - 1);

        if start_col == end_col {
            let col_idx = start_col;
            let col_x = if is_container_rtl {
                container_x + (resolved_cols - 1 - col_idx) as f32 * (layout_width + gap)
            } else {
                container_x + col_idx as f32 * (layout_width + gap)
            };
            let dx = col_x - container_x;
            let dy = -(col_idx as f32 * col_h);
            b.dimensions.content.origin.x += dx;
            b.dimensions.content.origin.y += dy;
            crate::block_flow::shift_descendants(&mut b, dx, dy);
            fragmented_boxes.push(b);
        } else {
            for c in start_col..=end_col {
                let col_top = c as f32 * col_h;
                let col_bottom = (c + 1) as f32 * col_h;
                let slice_top = rel_top.max(col_top);
                let slice_bottom = rel_bottom.min(col_bottom);
                let slice_h = slice_bottom - slice_top;
                if slice_h > 0.0 {
                    let mut frag = b.clone();
                    let col_x = if is_container_rtl {
                        container_x + (resolved_cols - 1 - c) as f32 * (layout_width + gap)
                    } else {
                        container_x + c as f32 * (layout_width + gap)
                    };
                    let new_x = col_x + (b.dimensions.content.x() - container_x);
                    let new_y = container_y + (slice_top - col_top);
                    let dx = new_x - frag.dimensions.content.x();
                    let dy = new_y - frag.dimensions.content.y();
                    frag.dimensions.content.origin = Point::new(new_x, new_y);
                    frag.dimensions.content.size.height = slice_h;
                    crate::block_flow::shift_descendants(&mut frag, dx, dy);
                    fragmented_boxes.push(frag);
                }
            }
        }
    }
    fragmented_boxes
}
