use mango_core::{EdgeSizes, Rect};

/// Writing mode orientations (CSS Writing Modes Level 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

/// Text and inline flow direction (CSS 2.1 §9.3.2 / CSS Writing Modes 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// Flow-relative logical edge sizes conforming to CSS Logical Properties and Values Level 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalEdgeSizes {
    pub inline_start: f32,
    pub inline_end: f32,
    pub block_start: f32,
    pub block_end: f32,
}

impl LogicalEdgeSizes {
    pub const ZERO: Self = Self {
        inline_start: 0.0,
        inline_end: 0.0,
        block_start: 0.0,
        block_end: 0.0,
    };

    pub fn new(inline_start: f32, inline_end: f32, block_start: f32, block_end: f32) -> Self {
        Self {
            inline_start,
            inline_end,
            block_start,
            block_end,
        }
    }

    /// Converts physical [`EdgeSizes`] to flow-relative [`LogicalEdgeSizes`].
    pub fn from_physical(physical: &EdgeSizes, mode: WritingMode, dir: Direction) -> Self {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.left,
                    inline_end: physical.right,
                    block_start: physical.top,
                    block_end: physical.bottom,
                },
                Direction::Rtl => Self {
                    inline_start: physical.right,
                    inline_end: physical.left,
                    block_start: physical.top,
                    block_end: physical.bottom,
                },
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.top,
                    inline_end: physical.bottom,
                    block_start: physical.right,
                    block_end: physical.left,
                },
                Direction::Rtl => Self {
                    inline_start: physical.bottom,
                    inline_end: physical.top,
                    block_start: physical.right,
                    block_end: physical.left,
                },
            },
            WritingMode::VerticalLr => match dir {
                Direction::Ltr => Self {
                    inline_start: physical.top,
                    inline_end: physical.bottom,
                    block_start: physical.left,
                    block_end: physical.right,
                },
                Direction::Rtl => Self {
                    inline_start: physical.bottom,
                    inline_end: physical.top,
                    block_start: physical.left,
                    block_end: physical.right,
                },
            },
        }
    }

    /// Converts flow-relative [`LogicalEdgeSizes`] to physical [`EdgeSizes`].
    pub fn to_physical(&self, mode: WritingMode, dir: Direction) -> EdgeSizes {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.block_start,
                    self.inline_end,
                    self.block_end,
                    self.inline_start,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.block_start,
                    self.inline_start,
                    self.block_end,
                    self.inline_end,
                ),
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.inline_start,
                    self.block_start,
                    self.inline_end,
                    self.block_end,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.inline_end,
                    self.block_start,
                    self.inline_start,
                    self.block_end,
                ),
            },
            WritingMode::VerticalLr => match dir {
                Direction::Ltr => EdgeSizes::new(
                    self.inline_start,
                    self.block_end,
                    self.inline_end,
                    self.block_start,
                ),
                Direction::Rtl => EdgeSizes::new(
                    self.inline_end,
                    self.block_end,
                    self.inline_start,
                    self.block_start,
                ),
            },
        }
    }

    #[inline]
    pub fn inline_sum(&self) -> f32 {
        self.inline_start + self.inline_end
    }

    #[inline]
    pub fn block_sum(&self) -> f32 {
        self.block_start + self.block_end
    }
}

/// Flow-relative rectangle in writing-mode coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalRect {
    pub inline_offset: f32,
    pub block_offset: f32,
    pub inline_size: f32,
    pub block_size: f32,
}

impl LogicalRect {
    pub fn new(inline_offset: f32, block_offset: f32, inline_size: f32, block_size: f32) -> Self {
        Self {
            inline_offset,
            block_offset,
            inline_size,
            block_size,
        }
    }

    /// Converts physical [`Rect`] within a container of given dimensions to [`LogicalRect`].
    pub fn from_physical(
        rect: &Rect,
        container_w: f32,
        _container_h: f32,
        mode: WritingMode,
        dir: Direction,
    ) -> Self {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Self {
                    inline_offset: rect.x(),
                    block_offset: rect.y(),
                    inline_size: rect.width(),
                    block_size: rect.height(),
                },
                Direction::Rtl => Self {
                    inline_offset: container_w - rect.right(),
                    block_offset: rect.y(),
                    inline_size: rect.width(),
                    block_size: rect.height(),
                },
            },
            WritingMode::VerticalRl => match dir {
                Direction::Ltr => Self {
                    inline_offset: rect.y(),
                    block_offset: container_w - rect.right(),
                    inline_size: rect.height(),
                    block_size: rect.width(),
                },
                Direction::Rtl => Self {
                    inline_offset: rect.y(),
                    block_offset: container_w - rect.right(),
                    inline_size: rect.height(),
                    block_size: rect.width(),
                },
            },
            WritingMode::VerticalLr => Self {
                inline_offset: rect.y(),
                block_offset: rect.x(),
                inline_size: rect.height(),
                block_size: rect.width(),
            },
        }
    }

    /// Converts [`LogicalRect`] back into physical [`Rect`].
    pub fn to_physical(
        &self,
        container_w: f32,
        _container_h: f32,
        mode: WritingMode,
        dir: Direction,
    ) -> Rect {
        match mode {
            WritingMode::HorizontalTb => match dir {
                Direction::Ltr => Rect::new(
                    self.inline_offset,
                    self.block_offset,
                    self.inline_size,
                    self.block_size,
                ),
                Direction::Rtl => Rect::new(
                    container_w - self.inline_offset - self.inline_size,
                    self.block_offset,
                    self.inline_size,
                    self.block_size,
                ),
            },
            WritingMode::VerticalRl => Rect::new(
                container_w - self.block_offset - self.block_size,
                self.inline_offset,
                self.block_size,
                self.inline_size,
            ),
            WritingMode::VerticalLr => Rect::new(
                self.block_offset,
                self.inline_offset,
                self.block_size,
                self.inline_size,
            ),
        }
    }
}
