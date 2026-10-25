use std::sync::Arc;

use mango_css::computed::ComputedStyle;

use crate::box_tree::LayoutBox;

/// An atomic inline fragment kind: either text (word/space), an atomic box (inline-block/replaced), or a forced line break.
#[derive(Debug, Clone)]
pub enum InlineAtomKind {
    Text { text: String, is_space: bool },
    AtomicBox(Box<LayoutBox>),
    OutOfFlow(Box<LayoutBox>),
    LineBreak,
    WordBreakOpportunity,
    SoftHyphen,
    Spacing,
}

/// An atomic inline fragment (a word, a whitespace gap, or an inline element box).
#[derive(Debug, Clone)]
pub struct InlineAtom {
    pub kind: InlineAtomKind,
    pub style: Arc<ComputedStyle>,
    pub width: f32,
    pub height: f32,
    pub link_target: Option<String>,
    pub link_frame: Option<String>,
    pub owner_inline_id: Option<mango_html::dom::NodeId>,
}

impl InlineAtom {
    pub fn is_space(&self) -> bool {
        match &self.kind {
            InlineAtomKind::Text { is_space, .. } => *is_space,
            InlineAtomKind::AtomicBox(_)
            | InlineAtomKind::OutOfFlow(_)
            | InlineAtomKind::LineBreak
            | InlineAtomKind::WordBreakOpportunity
            | InlineAtomKind::SoftHyphen
            | InlineAtomKind::Spacing => false,
        }
    }
}
