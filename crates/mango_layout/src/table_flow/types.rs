use mango_core::Color;
use mango_css::values::{BorderStyle, Display};

use crate::box_tree::LayoutBox;

#[derive(Debug, Clone, Copy)]
pub struct CellRef {
    pub group_idx: Option<usize>,
    pub row_idx: usize,
    pub cell_idx: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub row_span: usize,
    pub col_span: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderStyle,
    #[allow(dead_code)]
    pub color: Color,
    pub precedence: u8, // 4 = cell, 3 = row, 2 = rowgroup, 1 = table
}

impl Default for BorderSide {
    fn default() -> Self {
        Self {
            width: 0.0,
            style: BorderStyle::None,
            color: Color::TRANSPARENT,
            precedence: 0,
        }
    }
}

pub fn style_priority(s: BorderStyle) -> u8 {
    match s {
        BorderStyle::Double => 8,
        BorderStyle::Solid => 7,
        BorderStyle::Dashed => 6,
        BorderStyle::Dotted => 5,
        BorderStyle::Ridge => 4,
        BorderStyle::Outset => 3,
        BorderStyle::Groove => 2,
        BorderStyle::Inset => 1,
        BorderStyle::None | BorderStyle::Hidden => 0,
    }
}

/// Resolves conflicting borders per CSS 2.1 §17.6.2.1.
pub fn resolve_border_conflict(a: BorderSide, b: BorderSide) -> BorderSide {
    // 1. Hidden wins unconditionally and suppresses border
    if a.style == BorderStyle::Hidden || b.style == BorderStyle::Hidden {
        return BorderSide {
            width: 0.0,
            style: BorderStyle::Hidden,
            color: Color::TRANSPARENT,
            precedence: 255,
        };
    }

    // 2. None has the lowest priority
    if a.style == BorderStyle::None && b.style != BorderStyle::None {
        return b;
    }
    if b.style == BorderStyle::None && a.style != BorderStyle::None {
        return a;
    }

    // 3. Compare widths (larger width wins)
    if (a.width - b.width).abs() > 0.1 {
        return if a.width > b.width { a } else { b };
    }

    // 4. Compare style priority
    let a_prio = style_priority(a.style);
    let b_prio = style_priority(b.style);
    if a_prio != b_prio {
        return if a_prio > b_prio { a } else { b };
    }

    // 5. Compare element precedence (cell > row > rowgroup > table)
    if a.precedence != b.precedence {
        return if a.precedence > b.precedence { a } else { b };
    }

    // 6. Tie-breaker: prefer first side
    a
}

pub fn is_table_row(box_node: &LayoutBox) -> bool {
    box_node.style.as_ref().map(|s| s.display) == Some(Display::TableRow)
        || box_node.tag_name.as_deref() == Some("tr")
}

pub fn is_table_row_group(box_node: &LayoutBox) -> bool {
    matches!(
        box_node.tag_name.as_deref(),
        Some("thead") | Some("tbody") | Some("tfoot")
    ) || matches!(
        box_node.style.as_ref().map(|s| s.display),
        Some(Display::TableRowGroup)
            | Some(Display::TableHeaderGroup)
            | Some(Display::TableFooterGroup)
    )
}

pub fn is_table_cell(box_node: &LayoutBox) -> bool {
    box_node.style.as_ref().map(|s| s.display) == Some(Display::TableCell)
        || matches!(box_node.tag_name.as_deref(), Some("td") | Some("th"))
}

pub fn is_table_caption(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("caption")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableCaption)
}

pub fn is_table_col(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("col")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableColumn)
}

pub fn is_table_colgroup(box_node: &LayoutBox) -> bool {
    box_node.tag_name.as_deref() == Some("colgroup")
        || box_node.style.as_ref().map(|s| s.display) == Some(Display::TableColumnGroup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::Color;
    use mango_css::values::BorderStyle;

    #[test]
    fn test_style_priority() {
        assert_eq!(style_priority(BorderStyle::Double), 8);
        assert_eq!(style_priority(BorderStyle::Solid), 7);
        assert_eq!(style_priority(BorderStyle::Dashed), 6);
        assert_eq!(style_priority(BorderStyle::Dotted), 5);
        assert_eq!(style_priority(BorderStyle::Ridge), 4);
        assert_eq!(style_priority(BorderStyle::Outset), 3);
        assert_eq!(style_priority(BorderStyle::Groove), 2);
        assert_eq!(style_priority(BorderStyle::Inset), 1);
        assert_eq!(style_priority(BorderStyle::None), 0);
        assert_eq!(style_priority(BorderStyle::Hidden), 0);
    }

    #[test]
    fn test_resolve_border_conflict_hidden() {
        let a = BorderSide {
            width: 1.0,
            style: BorderStyle::Solid,
            color: Color::RED,
            precedence: 1,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Hidden,
            color: Color::BLUE,
            precedence: 2,
        };

        let result = resolve_border_conflict(a, b);
        assert_eq!(result.style, BorderStyle::Hidden);
        assert_eq!(result.width, 0.0);
        assert_eq!(result.precedence, 255);
        assert_eq!(result.color, Color::TRANSPARENT);
    }

    #[test]
    fn test_resolve_border_conflict_none() {
        let a = BorderSide {
            width: 1.0,
            style: BorderStyle::None,
            color: Color::RED,
            precedence: 1,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::BLUE,
            precedence: 2,
        };

        let result1 = resolve_border_conflict(a, b);
        assert_eq!(result1.style, BorderStyle::Solid);
        assert_eq!(result1.width, 2.0);

        let result2 = resolve_border_conflict(b, a);
        assert_eq!(result2.style, BorderStyle::Solid);
        assert_eq!(result2.width, 2.0);
    }

    #[test]
    fn test_resolve_border_conflict_width() {
        let a = BorderSide {
            width: 3.0,
            style: BorderStyle::Solid,
            color: Color::RED,
            precedence: 1,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Double,
            color: Color::BLUE,
            precedence: 2,
        };

        let result = resolve_border_conflict(a, b);
        assert_eq!(result.width, 3.0);
        assert_eq!(result.style, BorderStyle::Solid);
    }

    #[test]
    fn test_resolve_border_conflict_style_priority() {
        let a = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::RED,
            precedence: 1,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Double,
            color: Color::BLUE,
            precedence: 2,
        };

        let result = resolve_border_conflict(a, b);
        assert_eq!(result.style, BorderStyle::Double);
    }

    #[test]
    fn test_resolve_border_conflict_precedence() {
        let a = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::RED,
            precedence: 3,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::BLUE,
            precedence: 2,
        };

        let result = resolve_border_conflict(a, b);
        assert_eq!(result.precedence, 3);
        assert_eq!(result.color, Color::RED);
    }

    #[test]
    fn test_resolve_border_conflict_tie_breaker() {
        let a = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::RED,
            precedence: 2,
        };
        let b = BorderSide {
            width: 2.0,
            style: BorderStyle::Solid,
            color: Color::BLUE,
            precedence: 2,
        };

        let result = resolve_border_conflict(a, b);
        assert_eq!(result.color, Color::RED);
    }
}
