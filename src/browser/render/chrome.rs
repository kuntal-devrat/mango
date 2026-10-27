//! Browser chrome display list construction: scrollbars, tab strip, toolbar, omnibox, status bar, devtools, and find-in-page.

use mango_core::{Color, Rect};
use mango_render::display_list::{BorderWidths, DisplayCommand, DisplayList};
use mango_render::{FontFamily, FontWeight};

use super::super::BrowserChrome;
use crate::chrome_ui::*;

impl BrowserChrome {
    /// Builds the display list for the scrollbar overlay layer.
    ///
    /// Extracted from the main chrome layer so that scrolling only invalidates and
    /// re-rasterizes this tiny 12px strip, rather than repainting the entire window's
    /// tabs, toolbar, and devtools on CPU.
    pub fn build_scrollbar_display_list(
        &self,
        (width, height): (u32, u32),
    ) -> Option<(Rect, DisplayList)> {
        let max_s = self.max_scroll();
        if max_s <= 0.0 {
            return None;
        }

        let w = width as f32;
        let h = height as f32;
        let content_y = HEADER_HEIGHT + 1.0;
        let content_h = (h - HEADER_HEIGHT - STATUS_BAR_HEIGHT - 1.0).max(10.0);
        let doc_h = self.scrollable_height();

        let (scrollbar_w, custom_track, custom_thumb) = if let Some(root) = &self.root_box {
            if let Some(s) = &root.style {
                let w = match s.scrollbar_width {
                    mango_css::values::ScrollbarWidth::None => 0.0,
                    mango_css::values::ScrollbarWidth::Thin => 8.0,
                    mango_css::values::ScrollbarWidth::Auto => 12.0,
                };
                let (track, thumb) = if let Some((c_thumb, c_track)) = s.scrollbar_color {
                    (Some(c_track), Some(c_thumb))
                } else {
                    (None, None)
                };
                (w, track, thumb)
            } else {
                (12.0, None, None)
            }
        } else {
            (12.0, None, None)
        };

        if scrollbar_w <= 0.0 {
            return None;
        }

        let scrollbar_x = w - scrollbar_w;
        let bounds = Rect::new(scrollbar_x, content_y, scrollbar_w, content_h);

        let mut dl = DisplayList::new();

        // Track background in layer-local coordinates (0..scrollbar_w, 0..content_h)
        let track_rect = Rect::new(0.0, 0.0, scrollbar_w, content_h);
        dl.push(DisplayCommand::FillRect {
            rect: track_rect,
            color: custom_track.unwrap_or_else(|| Color::rgba(240, 240, 240, 140)),
        });

        // Thumb dimensions and local position
        let thumb_h =
            ((content_h / doc_h) * content_h).clamp(32.0f32.min(content_h), content_h);
        let max_thumb_travel = (content_h - thumb_h).max(1.0);
        let thumb_y_local = (self.scroll_y / max_s) * max_thumb_travel;

        let thumb_padding = 2.0f32;
        let thumb_rect = Rect::new(
            thumb_padding,
            thumb_y_local,
            (scrollbar_w - thumb_padding * 2.0).max(1.0),
            thumb_h,
        );

        let thumb_color = if let Some(ct) = custom_thumb {
            ct
        } else if self.scrollbar_dragging {
            Color::rgb(100, 100, 100)
        } else if self.hovered_target == Some(HoverTarget::ScrollbarThumb) {
            Color::rgb(140, 140, 140)
        } else {
            Color::rgb(190, 190, 190)
        };

        dl.push(DisplayCommand::FillRoundedRect {
            rect: thumb_rect,
            color: thumb_color,
            radii: [4.0, 4.0, 4.0, 4.0],
        });

        Some((bounds, dl))
    }

    /// Builds the display list for the browser chrome UI (tab strip,
    /// toolbar, omnibox, bookmark bar, status bar, popups, and devtools).
    ///
    /// Scrollbars are excluded from this layer to prevent scrolling from invalidating
    /// chrome textures.
    pub fn build_chrome_display_list(&self, (width, height): (u32, u32)) -> DisplayList {
        let mut dl = DisplayList::new();
        let w = width as f32;
        let h = height as f32;
        let content_y = HEADER_HEIGHT + 1.0;

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 1 — TAB STRIP (y = 0..TAB_BAR_HEIGHT) ──
        // ══════════════════════════════════════════════════════════════════════

        // Tab strip background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, 0.0, w, TAB_BAR_HEIGHT),
            color: TAB_BAR_BG,
        });

        let tab_count = self.tabs.len().max(1);
        let max_strip_w = (w - 42.0).max(100.0);
        let tab_w = (max_strip_w / tab_count as f32).clamp(80.0, 210.0);

        for (i, tab) in self.tabs.iter().enumerate() {
            let tab_x = i as f32 * tab_w;
            let is_active = i == self.active_tab_idx;
            let is_hovered = self.hovered_target == Some(HoverTarget::Tab(i));

            let tab_bg = if is_active {
                TAB_ACTIVE_BG
            } else if is_hovered {
                TAB_HOVER_BG
            } else {
                TAB_INACTIVE_BG
            };

            // Tab body (rounded top feel: offset y = 5.0 to TAB_BAR_HEIGHT)
            let tab_h = TAB_BAR_HEIGHT - 5.0;
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(tab_x + 2.0, 5.0, tab_w - 4.0, tab_h),
                color: tab_bg,
            });

            // If active, connect tab seamlessly to toolbar below
            if is_active {
                dl.push(DisplayCommand::FillRect {
                    rect: Rect::new(tab_x + 2.0, TAB_BAR_HEIGHT - 1.0, tab_w - 4.0, 2.0),
                    color: TAB_ACTIVE_BG,
                });
            } else {
                // Subtle separator between inactive tabs
                dl.push(DisplayCommand::DrawLine {
                    x1: tab_x + tab_w - 1.0,
                    y1: 12.0,
                    x2: tab_x + tab_w - 1.0,
                    y2: TAB_BAR_HEIGHT - 6.0,
                    color: Color::rgb(60, 64, 67),
                    thickness: 1.0,
                });
            }

            // Tab favicon dot / badge
            let dot_color = if is_active {
                Color::MANGO_ORANGE
            } else {
                Color::rgb(120, 124, 130)
            };
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(tab_x + 8.0, 14.0, 6.0, 6.0),
                color: dot_color,
            });

            // Tab title
            let char_count = tab.title.chars().count();
            let title_text = if char_count > 18 {
                let truncated: String = tab.title.chars().take(15).collect();
                format!("{truncated}...")
            } else {
                tab.title.clone()
            };

            dl.push(DisplayCommand::draw_text(
                title_text,
                tab_x + 18.0,
                11.0,
                if is_active {
                    TAB_TEXT_ACTIVE
                } else {
                    TAB_TEXT_INACTIVE
                },
                12.0,
                if is_active {
                    FontWeight::Bold
                } else {
                    FontWeight::Regular
                },
                FontFamily::SansSerif,
            ));

            // Tab close button (×)
            let close_x = tab_x + tab_w - 20.0;
            let close_hovered = self.hovered_target == Some(HoverTarget::TabClose(i));
            if close_hovered {
                dl.push(DisplayCommand::FillRect {
                    rect: Rect::new(close_x - 3.0, 9.0, 16.0, 16.0),
                    color: TAB_CLOSE_HOVER,
                });
            }

            dl.push(DisplayCommand::draw_text(
                "x".to_string(),
                close_x,
                11.0,
                if close_hovered {
                    Color::WHITE
                } else {
                    TAB_TEXT_INACTIVE
                },
                11.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));
        }

        // New tab button (+)
        let new_tab_x = (tab_count as f32 * tab_w + 4.0).min(w - 30.0);
        let new_tab_hovered = self.hovered_target == Some(HoverTarget::NewTab);
        if new_tab_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(new_tab_x - 2.0, 7.0, 22.0, 22.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "+".to_string(),
            new_tab_x + 3.0,
            9.0,
            if new_tab_hovered {
                Color::WHITE
            } else {
                TAB_TEXT_INACTIVE
            },
            16.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 2 — TOOLBAR & OMNIBOX (y = TAB_BAR_HEIGHT..HEADER_HEIGHT) ──
        // ══════════════════════════════════════════════════════════════════════

        // Toolbar background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, TAB_BAR_HEIGHT, w, TOOLBAR_HEIGHT),
            color: TOOLBAR_BG,
        });

        let btn_y = TAB_BAR_HEIGHT + 6.0;
        let can_back = self
            .tabs
            .get(self.active_tab_idx)
            .is_some_and(|t| t.can_go_back());
        let can_forward = self
            .tabs
            .get(self.active_tab_idx)
            .is_some_and(|t| t.can_go_forward());

        // Back button (◀ / <)
        let back_hovered = self.hovered_target == Some(HoverTarget::Back) && can_back;
        if back_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(6.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "<".to_string(),
            14.0,
            btn_y + 3.0,
            if can_back {
                TOOLBAR_TEXT
            } else {
                TOOLBAR_TEXT_DISABLED
            },
            15.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Forward button (▶ / >)
        let forward_hovered = self.hovered_target == Some(HoverTarget::Forward) && can_forward;
        if forward_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(36.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            ">".to_string(),
            44.0,
            btn_y + 3.0,
            if can_forward {
                TOOLBAR_TEXT
            } else {
                TOOLBAR_TEXT_DISABLED
            },
            15.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Reload button (↻ / R)
        let reload_hovered = self.hovered_target == Some(HoverTarget::Reload);
        if reload_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(66.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "R".to_string(),
            74.0,
            btn_y + 3.0,
            if reload_hovered {
                Color::WHITE
            } else {
                TOOLBAR_TEXT
            },
            14.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Home button (🏠 / H)
        let home_hovered = self.hovered_target == Some(HoverTarget::Home);
        if home_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(96.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            "H".to_string(),
            104.0,
            btn_y + 3.0,
            if home_hovered {
                Color::MANGO_ORANGE
            } else {
                TOOLBAR_TEXT
            },
            14.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // ── Omnibox (Address Bar) ──
        let addr_x = 132.0;
        let addr_y = TAB_BAR_HEIGHT + 5.0;
        let addr_w = (w - 176.0).max(120.0);
        let addr_h = 30.0;

        // Omnibox background
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(addr_x, addr_y, addr_w, addr_h),
            color: ADDRESSBAR_BG,
        });

        // Omnibox border when focused
        if self.address_focused {
            dl.push(DisplayCommand::DrawBorder {
                rect: Rect::new(addr_x, addr_y, addr_w, addr_h),
                color: ADDRESSBAR_BORDER,
                widths: BorderWidths {
                    top: 1.5,
                    right: 1.5,
                    bottom: 1.5,
                    left: 1.5,
                },
                radii: [4.0; 4],
            });
        }

        // Security / Mango badge on left of Omnibox
        let is_https = self.address_text.starts_with("https://");
        let badge_color = if is_https {
            Color::rgb(52, 168, 83)
        } else {
            Color::MANGO_ORANGE
        };
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(addr_x + 8.0, addr_y + 11.0, 8.0, 8.0),
            color: badge_color,
        });

        let display_text = if self.address_text.is_empty() {
            "Search or enter web address..."
        } else {
            &self.address_text
        };

        // Text selection highlight
        let text_render_x = addr_x + 22.0;
        if self.address_focused && self.is_all_selected && !self.address_text.is_empty() {
            let (sel_w, _) = mango_render::font_manager().measure_text(
                &self.address_text,
                13.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            );
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(text_render_x - 2.0, addr_y + 4.0, sel_w + 4.0, addr_h - 8.0),
                color: Color::rgb(0, 120, 215),
            });
        }

        let text_color = if self.address_text.is_empty() {
            Color::rgb(130, 134, 140)
        } else if self.address_focused && self.is_all_selected {
            Color::WHITE
        } else {
            ADDRESSBAR_TEXT
        };

        dl.push(DisplayCommand::draw_text(
            display_text.to_string(),
            text_render_x,
            addr_y + 6.0,
            text_color,
            13.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // Cursor
        if self.address_focused && !self.is_all_selected {
            let safe_pos = self.cursor_pos.min(self.address_text.len());
            let mut boundary = safe_pos;
            while boundary > 0 && !self.address_text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            let cursor_offset = mango_render::font_manager()
                .measure_text(
                    &self.address_text[..boundary],
                    13.0,
                    FontWeight::Regular,
                    FontFamily::SansSerif,
                )
                .0;
            let cursor_x = text_render_x + cursor_offset;
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(cursor_x, addr_y + 6.0, 1.5, addr_h - 12.0),
                color: Color::MANGO_ORANGE,
            });
        }

        // Mango menu button (⋮)
        let menu_x = w - 34.0;
        let menu_hovered = self.hovered_target == Some(HoverTarget::Menu);
        if menu_hovered {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(menu_x - 3.0, btn_y - 2.0, 26.0, 26.0),
                color: TOOLBAR_BTN_HOVER,
            });
        }
        dl.push(DisplayCommand::draw_text(
            ":".to_string(),
            menu_x + 5.0,
            btn_y + 1.0,
            TOOLBAR_TEXT,
            18.0,
            FontWeight::Bold,
            FontFamily::SansSerif,
        ));

        // Toolbar bottom separator line
        dl.push(DisplayCommand::DrawLine {
            x1: 0.0,
            y1: HEADER_HEIGHT,
            x2: w,
            y2: HEADER_HEIGHT,
            color: Color::rgb(70, 72, 77),
            thickness: 1.0,
        });

        // Loading indicator bar
        if self.is_loading {
            dl.push(DisplayCommand::FillRect {
                rect: Rect::new(0.0, HEADER_HEIGHT - 2.0, w * 0.7, 2.5),
                color: Color::MANGO_ORANGE,
            });
        }

        // ══════════════════════════════════════════════════════════════════════
        // ── CHROME REDESIGN: PART 3 — STATUS BAR (y = h - STATUS_BAR_HEIGHT..h) ──
        // ══════════════════════════════════════════════════════════════════════

        let status_y = h - STATUS_BAR_HEIGHT;
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(0.0, status_y, w, STATUS_BAR_HEIGHT),
            color: STATUS_BAR_BG,
        });

        dl.push(DisplayCommand::DrawLine {
            x1: 0.0,
            y1: status_y,
            x2: w,
            y2: status_y,
            color: Color::rgb(60, 64, 67),
            thickness: 1.0,
        });

        dl.push(DisplayCommand::draw_text(
            self.status_text.clone(),
            8.0,
            status_y + 5.0,
            STATUS_BAR_TEXT,
            11.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        let title_str = format!("Mango 2026 | {}", self.page_title);
        let title_x = (w - (title_str.len() as f32 * 6.5) - 16.0).max(200.0);
        dl.push(DisplayCommand::draw_text(
            title_str,
            title_x,
            status_y + 5.0,
            STATUS_BAR_TEXT,
            11.0,
            FontWeight::Regular,
            FontFamily::SansSerif,
        ));

        // ── OVERLAYS: CONTEXT MENU, DROPDOWNS, PICKERS, VALIDATION BUBBLE ──
        self.render_chrome_overlays(&mut dl);

        // ── DEVTOOLS & INSPECTOR PANEL ──
        if self.devtools.is_open {
            // Borrowed view: the inspector repaints every frame while it is open.
            let doc = self.document_view();
            let highlight_cmds =
                self.devtools
                    .render_highlight(self.root_box.as_ref(), self.scroll_y, content_y);
            for cmd in highlight_cmds {
                dl.push(cmd);
            }
            let panel_cmds = self
                .devtools
                .render_panel(w, h, &doc, self.root_box.as_ref());
            for cmd in panel_cmds {
                dl.push(cmd);
            }
        }

        // ── FIND-IN-PAGE FLOATING OVERLAY (Ctrl+F) ──
        if self.find_in_page.active {
            let bar_w = 320.0f32;
            let bar_h = 38.0f32;
            let bar_x = (w - bar_w - 24.0).max(10.0);
            let bar_y = content_y + 10.0;

            // Background card with shadow & border
            dl.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(bar_x - 1.0, bar_y - 1.0, bar_w + 2.0, bar_h + 2.0),
                color: Color::rgba(0, 0, 0, 40),
                radii: [8.0; 4],
            });
            dl.push(DisplayCommand::FillRoundedRect {
                rect: Rect::new(bar_x, bar_y, bar_w, bar_h),
                color: Color::rgb(255, 255, 255),
                radii: [7.0; 4],
            });
            dl.push(DisplayCommand::DrawBorder {
                rect: Rect::new(bar_x, bar_y, bar_w, bar_h),
                widths: BorderWidths::all(1.0),
                color: Color::rgb(218, 220, 224),
                radii: [7.0; 4],
            });

            // Search query text or placeholder
            let query_display = if self.find_in_page.query.is_empty() {
                "Find in page..."
            } else {
                &self.find_in_page.query
            };
            let text_color = if self.find_in_page.query.is_empty() {
                Color::rgb(140, 145, 155)
            } else {
                Color::rgb(32, 33, 36)
            };
            dl.push(DisplayCommand::draw_text(
                query_display,
                bar_x + 12.0,
                bar_y + 11.0,
                text_color,
                14.0,
                FontWeight::Regular,
                FontFamily::SansSerif,
            ));

            // Match count badge e.g. "1/5" or "0/0"
            let badge_str = if self.find_in_page.query.is_empty() {
                String::new()
            } else if self.find_in_page.matches.is_empty() {
                "0/0".to_string()
            } else {
                format!(
                    "{}/{}",
                    self.find_in_page.current_match + 1,
                    self.find_in_page.matches.len()
                )
            };
            if !badge_str.is_empty() {
                let badge_w = mango_layout::measure_text_width(&badge_str, 12.0);
                let badge_x = bar_x + bar_w - 95.0 - badge_w;
                dl.push(DisplayCommand::draw_text(
                    &badge_str,
                    badge_x,
                    bar_y + 12.0,
                    if self.find_in_page.matches.is_empty() {
                        Color::rgb(217, 48, 37)
                    } else {
                        Color::rgb(95, 99, 104)
                    },
                    12.0,
                    FontWeight::Bold,
                    FontFamily::SansSerif,
                ));
            }

            // Prev button (▲)
            dl.push(DisplayCommand::draw_text(
                "▲",
                bar_x + bar_w - 80.0,
                bar_y + 12.0,
                Color::rgb(95, 99, 104),
                12.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));

            // Next button (▼)
            dl.push(DisplayCommand::draw_text(
                "▼",
                bar_x + bar_w - 55.0,
                bar_y + 12.0,
                Color::rgb(95, 99, 104),
                12.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));

            // Close button (✕)
            dl.push(DisplayCommand::draw_text(
                "✕",
                bar_x + bar_w - 28.0,
                bar_y + 11.0,
                Color::rgb(95, 99, 104),
                14.0,
                FontWeight::Bold,
                FontFamily::SansSerif,
            ));
        }

        dl
    }
}
