//! Popup and overlay renderers: context menus, select dropdowns, form pickers, validation bubbles.

use mango_core::{Color, Rect};
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};
use mango_render::{FontFamily, FontWeight};

use super::super::BrowserChrome;
use super::super::forms::truncate_label;
use crate::form_handler::*;

impl BrowserChrome {
    /// Renders floating and modal chrome overlays:
    /// context menus, select dropdowns, pickers (color, date, time, file), and validation bubbles.
    pub(crate) fn render_chrome_overlays(&self, dl: &mut DisplayList) {
        // ── CONTEXT MENU POPUP ──
        if let Some(menu) = &self.context_menu {
            let menu_rect = Rect::new(menu.x, menu.y, menu.width, menu.height);
            dl.push(DisplayCommand::FillRoundedRect {
                rect: menu_rect,
                color: Color::rgb(41, 42, 45),
                radii: [6.0, 6.0, 6.0, 6.0],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: menu_rect,
                color: Color::rgb(70, 72, 77),
                widths: BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                radii: [6.0; 4],
            });

            for (i, item) in menu.items.iter().enumerate() {
                let item_y = menu.y + 4.0 + (i as f32 * 26.0);
                let item_rect = Rect::new(menu.x + 4.0, item_y, menu.width - 8.0, 24.0);

                if menu.hovered_idx == Some(i) && item.enabled {
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: item_rect,
                        color: Color::rgb(70, 72, 77),
                        radii: [4.0, 4.0, 4.0, 4.0],
                    });
                }

                let text_color = if item.enabled {
                    Color::rgb(232, 234, 237)
                } else {
                    Color::rgb(112, 115, 120)
                };

                dl.push(DisplayCommand::draw_text(
                    item.label.clone(),
                    menu.x + 12.0,
                    item_y + 4.0,
                    text_color,
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                ));
            }
        }

        // ── SELECT DROPDOWN POPUP OVERLAY ──
        if let Some(dropdown) = &self.select_dropdown {
            let total_h = (dropdown.options.len() as f32 * dropdown.item_height + 8.0).min(300.0);
            let drop_rect = Rect::new(dropdown.x, dropdown.y, dropdown.width, total_h);
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

            for (i, opt) in dropdown.options.iter().enumerate() {
                let item_y = dropdown.y + 4.0 + (i as f32 * dropdown.item_height);
                if item_y + dropdown.item_height > dropdown.y + total_h {
                    break;
                }
                let item_rect = Rect::new(
                    dropdown.x + 4.0,
                    item_y,
                    dropdown.width - 8.0,
                    dropdown.item_height - 2.0,
                );

                if dropdown.hovered_idx == Some(i) {
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
                    dropdown.x + 12.0,
                    item_y + 4.0,
                    text_color,
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                ));
            }
        }

        // ── PICKER OVERLAY POPUP (color / date / time / file) ──
        if let Some(picker) = &self.picker {
            let panel = Rect::new(picker.x, picker.y, picker.width, picker.height);
            dl.push(DisplayCommand::DrawBoxShadow {
                rect: panel,
                color: Color::rgba(0, 0, 0, 70),
                offset_x: 0.0,
                offset_y: 2.0,
                blur_radius: 6.0,
                spread_radius: 0.0,
                radii: [6.0; 4],
                inset: false,
            });
            dl.push(DisplayCommand::FillRoundedRect {
                rect: panel,
                color: Color::rgb(255, 255, 255),
                radii: [6.0; 4],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: panel,
                color: Color::rgb(180, 185, 195),
                widths: BorderWidths {
                    top: 1.0,
                    right: 1.0,
                    bottom: 1.0,
                    left: 1.0,
                },
                radii: [6.0; 4],
            });

            match &picker.kind {
                PickerKind::Color => {
                    for (i, hex) in mango_layout::COLOR_SWATCHES.iter().enumerate() {
                        let col = (i % PICKER_SWATCH_COLS) as f32;
                        let row = (i / PICKER_SWATCH_COLS) as f32;
                        let rect = Rect::new(
                            picker.x
                                + PICKER_SWATCH_PAD
                                + col * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP),
                            picker.y
                                + PICKER_SWATCH_PAD
                                + row * (PICKER_SWATCH_CELL + PICKER_SWATCH_GAP),
                            PICKER_SWATCH_CELL,
                            PICKER_SWATCH_CELL,
                        );
                        let color = mango_css::values::Value::parse_color(hex)
                            .unwrap_or(Color::rgb(255, 0, 255));
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect,
                            color,
                            radii: [3.0; 4],
                        });
                        if picker.hover == Some(PickerHit::ColorSwatch(i as u32)) {
                            dl.push(DisplayCommand::DrawBorder {
                                rect,
                                color: Color::rgb(0, 120, 215),
                                widths: BorderWidths {
                                    top: 2.0,
                                    right: 2.0,
                                    bottom: 2.0,
                                    left: 2.0,
                                },
                                radii: [3.0; 4],
                            });
                        }
                    }
                }
                PickerKind::Date { year, month } => {
                    // Header: "<" / Month Year / ">" (ASCII arrows for glyph coverage).
                    let prev_rect =
                        Rect::new(picker.x + 4.0, picker.y + PICKER_DATE_HEADER_Y, 26.0, 22.0);
                    let next_rect = Rect::new(
                        picker.x + picker.width - 30.0,
                        picker.y + PICKER_DATE_HEADER_Y,
                        26.0,
                        22.0,
                    );
                    if picker.hover == Some(PickerHit::PrevMonth) {
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect: prev_rect,
                            color: Color::rgb(235, 238, 245),
                            radii: [4.0; 4],
                        });
                    }
                    if picker.hover == Some(PickerHit::NextMonth) {
                        dl.push(DisplayCommand::FillRoundedRect {
                            rect: next_rect,
                            color: Color::rgb(235, 238, 245),
                            radii: [4.0; 4],
                        });
                    }
                    let arrow_font = 14.0;
                    dl.push(DisplayCommand::draw_text(
                        "<",
                        prev_rect.x()
                            + (prev_rect.width()
                                - mango_layout::measure_text_width("<", arrow_font))
                                / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(60, 64, 67),
                        arrow_font,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                    dl.push(DisplayCommand::draw_text(
                        ">",
                        next_rect.x()
                            + (next_rect.width()
                                - mango_layout::measure_text_width(">", arrow_font))
                                / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(60, 64, 67),
                        arrow_font,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                    let title = format!("{} {}", month_name(*month), year);
                    let title_w = mango_layout::measure_text_width(&title, 13.0);
                    dl.push(DisplayCommand::draw_text(
                        &title,
                        picker.x + (picker.width - title_w) / 2.0,
                        picker.y + PICKER_DATE_HEADER_Y + 4.0,
                        Color::rgb(32, 33, 36),
                        13.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));

                    // Weekday header row.
                    let weekdays = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
                    let col_w = (picker.width - PICKER_DATE_PAD * 2.0) / 7.0;
                    for (i, wd) in weekdays.iter().enumerate() {
                        let w = mango_layout::measure_text_width(wd, 10.0);
                        dl.push(DisplayCommand::draw_text(
                            *wd,
                            picker.x + PICKER_DATE_PAD + i as f32 * col_w + (col_w - w) / 2.0,
                            picker.y + 34.0,
                            Color::rgb(120, 124, 128),
                            10.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }

                    // Day cells.
                    let lead = weekday_sunday0(days_from_civil(*year, *month, 1));
                    let dim = days_in_month(*year, *month);
                    let selected = self
                        .js_runtime
                        .as_ref()
                        .and_then(|rt| rt.get_node_attribute(picker.node_id, "value"))
                        .and_then(|v| parse_date_value(&v));
                    for day in 1..=dim {
                        let idx = lead + i64::from(day) - 1;
                        if !(0..42).contains(&idx) {
                            continue;
                        }
                        let row = idx / 7;
                        let col = idx % 7;
                        let cell = Rect::new(
                            picker.x + PICKER_DATE_PAD + col as f32 * col_w + 1.0,
                            picker.y + PICKER_DATE_GRID_Y + row as f32 * PICKER_DATE_ROW_H,
                            col_w - 2.0,
                            20.0,
                        );
                        let is_sel = selected == Some((*year, *month, day));
                        if is_sel {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect: cell,
                                color: Color::rgb(0, 120, 215),
                                radii: [4.0; 4],
                            });
                        } else if picker.hover == Some(PickerHit::Day(day)) {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect: cell,
                                color: Color::rgb(225, 235, 255),
                                radii: [4.0; 4],
                            });
                        }
                        let label = day.to_string();
                        let w = mango_layout::measure_text_width(&label, 12.0);
                        dl.push(DisplayCommand::draw_text(
                            &label,
                            cell.x() + (cell.width() - w) / 2.0,
                            cell.y() + 4.0,
                            if is_sel {
                                Color::WHITE
                            } else {
                                Color::rgb(32, 33, 36)
                            },
                            12.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }
                }
                PickerKind::Time { hour, minute, .. } => {
                    let value = format!("{:02}:{:02}", hour, minute);
                    let vw = mango_layout::measure_text_width(&value, 18.0);
                    dl.push(DisplayCommand::draw_text(
                        &value,
                        picker.x + (picker.width - vw) / 2.0,
                        picker.y + 10.0,
                        Color::rgb(32, 33, 36),
                        18.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));

                    let stepper =
                        |dl: &mut DisplayList, x: f32, y: f32, label: &str, hovered: bool| {
                            let rect = Rect::new(x, y, PICKER_TIME_BTN_W, PICKER_TIME_BTN_H);
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect,
                                color: if hovered {
                                    Color::rgb(210, 226, 245)
                                } else {
                                    Color::rgb(240, 241, 243)
                                },
                                radii: [4.0; 4],
                            });
                            let w = mango_layout::measure_text_width(label, 13.0);
                            dl.push(DisplayCommand::draw_text(
                                label,
                                x + (PICKER_TIME_BTN_W - w) / 2.0,
                                y + 5.0,
                                Color::rgb(60, 64, 67),
                                13.0,
                                FontWeight::Bold,
                                FontFamily::SansSerif,
                            ));
                        };
                    let lx = picker.x + 10.0;
                    let rx = picker.x + picker.width - 10.0 - PICKER_TIME_BTN_W;
                    stepper(
                        dl,
                        lx,
                        picker.y + PICKER_TIME_HOUR_Y,
                        "-",
                        picker.hover == Some(PickerHit::HourDown),
                    );
                    stepper(
                        dl,
                        rx,
                        picker.y + PICKER_TIME_HOUR_Y,
                        "+",
                        picker.hover == Some(PickerHit::HourUp),
                    );
                    stepper(
                        dl,
                        lx,
                        picker.y + PICKER_TIME_MIN_Y,
                        "-",
                        picker.hover == Some(PickerHit::MinDown),
                    );
                    stepper(
                        dl,
                        rx,
                        picker.y + PICKER_TIME_MIN_Y,
                        "+",
                        picker.hover == Some(PickerHit::MinUp),
                    );

                    let center_label = |dl: &mut DisplayList, y: f32, text: &str| {
                        let w = mango_layout::measure_text_width(text, 14.0);
                        dl.push(DisplayCommand::draw_text(
                            text,
                            picker.x + (picker.width - w) / 2.0,
                            y,
                            Color::rgb(32, 33, 36),
                            14.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    };
                    let hour_text = hour.to_string();
                    let minute_text = minute.to_string();
                    center_label(dl, picker.y + PICKER_TIME_HOUR_Y + 5.0, &hour_text);
                    center_label(dl, picker.y + PICKER_TIME_MIN_Y + 5.0, &minute_text);

                    let set_rect = Rect::new(
                        picker.x + (picker.width - PICKER_TIME_SET_W) / 2.0,
                        picker.y + PICKER_TIME_SET_Y,
                        PICKER_TIME_SET_W,
                        PICKER_TIME_SET_H,
                    );
                    dl.push(DisplayCommand::FillRoundedRect {
                        rect: set_rect,
                        color: if picker.hover == Some(PickerHit::SetTime) {
                            Color::rgb(0, 100, 190)
                        } else {
                            Color::rgb(0, 120, 215)
                        },
                        radii: [4.0; 4],
                    });
                    let set_w = mango_layout::measure_text_width("Set", 13.0);
                    dl.push(DisplayCommand::draw_text(
                        "Set",
                        set_rect.x() + (set_rect.width() - set_w) / 2.0,
                        picker.y + PICKER_TIME_SET_Y + 6.0,
                        Color::WHITE,
                        13.0,
                        FontWeight::Bold,
                        FontFamily::SansSerif,
                    ));
                }
                PickerKind::File {
                    dir,
                    entries,
                    offset,
                } => {
                    // Header showing the current directory path.
                    dl.push(DisplayCommand::FillRect {
                        rect: Rect::new(
                            picker.x + 1.0,
                            picker.y + 1.0,
                            picker.width - 2.0,
                            PICKER_FILE_ROW_Y - 1.0,
                        ),
                        color: Color::rgb(246, 247, 249),
                    });
                    dl.push(DisplayCommand::DrawLine {
                        x1: picker.x + 1.0,
                        y1: picker.y + PICKER_FILE_ROW_Y - 1.0,
                        x2: picker.x + picker.width - 1.0,
                        y2: picker.y + PICKER_FILE_ROW_Y - 1.0,
                        color: Color::rgb(216, 220, 226),
                        thickness: 1.0,
                    });
                    let path = truncate_label(&dir.display().to_string(), 46);
                    dl.push(DisplayCommand::draw_text(
                        &path,
                        picker.x + 8.0,
                        picker.y + 7.0,
                        Color::rgb(60, 64, 67),
                        11.0,
                        FontWeight::Regular,
                        FontFamily::SansSerif,
                    ));

                    let has_parent = dir.parent().is_some();
                    for row in 0..PICKER_FILE_ROWS {
                        let row_y = picker.y + PICKER_FILE_ROW_Y + row as f32 * PICKER_FILE_ROW_H;
                        if row_y + PICKER_FILE_ROW_H > picker.y + picker.height {
                            break;
                        }
                        let (row_hit, label, is_dir) = if has_parent && row == 0 {
                            (Some(PickerHit::FileParent), "..".to_string(), true)
                        } else {
                            let entry_row = row - usize::from(has_parent);
                            match entries.get(*offset + entry_row) {
                                Some(e) => (
                                    Some(PickerHit::FileRow((*offset + entry_row) as u32)),
                                    format!("{}{}", e.name, if e.is_dir { "/" } else { "" }),
                                    e.is_dir,
                                ),
                                None => (None, String::new(), false),
                            }
                        };
                        let Some(row_hit) = row_hit else {
                            continue;
                        };
                        let rect = Rect::new(
                            picker.x + 4.0,
                            row_y,
                            picker.width - 8.0,
                            PICKER_FILE_ROW_H - 1.0,
                        );
                        if picker.hover == Some(row_hit) {
                            dl.push(DisplayCommand::FillRoundedRect {
                                rect,
                                color: Color::rgb(225, 235, 255),
                                radii: [4.0; 4],
                            });
                        }
                        let label = truncate_label(&label, 44);
                        dl.push(DisplayCommand::draw_text(
                            &label,
                            rect.x() + 6.0,
                            row_y + 4.0,
                            if is_dir {
                                Color::rgb(0, 90, 170)
                            } else {
                                Color::rgb(32, 33, 36)
                            },
                            12.0,
                            FontWeight::Regular,
                            FontFamily::SansSerif,
                        ));
                    }
                }
            }
        }

        // ── HTML5 CONSTRAINT VALIDATION BUBBLE OVERLAY ──
        if let Some(bubble) = &self.validation_bubble {
            for cmd in bubble.render() {
                dl.push(cmd);
            }
        }
    }
}
