use super::atom::{InlineAtom, InlineAtomKind};

/// Measures the width of a string in pixels using `fontdue` proportional advance widths with default styling.
pub fn measure_text_width(text: &str, font_size: f32) -> f32 {
    let fm = mango_render::font::font_manager();
    let (width, _height) = fm.measure_text(
        text,
        font_size,
        mango_render::FontWeight::Regular,
        mango_render::FontFamily::SansSerif,
    );
    width
}

/// Measures text width using explicit font size, weight, and family.
pub fn measure_text_width_with_style(
    text: &str,
    font_size: f32,
    weight: mango_render::FontWeight,
    family: mango_render::FontFamily,
) -> f32 {
    measure_text_width_with_style_and_spacing(text, font_size, weight, family, 0.0)
}

/// Measures text width using explicit font size, weight, family, and letter spacing.
pub fn measure_text_width_with_style_and_spacing(
    text: &str,
    font_size: f32,
    weight: mango_render::FontWeight,
    family: mango_render::FontFamily,
    letter_spacing: f32,
) -> f32 {
    let fm = mango_render::font::font_manager();
    let (width, _height) =
        fm.measure_text_with_spacing(text, font_size, weight, family, letter_spacing);
    width
}

/// Computes the balanced line width according to CSS Text 4 §7 (text-wrap: balance).
/// Balances lines across inline flow by finding the line width that minimizes variance.
pub(crate) fn compute_balanced_line_width(
    atoms: &[InlineAtom],
    container_width: f32,
) -> Option<f32> {
    if atoms.is_empty() || container_width <= 0.0 {
        return None;
    }
    let mut total_w = 0.0f32;
    let mut max_atom_w = 0.0f32;
    for atom in atoms {
        if matches!(atom.kind, InlineAtomKind::LineBreak { .. }) {
            return None;
        }
        total_w += atom.width;
        if atom.width > max_atom_w {
            max_atom_w = atom.width;
        }
    }
    if total_w <= container_width {
        return None;
    }
    let target_lines = ((total_w / container_width).ceil() as usize).max(1);
    if target_lines > 10 {
        return None;
    }

    let simulate = |w: f32| -> Vec<f32> {
        let mut lines = Vec::new();
        let mut cur = 0.0f32;
        for a in atoms {
            if cur > 0.0 && cur + a.width > w {
                lines.push(cur);
                cur = a.width;
            } else {
                cur += a.width;
            }
        }
        if cur > 0.0 {
            lines.push(cur);
        }
        lines
    };

    let variance = |lines: &[f32]| -> f32 {
        if lines.is_empty() {
            return 0.0;
        }
        let mean = lines.iter().sum::<f32>() / lines.len() as f32;
        lines.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / lines.len() as f32
    };

    let mut low = max_atom_w;
    let mut high = container_width;
    let mut best_w = container_width;
    let mut best_var = f32::MAX;

    for _ in 0..20 {
        let mid = (low + high) * 0.5;
        let lines = simulate(mid);
        if lines.len() <= target_lines {
            let var = variance(&lines);
            if var < best_var {
                best_var = var;
                best_w = mid;
            }
            high = mid;
        } else {
            low = mid;
        }
    }
    Some(best_w)
}
