use mango_platform::input::KeyState;

use super::super::BrowserChrome;
use crate::devtools::DevToolsTab;

impl BrowserChrome {
    pub(crate) fn devtools_hit_test(&mut self, state: KeyState) -> bool {
        if !self.devtools.is_open {
            return false;
        }
        let panel_y = (self.height as f32 - self.devtools.height).max(0.0);
        if self.mouse_y < panel_y {
            return false;
        }
        if state != KeyState::Released {
            return true;
        }

        let tab_bar_h = 26.0;
        if self.mouse_y <= panel_y + tab_bar_h {
            if self.mouse_x >= self.width as f32 - 32.0 {
                self.devtools.is_open = false;
            } else {
                let tab = ((self.mouse_x - 10.0) / 72.0).floor() as usize;
                let selected = [
                    DevToolsTab::Elements,
                    DevToolsTab::Console,
                    DevToolsTab::Network,
                    DevToolsTab::Sources,
                    DevToolsTab::Computed,
                    DevToolsTab::Performance,
                    DevToolsTab::Debugger,
                ]
                .get(tab)
                .copied();
                if let Some(tab) = selected {
                    self.devtools.active_tab = tab;
                }
            }
            self.page_dirty = true;
            return true;
        }

        if self.devtools.active_tab == DevToolsTab::Console {
            let content_y = panel_y + tab_bar_h;
            if self.mouse_y >= content_y && self.mouse_x >= self.width as f32 - 84.0 {
                self.devtools.clear_console();
                self.page_dirty = true;
            }
        }
        true
    }

    pub(crate) fn devtools_scroll(&mut self, delta_y: f32) -> bool {
        if !self.devtools.is_open {
            return false;
        }
        let panel_y = (self.height as f32 - self.devtools.height).max(0.0);
        if self.mouse_y < panel_y || self.devtools.active_tab != DevToolsTab::Console {
            return false;
        }
        self.devtools
            .scroll_console(if delta_y < 0.0 { 3 } else { -3 });
        self.page_dirty = true;
        true
    }
}
