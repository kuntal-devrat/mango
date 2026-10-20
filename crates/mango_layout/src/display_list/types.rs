use mango_core::{Color, Rect};
use mango_render::display_list::DisplayList;

use crate::box_tree::LayoutBox;

use super::builder::build_display_list_with_scroll_xy;

/// A cached display list record with dirty-tracking and scroll offsets.
#[derive(Debug, Clone, Default)]
pub struct DisplayListCache {
    cached_scroll_x: Option<f32>,
    cached_scroll_y: Option<f32>,
    cached_list: Option<DisplayList>,
    dirty_rects: Vec<Rect>,
}

impl DisplayListCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the cached display list if valid, or builds and caches it.
    pub fn get_or_build(&mut self, root: &LayoutBox, scroll_y: f32) -> DisplayList {
        self.get_or_build_xy(root, 0.0, scroll_y)
    }

    /// Returns the cached display list for horizontal and vertical scroll offsets.
    pub fn get_or_build_xy(
        &mut self,
        root: &LayoutBox,
        scroll_x: f32,
        scroll_y: f32,
    ) -> DisplayList {
        if !root.is_dirty
            && self.cached_scroll_x == Some(scroll_x)
            && self.cached_scroll_y == Some(scroll_y)
            && let Some(ref list) = self.cached_list
        {
            return list.clone();
        }

        let list = build_display_list_with_scroll_xy(root, scroll_x, scroll_y);
        self.cached_scroll_x = Some(scroll_x);
        self.cached_scroll_y = Some(scroll_y);
        self.cached_list = Some(list.clone());
        self.dirty_rects.clear();
        list
    }

    /// Invalidate the display list cache with an optional dirty region.
    pub fn invalidate(&mut self, region: Option<Rect>) {
        self.cached_list = None;
        if let Some(r) = region {
            self.dirty_rects.push(r);
        }
    }

    /// Clears the cached display list completely.
    pub fn clear(&mut self) {
        self.invalidate(None);
    }

    /// Returns the list of dirty rects accumulated since last build.
    pub fn dirty_regions(&self) -> &[Rect] {
        &self.dirty_rects
    }

    /// Returns true if a cached display list is currently available.
    pub fn is_cached(&self) -> bool {
        self.cached_list.is_some()
    }

    /// Updates the cached display list with a newly built one and computes the diff.
    ///
    /// If an old display list was cached, this returns a `DisplayListDiff` with the minimal
    /// changed commands and consolidated damage rect. Otherwise, it treats the entire new list
    /// as freshly inserted.
    pub fn update_and_diff(
        &mut self,
        new_list: DisplayList,
        scroll_y: f32,
    ) -> mango_render::DisplayListDiff {
        self.update_and_diff_xy(new_list, 0.0, scroll_y)
    }

    /// Updates the cached display list with horizontal and vertical scroll offsets and computes the diff.
    pub fn update_and_diff_xy(
        &mut self,
        new_list: DisplayList,
        scroll_x: f32,
        scroll_y: f32,
    ) -> mango_render::DisplayListDiff {
        let diff = if let Some(ref old_list) = self.cached_list {
            old_list.diff(&new_list)
        } else {
            let empty = DisplayList::new();
            empty.diff(&new_list)
        };

        if let Some(damage) = diff.damage_rect() {
            self.dirty_rects.push(damage);
        }

        self.cached_scroll_x = Some(scroll_x);
        self.cached_scroll_y = Some(scroll_y);
        self.cached_list = Some(new_list);
        diff
    }
}

pub(crate) fn apply_opacity(color: Color, opacity: f32) -> Color {
    if opacity >= 1.0 {
        color
    } else {
        let alpha = ((color.a as f32) * opacity.clamp(0.0, 1.0)).round() as u8;
        Color::rgba(color.r, color.g, color.b, alpha)
    }
}

/// Generates a crisp, antialiased magnifying glass search icon in 0xAARRGGBB format.
pub(crate) fn generate_search_icon(size: u32, color: Color) -> Vec<u32> {
    let mut pixels = vec![0u32; (size * size) as usize];
    let cx = size as f32 * 0.40;
    let cy = size as f32 * 0.40;
    let r_out = size as f32 * 0.26;
    let r_in = (size as f32 * 0.16).max(r_out - 2.0);
    let handle_width = (size as f32 * 0.12).max(1.8);

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = px - cx;
            let dy = py - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            // Circular lens rim
            let mut alpha = 0.0f32;
            if dist >= r_in - 0.5 && dist <= r_out + 0.5 {
                let a1 = (dist - (r_in - 0.5)).clamp(0.0, 1.0);
                let a2 = ((r_out + 0.5) - dist).clamp(0.0, 1.0);
                alpha = alpha.max(a1.min(a2));
            }

            // Diagonal handle
            let hx_start = cx + r_out * 0.70;
            let hy_start = cy + r_out * 0.70;
            let hx_end = size as f32 * 0.82;
            let hy_end = size as f32 * 0.82;
            let seg_dx = hx_end - hx_start;
            let seg_dy = hy_end - hy_start;
            let seg_len_sq = seg_dx * seg_dx + seg_dy * seg_dy;
            if seg_len_sq > 0.0 {
                let t = (((px - hx_start) * seg_dx + (py - hy_start) * seg_dy) / seg_len_sq)
                    .clamp(0.0, 1.0);
                let proj_x = hx_start + t * seg_dx;
                let proj_y = hy_start + t * seg_dy;
                let h_dist = ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt();
                let h_alpha = ((handle_width * 0.5 + 0.5) - h_dist).clamp(0.0, 1.0);
                alpha = alpha.max(h_alpha);
            }

            if alpha > 0.0 {
                let a = ((color.a as f32) * alpha).round() as u32;
                pixels[(y * size + x) as usize] = (a << 24)
                    | ((color.r as u32) << 16)
                    | ((color.g as u32) << 8)
                    | (color.b as u32);
            }
        }
    }
    pixels
}

/// Composes two 2D affine matrices: `outer * inner` (apply `inner` first).
pub(crate) fn mul_matrix(outer: [f32; 6], inner: [f32; 6]) -> [f32; 6] {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Resolves a `transform-origin` component against the element's box size.
pub(crate) fn resolve_origin(length: mango_css::values::Length, size: f32) -> f32 {
    match length {
        mango_css::values::Length::Px(px) => px,
        mango_css::values::Length::Percent(pct) => size * pct / 100.0,
        mango_css::values::Length::Em(em) => em * 16.0,
        mango_css::values::Length::Rem(rem) => rem * 16.0,
        _ => 0.0,
    }
}

pub const COLOR_SWATCHES: [&str; 28] = [
    "#000000", "#333333", "#666666", "#999999", "#cccccc", "#ffffff", "#ff0000", "#ff4040",
    "#ff8000", "#ffb300", "#ffff00", "#d4ff00", "#00e000", "#00ff80", "#00ffff", "#0080ff",
    "#0040ff", "#4000ff", "#8000ff", "#ff00ff", "#ff0080", "#8b4513", "#daa520", "#008080",
    "#2f4f4f", "#800000", "#000080", "#4b0082",
];

pub(crate) fn format_media_time(seconds: f32) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total_secs = seconds as u32;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{}:{:02}", mins, secs)
}

pub(crate) fn lookup_image_cached(url: &str) -> Option<mango_render::image_decode::DecodedImage> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(img) = mango_render::image_decode::decode_data_uri(trimmed) {
        return Some(img);
    }
    if let Some(img) = mango_render::image_decode::get_cached_image(trimmed) {
        return Some(img);
    }
    if trimmed.starts_with("//") {
        let https_url = format!("https:{trimmed}");
        if let Some(img) = mango_render::image_decode::get_cached_image(&https_url) {
            return Some(img);
        }
    }
    None
}
