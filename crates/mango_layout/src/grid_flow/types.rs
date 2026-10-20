#[derive(Debug, Clone, Copy)]
pub struct PlacedItem {
    pub child_idx: usize,
    pub col_start: usize,
    pub col_span: usize,
    pub row_start: usize,
    pub row_span: usize,
}
