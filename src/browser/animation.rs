//! CSS animation, transition, JS timer ticking, and smooth scrolling dispatch.

use super::BrowserChrome;
use mango_core::Size;

impl BrowserChrome {
    /// Advances active CSS transitions and `@keyframes` animations, and steps smooth scrolling.
    ///
    /// Performs at most ONE layout per call and never clones the full
    /// `style_snapshot`: the snapshot is borrowed while the owned root box is
    /// blended in place.
    pub fn tick_animations(&mut self, dt_ms: f32) -> bool {
        let mut needs_redraw = false;

        if let Some(mut anim) = self.smooth_scroll.take() {
            let (new_y, finished) = anim.step(std::time::Instant::now());
            if (self.scroll_y_f64 - new_y).abs() > 0.001 {
                self.scroll_y_f64 = new_y;
                self.scroll_y = new_y as f32;
                if let Some(tab) = self.tabs.get_mut(self.active_tab_idx) {
                    tab.scroll_y_f64 = new_y;
                    tab.scroll_y = self.scroll_y;
                }
                self.display_list_cache.lock().unwrap().clear();
                needs_redraw = true;
            }
            if !finished {
                self.smooth_scroll = Some(anim);
            }
        }

        if self.root_box.is_none() || !self.anim_state.is_animating() {
            return needs_redraw;
        }
        if dt_ms <= 0.0 {
            return needs_redraw;
        }
        self.anim_state.advance(dt_ms);

        let mut root = match self.root_box.take() {
            Some(r) => r,
            None => return needs_redraw,
        };

        // Borrow (never clone) the base-style snapshot while blending the
        // owned root box in place.
        let snapshot = &self.style_snapshot;
        let anim_state = &self.anim_state;
        mango_layout::apply_box_style_overrides(&mut root, &mut |node, style| {
            let Some(node) = node else { return };
            let Some(base) = snapshot.get(&node.raw()) else {
                return;
            };
            let mut blended = anim_state.transitions.current_style(node, base);
            anim_state.animations.apply(node, &mut blended);
            *style = blended;
        });

        let viewport = Size::new(self.content_width(), self.content_height());
        self.page_display_list = mango_layout::relayout_box_tree(&mut root, viewport);
        self.cached_scrollable_height = crate::scroll::compute_scrollable_height(Some(&root));
        self.page_dirty = true;
        self.root_box = Some(root);
        true
    }

    /// Ticks JavaScript timers and background execution. Returns `true` if a DOM mutation occurred.
    pub fn tick_js(&mut self) -> bool {
        let mutated = if let Some(ref mut rt) = self.js_runtime {
            let mutated = rt.tick();
            if let Some(alert_msg) = rt.take_status_text() {
                self.status_text = alert_msg;
            }
            for msg in rt.take_console_messages() {
                let lvl = match msg.level {
                    mango_js::console::ConsoleLevel::Log => crate::devtools::ConsoleLevel::Log,
                    mango_js::console::ConsoleLevel::Info => crate::devtools::ConsoleLevel::Info,
                    mango_js::console::ConsoleLevel::Warn => crate::devtools::ConsoleLevel::Warn,
                    mango_js::console::ConsoleLevel::Error => crate::devtools::ConsoleLevel::Error,
                };
                self.devtools.log(lvl, msg.text);
            }
            if let Some(redirect) = rt.take_pending_navigation() {
                self.navigate(&redirect);
                return true;
            }
            if let Some(open_req) = rt.take_pending_open_window() {
                let nav_url = if let Some(base) = &self.base_url {
                    base.resolve(&open_req.url)
                        .map(|u| u.to_string())
                        .unwrap_or(open_req.url)
                } else {
                    open_req.url
                };
                self.new_tab_and_navigate(&nav_url);
                return true;
            }
            mutated
        } else {
            return false;
        };

        if mutated && self.root_box.is_some() {
            self.relayout();
            return true;
        }
        false
    }

    /// Returns true if the active page has pending animations, smooth scrolling, or JavaScript timers.
    pub fn has_pending_timers(&self) -> bool {
        self.smooth_scroll.is_some()
            || self.anim_state.is_animating()
            || self
                .js_runtime
                .as_ref()
                .is_some_and(|rt| rt.has_pending_timers())
    }

    /// Returns the earliest deadline when a JavaScript timer needs execution.
    pub fn next_timer_deadline(&self) -> Option<std::time::Instant> {
        self.js_runtime
            .as_ref()
            .and_then(|rt| rt.next_timer_deadline())
    }

    /// Triggers smooth scrolling to `target_y` over the specified duration.
    pub fn scroll_smooth_to(&mut self, target_y: f32, duration: Option<std::time::Duration>) {
        self.scroll_smooth_to_f64(target_y as f64, duration);
    }

    /// Triggers smooth scrolling to `target_y` with 64-bit precision over the specified duration.
    pub fn scroll_smooth_to_f64(&mut self, target_y: f64, duration: Option<std::time::Duration>) {
        let max_s = self.max_scroll() as f64;
        let dur = duration.unwrap_or(std::time::Duration::from_millis(300));
        if let Some(existing) = &mut self.smooth_scroll {
            existing.update_target(target_y, dur);
        } else {
            self.smooth_scroll = Some(crate::scroll::SmoothScrollAnimation::new(
                self.scroll_y_f64,
                target_y,
                max_s,
                dur,
            ));
        }
    }

    /// Returns whether any CSS transitions or animations are currently running.
    pub fn is_animating(&self) -> bool {
        self.anim_state.is_animating()
    }

    /// Returns a reference to the active CSS animation state.
    pub fn animation_state(&self) -> &mango_css::AnimationState {
        &self.anim_state
    }
}
