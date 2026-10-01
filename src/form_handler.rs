//! Form handler and interactive picker subsystem for Mango Browser (OPT-001).
//!
//! Provides overlay management, geometry calculation, hit-testing, and rendering
//! for `<select>` dropdowns and specialized pickers (`color`, `date`, `time`, `file`).

use std::path::{Path, PathBuf};

use mango_core::{Color, Rect};
use mango_render::display_list::{BorderWidths, DisplayCommand};
use mango_render::font::{FontFamily, FontWeight};

/// Rows visible at once in the `<input type="file">` picker listing.
pub const PICKER_FILE_ROWS: usize = 10;

// Picker overlay geometry — shared between hit testing and painting.
pub const PICKER_SWATCH_CELL: f32 = 22.0;
pub const PICKER_SWATCH_GAP: f32 = 4.0;
pub const PICKER_SWATCH_PAD: f32 = 8.0;
pub const PICKER_SWATCH_COLS: usize = 7;
pub const PICKER_SWATCH_ROWS: usize = 4;
pub const PICKER_DATE_HEADER_Y: f32 = 6.0;
pub const PICKER_DATE_GRID_Y: f32 = 50.0;
pub const PICKER_DATE_ROW_H: f32 = 22.0;
pub const PICKER_DATE_PAD: f32 = 8.0;
pub const PICKER_TIME_HOUR_Y: f32 = 40.0;
pub const PICKER_TIME_MIN_Y: f32 = 74.0;
pub const PICKER_TIME_BTN_W: f32 = 34.0;
pub const PICKER_TIME_BTN_H: f32 = 24.0;
pub const PICKER_TIME_SET_Y: f32 = 106.0;
pub const PICKER_TIME_SET_W: f32 = 76.0;
pub const PICKER_TIME_SET_H: f32 = 26.0;
pub const PICKER_FILE_ROW_Y: f32 = 26.0;
pub const PICKER_FILE_ROW_H: f32 = 20.0;

/// An option within an open `<select>` dropdown menu.
#[derive(Debug, Clone)]
pub struct SelectDropdownOption {
    pub node_id: mango_html::dom::NodeId,
    pub text: String,
    pub value: String,
    pub selected: bool,
}

/// The active popup select dropdown menu.
#[derive(Debug, Clone)]
pub struct SelectDropdown {
    pub select_node_id: mango_html::dom::NodeId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub item_height: f32,
    pub options: Vec<SelectDropdownOption>,
    pub hovered_idx: Option<usize>,
}

impl SelectDropdown {
    /// Tests whether coordinates are within dropdown boundaries.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let total_h = (self.options.len() as f32 * self.item_height + 8.0).min(300.0);
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + total_h
    }

    /// Renders the select dropdown popup overlay.
    pub fn render(&self) -> Vec<DisplayCommand> {
        let mut dl = Vec::new();
        let total_h = (self.options.len() as f32 * self.item_height + 8.0).min(300.0);
        let drop_rect = Rect::new(self.x, self.y, self.width, total_h);

        dl.push(DisplayCommand::FillRoundedRect {
            rect: drop_rect,
            color: Color::rgb(255, 255, 255),
            radii: [6.0, 6.0, 6.0, 6.0],
        });
        dl.push(DisplayCommand::DrawBorder {
            rect: drop_rect,
            color: Color::rgb(180, 185, 195),
            widths: BorderWidths {
                top: 1.0,
                right: 1.0,
                bottom: 1.0,
                left: 1.0,
            },
            radii: [6.0; 4],
        });

        for (i, opt) in self.options.iter().enumerate() {
            let item_y = self.y + 4.0 + (i as f32 * self.item_height);
            if item_y + self.item_height > self.y + total_h {
                break;
            }
            let item_rect = Rect::new(self.x + 4.0, item_y, self.width - 8.0, self.item_height - 2.0);

            if self.hovered_idx == Some(i) {
                dl.push(DisplayCommand::FillRoundedRect {
                    rect: item_rect,
                    color: Color::rgb(225, 235, 255),
                    radii: [4.0, 4.0, 4.0, 4.0],
                });
            } else if opt.selected {
                dl.push(DisplayCommand::FillRoundedRect {
                    rect: item_rect,
                    color: Color::rgb(240, 243, 250),
                    radii: [4.0, 4.0, 4.0, 4.0],
                });
            }

            let text_color = if opt.selected {
                Color::rgb(24, 90, 188)
            } else {
                Color::rgb(32, 33, 36)
            };

            dl.push(DisplayCommand::draw_text(
                opt.text.clone(),
                self.x + 12.0,
                item_y + 4.0,
                text_color,
                13.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            ));
        }

        dl
    }
}

/// One actionable region inside an open picker overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerHit {
    ColorSwatch(u32),
    PrevMonth,
    NextMonth,
    Day(u32),
    HourUp,
    HourDown,
    MinUp,
    MinDown,
    SetTime,
    FileParent,
    FileRow(u32),
}

/// A single directory row in the `<input type="file">` picker.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Which picker variant is currently open.
#[derive(Debug, Clone)]
pub enum PickerKind {
    /// `<input type="color">` swatch palette.
    Color,
    /// `<input type="date">` calendar grid (`month` is 1..=12).
    Date { year: i32, month: u32 },
    /// `<input type="time">` hour/minute spinner panel.
    Time { hour: u32, minute: u32, minute_step: u32 },
    /// `<input type="file">` directory browser.
    File {
        dir: PathBuf,
        entries: Vec<FileEntry>,
        offset: usize,
    },
}

/// The active popup picker overlay for `color`/`date`/`time`/`file` inputs.
#[derive(Debug, Clone)]
pub struct PickerOverlay {
    pub node_id: mango_html::dom::NodeId,
    pub kind: PickerKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub hover: Option<PickerHit>,
}

/// Formats a numeric control value without a trailing `.0`.
pub fn fmt_number(v: f32) -> String {
    if v.is_finite() && v.fract().abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.6}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Lists a directory's entries (directories first, then files, alphabetically),
/// capped for safety. Used by the `<input type="file">` picker.
pub fn list_directory(dir: &Path) -> Vec<FileEntry> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten().take(1000) {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push(FileEntry { name, is_dir });
            } else {
                files.push(FileEntry { name, is_dir });
            }
        }
    }
    let key = |e: &FileEntry| e.name.to_lowercase();
    dirs.sort_by_key(key);
    files.sort_by_key(key);
    dirs.extend(files);
    dirs
}

/// Returns the fixed `(width, height)` of each picker overlay variant.
pub fn picker_size(kind: &PickerKind) -> (f32, f32) {
    match kind {
        PickerKind::Color => (
            PICKER_SWATCH_PAD * 2.0
                + PICKER_SWATCH_COLS as f32 * PICKER_SWATCH_CELL
                + (PICKER_SWATCH_COLS - 1) as f32 * PICKER_SWATCH_GAP,
            PICKER_SWATCH_PAD * 2.0
                + PICKER_SWATCH_ROWS as f32 * PICKER_SWATCH_CELL
                + (PICKER_SWATCH_ROWS - 1) as f32 * PICKER_SWATCH_GAP,
        ),
        PickerKind::Date { .. } => (210.0, 186.0),
        PickerKind::Time { .. } => (150.0, 140.0),
        PickerKind::File { .. } => (300.0, 250.0),
    }
}

/// Hit-tests a point (screen coordinates) against an open picker overlay,
/// returning the action it maps to. `None` means outside the panel or an
/// inert region inside it.
pub fn picker_hit_test(picker: &PickerOverlay, x: f32, y: f32) -> Option<PickerHit> {
    if x < picker.x
        || x > picker.x + picker.width
        || y < picker.y
        || y > picker.y + picker.height
    {
        return None;
    }
    let rx = x - picker.x;
    let ry = y - picker.y;

    match &picker.kind {
        PickerKind::Color => {
            let col = (rx - PICKER_SWATCH_PAD) / (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP);
            let row = (ry - PICKER_SWATCH_PAD) / (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP);
            if col >= 0.0
                && col < PICKER_SWATCH_COLS as f32
                && row >= 0.0
                && row < PICKER_SWATCH_ROWS as f32
            {
                let col_i = col as usize;
                let row_i = row as usize;
                let cx =
                    PICKER_SWATCH_PAD + col_i as f32 * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP);
                let cy =
                    PICKER_SWATCH_PAD + row_i as f32 * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP);
                if rx - cx <= PICKER_SWATCH_CELL && ry - cy <= PICKER_SWATCH_CELL {
                    return Some(PickerHit::ColorSwatch(
                        (row_i * PICKER_SWATCH_COLS + col_i) as u32,
                    ));
                }
            }
            None
        }
        PickerKind::Date { year, month } => {
            if ry >= PICKER_DATE_HEADER_Y && ry <= PICKER_DATE_HEADER_Y + 22.0 {
                if rx < 34.0 {
                    return Some(PickerHit::PrevMonth);
                }
                if rx > picker.width - 34.0 {
                    return Some(PickerHit::NextMonth);
                }
                return None;
            }
            let col_w = (picker.width - PICKER_DATE_PAD * 2.0) / 7.0;
            if rx >= PICKER_DATE_PAD
                && rx < picker.width - PICKER_DATE_PAD
                && ry >= PICKER_DATE_GRID_Y
                && ry < PICKER_DATE_GRID_Y + 6.0 * PICKER_DATE_ROW_H
            {
                let col = ((rx - PICKER_DATE_PAD) / col_w) as i64;
                let row = ((ry - PICKER_DATE_GRID_Y) / PICKER_DATE_ROW_H) as i64;
                if col < 7 && row < 6 {
                    let lead = weekday_sunday0(days_from_civil(*year, *month, 1));
                    let day_idx = row * 7 + col - lead;
                    if (0..=30).contains(&day_idx) {
                        let day = (day_idx + 1) as u32;
                        if day <= days_in_month(*year, *month) {
                            return Some(PickerHit::Day(day));
                        }
                    }
                }
            }
            None
        }
        PickerKind::Time { .. } => {
            if ry >= PICKER_TIME_HOUR_Y && ry < PICKER_TIME_HOUR_Y + PICKER_TIME_BTN_H {
                if rx >= 10.0 && rx < 10.0 + PICKER_TIME_BTN_W {
                    return Some(PickerHit::HourDown);
                }
                if rx >= picker.width - 10.0 - PICKER_TIME_BTN_W && rx < picker.width - 10.0 {
                    return Some(PickerHit::HourUp);
                }
            } else if ry >= PICKER_TIME_MIN_Y && ry < PICKER_TIME_MIN_Y + PICKER_TIME_BTN_H {
                if rx >= 10.0 && rx < 10.0 + PICKER_TIME_BTN_W {
                    return Some(PickerHit::MinDown);
                }
                if rx >= picker.width - 10.0 - PICKER_TIME_BTN_W && rx < picker.width - 10.0 {
                    return Some(PickerHit::MinUp);
                }
            } else if ry >= PICKER_TIME_SET_Y && ry < PICKER_TIME_SET_Y + PICKER_TIME_SET_H {
                let sx = (picker.width - PICKER_TIME_SET_W) / 2.0;
                if rx >= sx && rx < sx + PICKER_TIME_SET_W {
                    return Some(PickerHit::SetTime);
                }
            }
            None
        }
        PickerKind::File {
            dir,
            entries,
            offset,
        } => {
            if ry >= PICKER_FILE_ROW_Y
                && ry < PICKER_FILE_ROW_Y + PICKER_FILE_ROWS as f32 * PICKER_FILE_ROW_H
                && rx >= 4.0
                && rx <= picker.width - 4.0
            {
                let row = ((ry - PICKER_FILE_ROW_Y) / PICKER_FILE_ROW_H) as usize;
                let has_parent = dir.parent().is_some();
                if has_parent && row == 0 {
                    return Some(PickerHit::FileParent);
                }
                let entry_row = row - usize::from(has_parent);
                if entry_row < entries.len() {
                    return Some(PickerHit::FileRow((offset + entry_row) as u32));
                }
            }
            None
        }
    }
}

/// Days since 1970-01-01 (UTC) for the current date.
pub fn unix_days_now() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

/// Converts days-since-1970 to `(year, month, day)` (Howard Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Inverse of [`civil_from_days`]: `(year, month, day)` → days since 1970-01-01.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = y as i64 - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// Days in the given `(year, month)` (month is 1..=12), honoring leap years.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Weekday for a day count since 1970-01-01, where 0 = Sunday
/// (1970-01-01 itself was a Thursday).
pub fn weekday_sunday0(days: i64) -> i64 {
    ((days + 4) % 7 + 7) % 7
}

/// Parses a `YYYY-MM-DD` date value.
pub fn parse_date_value(v: &str) -> Option<(i32, u32, u32)> {
    let mut parts = v.trim().split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if (1..=12).contains(&m) && d >= 1 && d <= 31 && (1..=9999).contains(&y) {
        Some((y, m, d))
    } else {
        None
    }
}

/// Parses an `HH:MM` (or `HH:MM:SS`) time value.
pub fn parse_time_value(v: &str) -> Option<(u32, u32)> {
    let mut parts = v.trim().split(':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    if h < 24 && m < 60 {
        Some((h, m))
    } else {
        None
    }
}

/// English month name for the date picker header (month is 1..=12).
pub fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_and_time_parsing() {
        assert_eq!(parse_date_value("2026-03-26"), Some((2026, 3, 26)));
        assert_eq!(parse_date_value("invalid"), None);

        assert_eq!(parse_time_value("14:30"), Some((14, 30)));
        assert_eq!(parse_time_value("25:00"), None);
    }

    #[test]
    fn test_civil_calendar_conversions() {
        let days = days_from_civil(2026, 3, 26);
        let civil = civil_from_days(days);
        assert_eq!(civil, (2026, 3, 26));

        assert_eq!(days_in_month(2024, 2), 29); // Leap year
        assert_eq!(days_in_month(2026, 2), 28); // Non-leap year
    }

    #[test]
    fn test_picker_dimensions_and_swatch_hit() {
        let kind = PickerKind::Color;
        let (w, h) = picker_size(&kind);
        assert!(w > 0.0 && h > 0.0);

        let picker = PickerOverlay {
            node_id: mango_html::dom::NodeId::from_raw(1),
            kind,
            x: 50.0,
            y: 50.0,
            width: w,
            height: h,
            hover: None,
        };

        let hit = picker_hit_test(&picker, 50.0 + PICKER_SWATCH_PAD + 2.0, 50.0 + PICKER_SWATCH_PAD + 2.0);
        assert_eq!(hit, Some(PickerHit::ColorSwatch(0)));
    }
}
