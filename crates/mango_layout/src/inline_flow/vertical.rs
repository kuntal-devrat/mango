use mango_core::Point;
use mango_css::values::WritingMode;

use crate::box_model::BoxType;
use crate::box_tree::LayoutBox;
use crate::float::FloatContext;

use super::atom::{InlineAtom, InlineAtomKind};
use super::collector::collect_inline_atoms;

pub(crate) fn layout_vertical_inline_children(
    container: &mut LayoutBox,
    _float_ctx: &mut FloatContext,
    writing_mode: WritingMode,
) -> f32 {
    let container_x = container.dimensions.content.x();
    let container_y = container.dimensions.content.y();
    let container_width = container.dimensions.content.width();
    let max_col_h = if container.dimensions.content.height() > 0.0 {
        container.dimensions.content.height()
    } else {
        400.0
    };

    if container.raw_children.is_none() {
        container.raw_children = Some(std::mem::take(&mut container.children));
    }

    let mut atoms: Vec<InlineAtom> = Vec::new();
    if let Some(source_children) = container.raw_children.as_deref() {
        for child in source_children {
            collect_inline_atoms(
                child,
                &mut atoms,
                child.link_target.as_deref(),
                child.link_frame.as_deref(),
                container_width,
                max_col_h,
            );
        }
    }

    if atoms.is_empty() {
        let mut oof_children = Vec::new();
        if let Some(source_children) = container.raw_children.as_deref() {
            for sc in source_children {
                let is_oof = sc.style.as_ref().is_some_and(|s| {
                    matches!(
                        s.position,
                        mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                    )
                });
                if is_oof {
                    oof_children.push(sc.clone());
                }
            }
        }
        container.children = oof_children;
        return 0.0;
    }

    struct VertLine {
        boxes: Vec<LayoutBox>,
        height: f32,
    }

    let mut lines: Vec<VertLine> = Vec::new();
    let mut cur_line_boxes: Vec<LayoutBox> = Vec::new();
    let mut cur_y = container_y;
    let base_col_w = container
        .style
        .as_ref()
        .map_or(24.0, |s| s.font_size * 1.5)
        .max(16.0);

    for atom in atoms {
        if matches!(atom.kind, InlineAtomKind::LineBreak) {
            lines.push(VertLine {
                boxes: std::mem::take(&mut cur_line_boxes),
                height: cur_y - container_y,
            });
            cur_y = container_y;
            continue;
        }

        match atom.kind {
            InlineAtomKind::Text { text, is_space } => {
                if is_space && cur_line_boxes.is_empty() {
                    continue;
                }
                let font_size = atom.style.font_size;
                let glyph_adv = (font_size * 1.2).ceil();

                for ch in text.chars() {
                    if cur_y + glyph_adv > container_y + max_col_h && !cur_line_boxes.is_empty() {
                        lines.push(VertLine {
                            boxes: std::mem::take(&mut cur_line_boxes),
                            height: cur_y - container_y,
                        });
                        cur_y = container_y;
                    }

                    let mut box_node =
                        LayoutBox::new(BoxType::TextNode(ch.to_string()), Some(atom.style.clone()));
                    box_node.link_target = atom.link_target.clone();
                    box_node.dimensions.content.origin = Point::new(0.0, cur_y);
                    box_node.dimensions.content.size = mango_core::Size::new(base_col_w, glyph_adv);
                    cur_line_boxes.push(box_node);
                    cur_y += glyph_adv;
                }
            }
            InlineAtomKind::AtomicBox(mut atomic_box) => {
                let adv = atomic_box.dimensions.margin_box().height().max(16.0);
                if cur_y + adv > container_y + max_col_h && !cur_line_boxes.is_empty() {
                    lines.push(VertLine {
                        boxes: std::mem::take(&mut cur_line_boxes),
                        height: cur_y - container_y,
                    });
                    cur_y = container_y;
                }
                atomic_box.dimensions.content.origin = Point::new(0.0, cur_y);
                cur_line_boxes.push(*atomic_box);
                cur_y += adv;
            }
            InlineAtomKind::Spacing => {
                cur_y += atom.width;
            }
            _ => {}
        }
    }

    if !cur_line_boxes.is_empty() {
        lines.push(VertLine {
            boxes: cur_line_boxes,
            height: cur_y - container_y,
        });
    }

    let num_lines = lines.len().max(1);
    let total_width = num_lines as f32 * base_col_w;
    let eff_container_w = if container_width > 0.0 {
        container_width
    } else {
        total_width
    };

    let mut positioned_boxes = Vec::new();
    let mut max_line_h = 0.0f32;

    for (line_idx, line) in lines.into_iter().enumerate() {
        max_line_h = max_line_h.max(line.height);
        let line_x = match writing_mode {
            WritingMode::VerticalRl => {
                container_x + eff_container_w - (line_idx as f32 + 1.0) * base_col_w
            }
            _ => {
                // VerticalLr or fallback
                container_x + line_idx as f32 * base_col_w
            }
        };

        for mut b in line.boxes {
            b.dimensions.content.origin.x = line_x;
            positioned_boxes.push(b);
        }
    }

    if container.dimensions.content.size.width <= 0.0 {
        container.dimensions.content.size.width = total_width;
    }

    if let Some(source_children) = container.raw_children.as_deref() {
        for sc in source_children {
            let is_oof = sc.style.as_ref().is_some_and(|s| {
                matches!(
                    s.position,
                    mango_css::values::Position::Absolute | mango_css::values::Position::Fixed
                )
            });
            if is_oof {
                positioned_boxes.push(sc.clone());
            }
        }
    }

    container.children = positioned_boxes;
    max_line_h
}
