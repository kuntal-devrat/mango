//! Display list: a flat list of paint commands.
//!
//! The layout engine produces a tree of boxes. The display list flattens
//! this into an ordered sequence of draw commands that can be rasterized
//! efficiently in a single pass.

use mango_core::{Color, Rect};

use crate::font::{FontFamily, FontStyle, FontWeight, TextDecoration};

/// A single paint command in the display list.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayCommand {
    /// Fill a rectangle with a solid color.
    FillRect { rect: Rect, color: Color },

    /// Fill a rounded rectangle with a solid color and corner radii [top-left, top-right, bottom-right, bottom-left].
    FillRoundedRect {
        rect: Rect,
        color: Color,
        radii: [f32; 4],
    },

    /// Draw a box shadow around a rectangle.
    DrawBoxShadow {
        rect: Rect,
        color: Color,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        radii: [f32; 4],
        inset: bool,
    },

    /// Draw a border around a rectangle, optionally with corner radii [top-left, top-right, bottom-right, bottom-left].
    DrawBorder {
        rect: Rect,
        color: Color,
        widths: BorderWidths,
        radii: [f32; 4],
    },

    /// Draw a text string at a position.
    DrawText {
        text: String,
        x: f32,
        y: f32,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        decoration: TextDecoration,
        letter_spacing: f32,
    },

    /// Draw a horizontal line (e.g., `<hr>`).
    DrawLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        thickness: f32,
    },

    /// Draw a decoded image at a position.
    DrawImage {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        /// Pixel data in `0xRRGGBB` format, row-major.
        pixels: Vec<u32>,
    },

    /// Fill a rectangle with a CSS gradient.
    FillGradient {
        rect: Rect,
        /// The parsed CSS gradient (linear, radial, or conic).
        gradient: Box<mango_css::values::Gradient>,
        /// Corner radii `[top-left, top-right, bottom-right, bottom-left]`.
        radii: [f32; 4],
        /// Multiplies the gradient's alpha (used for inherited `opacity`).
        opacity: f32,
    },

    /// Draw a blurred text shadow behind a text run.
    DrawTextShadow {
        text: String,
        x: f32,
        y: f32,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        blur_radius: f32,
        letter_spacing: f32,
    },

    /// Push a 2D affine transform (`matrix(a, b, c, d, e, f)`) onto the transform stack.
    ///
    /// All subsequent drawing commands are mapped through the accumulated matrix
    /// until the matching [`DisplayCommand::PopTransform`].
    PushTransform { matrix: [f32; 6] },

    /// Pop the top transform from the transform stack.
    PopTransform,

    /// Push a clipping rectangle onto the clip stack.
    PushClip { rect: Rect },

    /// Pop the top clipping rectangle from the clip stack.
    PopClip,

    /// Push a filter list applied to subsequent drawing commands.
    PushFilter {
        filters: Vec<mango_css::values::FilterFunction>,
        rect: Rect,
    },

    /// Pop the top filter from the filter stack.
    PopFilter,

    /// Push a backdrop filter applied to background pixels under `rect`.
    PushBackdropFilter {
        filters: Vec<mango_css::values::FilterFunction>,
        rect: Rect,
    },

    /// Pop the top backdrop filter.
    PopBackdropFilter,

    /// Push a blend mode for compositing subsequent commands.
    PushBlendMode { mode: mango_css::values::BlendMode },

    /// Pop the top blend mode.
    PopBlendMode,

    /// Push a clip path shape for clipping subsequent commands.
    PushClipPath {
        clip_path: Box<mango_css::values::ClipPath>,
        rect: Rect,
    },

    /// Pop the top clip path.
    PopClipPath,

    /// Draw a 9-slice scaled border image around a rectangle.
    DrawBorderImage {
        rect: Rect,
        /// Decoded image pixels in 0xRRGGBB / 0xAARRGGBB format.
        pixels: Vec<u32>,
        img_width: u32,
        img_height: u32,
        /// Slices in source image coordinates: [top, right, bottom, left].
        slice: [f32; 4],
        /// Destination border widths: [top, right, bottom, left].
        widths: BorderWidths,
        /// Horizontal repeat mode.
        repeat_h: mango_css::values::BorderImageRepeat,
        /// Vertical repeat mode.
        repeat_v: mango_css::values::BorderImageRepeat,
        /// Whether the middle slice is painted.
        fill: bool,
    },

    /// Draw text clipped to a gradient fill (used for `background-clip: text`).
    DrawTextWithGradient {
        text: String,
        x: f32,
        y: f32,
        gradient: Box<mango_css::values::Gradient>,
        gradient_rect: Rect,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
        style: FontStyle,
        decoration: TextDecoration,
        letter_spacing: f32,
    },
}

impl DisplayCommand {
    /// Convenience constructor for text with default normal style and no decoration.
    pub fn draw_text(
        text: impl Into<String>,
        x: f32,
        y: f32,
        color: Color,
        font_size: f32,
        weight: FontWeight,
        family: FontFamily,
    ) -> Self {
        Self::DrawText {
            text: text.into(),
            x,
            y,
            color,
            font_size,
            weight,
            family,
            style: FontStyle::Normal,
            decoration: TextDecoration::None,
            letter_spacing: 0.0,
        }
    }
}

/// Border widths for each side.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BorderWidths {
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn all(width: f32) -> Self {
        Self {
            top: width,
            right: width,
            bottom: width,
            left: width,
        }
    }
}

impl DisplayCommand {
    /// Returns the approximate 2D bounding rectangle affected by this drawing command.
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            DisplayCommand::FillRect { rect, .. } => Some(*rect),
            DisplayCommand::FillRoundedRect { rect, .. } => Some(*rect),
            DisplayCommand::DrawBoxShadow {
                rect,
                offset_x,
                offset_y,
                blur_radius,
                spread_radius,
                ..
            } => {
                let pad = blur_radius + spread_radius;
                Some(Rect::new(
                    rect.x() + offset_x - pad,
                    rect.y() + offset_y - pad,
                    rect.width() + pad * 2.0,
                    rect.height() + pad * 2.0,
                ))
            }
            DisplayCommand::DrawBorder { rect, widths, .. } => Some(Rect::new(
                rect.x() - widths.left,
                rect.y() - widths.top,
                rect.width() + widths.left + widths.right,
                rect.height() + widths.top + widths.bottom,
            )),
            DisplayCommand::DrawText {
                text,
                x,
                y,
                font_size,
                ..
            } => {
                let approx_w = (text.len() as f32) * font_size * 0.6;
                Some(Rect::new(*x, *y, approx_w, *font_size * 1.2))
            }
            DisplayCommand::DrawLine {
                x1,
                y1,
                x2,
                y2,
                thickness,
                ..
            } => {
                let min_x = x1.min(*x2) - thickness / 2.0;
                let min_y = y1.min(*y2) - thickness / 2.0;
                let max_x = x1.max(*x2) + thickness / 2.0;
                let max_y = y1.max(*y2) + thickness / 2.0;
                Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
            }
            DisplayCommand::DrawImage {
                x,
                y,
                width,
                height,
                ..
            } => Some(Rect::new(*x, *y, *width, *height)),
            DisplayCommand::FillGradient { rect, .. } => Some(*rect),
            DisplayCommand::DrawTextShadow {
                text,
                x,
                y,
                font_size,
                blur_radius,
                ..
            } => {
                let approx_w = (text.len() as f32) * font_size * 0.6;
                Some(Rect::new(
                    *x - blur_radius,
                    *y - blur_radius,
                    approx_w + blur_radius * 2.0,
                    font_size * 1.2 + blur_radius * 2.0,
                ))
            }
            DisplayCommand::DrawBorderImage { rect, widths, .. } => Some(Rect::new(
                rect.x() - widths.left,
                rect.y() - widths.top,
                rect.width() + widths.left + widths.right,
                rect.height() + widths.top + widths.bottom,
            )),
            DisplayCommand::DrawTextWithGradient {
                text,
                x,
                y,
                font_size,
                ..
            } => {
                let approx_w = (text.len() as f32) * font_size * 0.6;
                Some(Rect::new(*x, *y, approx_w, *font_size * 1.2))
            }
            DisplayCommand::PushClip { rect }
            | DisplayCommand::PushFilter { rect, .. }
            | DisplayCommand::PushBackdropFilter { rect, .. }
            | DisplayCommand::PushClipPath { rect, .. } => Some(*rect),
            DisplayCommand::PopClip
            | DisplayCommand::PushTransform { .. }
            | DisplayCommand::PopTransform
            | DisplayCommand::PopFilter
            | DisplayCommand::PopBackdropFilter
            | DisplayCommand::PushBlendMode { .. }
            | DisplayCommand::PopBlendMode
            | DisplayCommand::PopClipPath => None,
        }
    }
}

/// An operation to apply when patching a display list (ARCH-004).
#[derive(Debug, Clone, PartialEq)]
pub enum DiffOp {
    /// Command at index is unchanged.
    Retain { count: usize },
    /// Commands were replaced or inserted at this index.
    Insert {
        index: usize,
        command: DisplayCommand,
    },
    /// Command was removed at this index.
    Remove { index: usize },
}

/// The result of diffing two display lists (ARCH-004).
///
/// Contains the sequence of operations between an old and new display list,
/// along with the consolidated damaged/dirty bounding rect to enable minimal repainting.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisplayListDiff {
    pub ops: Vec<DiffOp>,
    pub changed_count: usize,
    pub damage_rect: Option<Rect>,
    pub is_identical: bool,
    pub old_count: usize,
    pub new_count: usize,
}

impl DisplayListDiff {
    /// Computes the difference between `old_list` and `new_list` (ARCH-004).
    ///
    /// Identifies identical prefixes and suffixes to isolate the exact changed range,
    /// emits targeted operations, and calculates the minimal bounding box of
    /// the damaged screen region.
    pub fn diff(old_list: &DisplayList, new_list: &DisplayList) -> Self {
        if old_list.commands == new_list.commands {
            return Self {
                ops: vec![DiffOp::Retain {
                    count: old_list.len(),
                }],
                changed_count: 0,
                damage_rect: None,
                is_identical: true,
                old_count: old_list.len(),
                new_count: new_list.len(),
            };
        }

        let old_cmds = old_list.as_slice();
        let new_cmds = new_list.as_slice();

        // Find common prefix
        let mut prefix_len = 0;
        while prefix_len < old_cmds.len()
            && prefix_len < new_cmds.len()
            && old_cmds[prefix_len] == new_cmds[prefix_len]
        {
            prefix_len += 1;
        }

        // Find common suffix
        let mut old_suffix = old_cmds.len();
        let mut new_suffix = new_cmds.len();
        while old_suffix > prefix_len
            && new_suffix > prefix_len
            && old_cmds[old_suffix - 1] == new_cmds[new_suffix - 1]
        {
            old_suffix -= 1;
            new_suffix -= 1;
        }

        let mut ops = Vec::new();
        if prefix_len > 0 {
            ops.push(DiffOp::Retain { count: prefix_len });
        }

        let mut damage: Option<Rect> = None;
        let mut union_damage = |rect_opt: Option<Rect>| {
            if let Some(r) = rect_opt {
                damage = match damage {
                    None => Some(r),
                    Some(cur) => Some(cur.union(&r)),
                };
            }
        };

        // Old commands that were removed/replaced contribute to damage
        for old_cmd in &old_cmds[prefix_len..old_suffix] {
            union_damage(old_cmd.bounds());
        }

        // New commands that were added/replaced contribute to ops and damage
        let changed_range = &new_cmds[prefix_len..new_suffix];
        for (idx_offset, new_cmd) in changed_range.iter().enumerate() {
            let cmd_idx = prefix_len + idx_offset;
            union_damage(new_cmd.bounds());
            ops.push(DiffOp::Insert {
                index: cmd_idx,
                command: new_cmd.clone(),
            });
        }

        let suffix_count = old_cmds.len() - old_suffix;
        if suffix_count > 0 {
            ops.push(DiffOp::Retain {
                count: suffix_count,
            });
        }

        let changed_count = (old_suffix - prefix_len) + (new_suffix - prefix_len);

        Self {
            ops,
            changed_count,
            damage_rect: damage,
            is_identical: false,
            old_count: old_cmds.len(),
            new_count: new_cmds.len(),
        }
    }

    /// Returns `true` if there are any differences between the two display lists.
    pub fn has_changes(&self) -> bool {
        !self.is_identical
    }

    /// Returns the bounding rectangle of the damaged area requiring redrawing.
    pub fn damage_rect(&self) -> Option<Rect> {
        self.damage_rect
    }
}

/// An ordered list of display commands ready for painting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Adds a command to the display list.
    pub fn push(&mut self, command: DisplayCommand) {
        self.commands.push(command);
    }

    /// Returns an iterator over the display commands.
    pub fn iter(&self) -> impl Iterator<Item = &DisplayCommand> {
        self.commands.iter()
    }

    /// Returns the number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if the display list is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a slice of the display commands.
    pub fn as_slice(&self) -> &[DisplayCommand] {
        &self.commands
    }

    /// Clears all commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Computes the difference between this display list and another (ARCH-004).
    pub fn diff(&self, new_list: &DisplayList) -> DisplayListDiff {
        DisplayListDiff::diff(self, new_list)
    }
}

impl std::ops::Deref for DisplayList {
    type Target = [DisplayCommand];

    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

impl IntoIterator for DisplayList {
    type Item = DisplayCommand;
    type IntoIter = std::vec::IntoIter<DisplayCommand>;

    fn into_iter(self) -> Self::IntoIter {
        self.commands.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_list_diff_identical() {
        let mut dl1 = DisplayList::new();
        dl1.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });

        let dl2 = dl1.clone();
        let diff = dl1.diff(&dl2);

        assert!(diff.is_identical);
        assert!(!diff.has_changes());
        assert_eq!(diff.damage_rect(), None);
        assert_eq!(diff.changed_count, 0);
    }

    #[test]
    fn test_display_list_diff_modified_command() {
        let mut dl1 = DisplayList::new();
        dl1.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });
        dl1.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 100.0, 100.0, 100.0),
            color: Color::BLUE,
        });

        let mut dl2 = DisplayList::new();
        dl2.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 100.0, 100.0),
            color: Color::RED,
        });
        // Modified color from BLUE to GREEN
        dl2.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 100.0, 100.0, 100.0),
            color: Color::GREEN,
        });

        let diff = dl1.diff(&dl2);
        assert!(!diff.is_identical);
        assert!(diff.has_changes());
        assert_eq!(diff.changed_count, 2); // 1 old replaced + 1 new inserted
        assert_eq!(
            diff.damage_rect(),
            Some(Rect::new(0.0, 100.0, 100.0, 100.0))
        );
    }

    #[test]
    fn test_display_list_diff_appended_command() {
        let mut dl1 = DisplayList::new();
        dl1.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            color: Color::RED,
        });

        let mut dl2 = dl1.clone();
        dl2.push(DisplayCommand::FillRect {
            rect: Rect::new(50.0, 50.0, 50.0, 50.0),
            color: Color::GREEN,
        });

        let diff = dl1.diff(&dl2);
        assert!(!diff.is_identical);
        assert_eq!(diff.ops.len(), 2); // Retain(1) + Insert(1)
        assert_eq!(diff.damage_rect(), Some(Rect::new(50.0, 50.0, 50.0, 50.0)));
    }
}
