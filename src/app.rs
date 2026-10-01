//! Application state and the winit event loop handler.
//!
//! Implements `winit::application::ApplicationHandler` to receive window
//! events and drive the browser's main loop, timers, and rendering.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

use mango_platform::input::{KeyEvent, KeyState, MangoKey, Modifiers};
use mango_platform::window::{PixelBuffer, create_window};

use crate::browser::BrowserChrome;

/// The top-level application state.
pub struct MangoApp {
    /// The main window (created on resume).
    window: Option<Arc<Window>>,
    /// The pixel buffer for rendering.
    pixel_buffer: Option<PixelBuffer>,
    /// The browser chrome (UI, tabs, navigation, JS runtime).
    browser: Option<BrowserChrome>,
    /// Current modifier key state.
    modifiers: Modifiers,
    /// Timestamp of the previous animation tick, for frame-delta computation.
    last_frame: Option<Instant>,
}

impl Default for MangoApp {
    fn default() -> Self {
        Self::new()
    }
}

impl MangoApp {
    pub fn new() -> Self {
        Self {
            window: None,
            pixel_buffer: None,
            browser: None,
            modifiers: Modifiers::default(),
            last_frame: None,
        }
    }

    /// Redraws the entire window.
    fn redraw(&mut self) {
        let Some(browser) = &self.browser else {
            return;
        };
        let Some(pixel_buffer) = &mut self.pixel_buffer else {
            return;
        };

        let display_list = browser.build_display_list(pixel_buffer.size());

        pixel_buffer.draw(|buffer, width, height| {
            let bg = mango_core::Color::MANGO_LIGHT;
            buffer.fill(bg.to_rgb_u32());
            mango_render::painter::paint(&display_list, buffer, width, height);
        });
    }
}

impl ApplicationHandler for MangoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // Already initialized
        }

        log::info!("Creating Mango window...");
        let window = create_window(event_loop);
        let pixel_buffer = PixelBuffer::new(window.clone());

        let size = window.inner_size();
        let mut browser = BrowserChrome::new(size.width, size.height);
        browser.async_navigation = true;

        self.window = Some(window.clone());
        self.pixel_buffer = Some(pixel_buffer);
        self.browser = Some(browser);

        window.request_redraw();
        log::info!("Mango is ready with Phase 7 Web Layout & Viewport Engine! 🥭");
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut needs_redraw = false;

        // Frame delta for the CSS animation/transition runtime.
        let now = Instant::now();
        let dt_ms = self
            .last_frame
            .map(|prev| now.duration_since(prev).as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        self.last_frame = Some(now);

        if let Some(browser) = &mut self.browser {
            // Check for completed background navigation
            if browser.tick_navigation() {
                needs_redraw = true;
            }

            // Tick JS event loop (setTimeout/setInterval)
            if browser.tick_js() {
                needs_redraw = true;
            }

            // Drive CSS transitions and @keyframes animations (GAP-004/GAP-006)
            if browser.tick_animations(dt_ms) {
                needs_redraw = true;
            }

            // Keep draining the background image backlog (OPT-009) so
            // image-heavy pages finish loading instead of dropping images.
            if browser.drain_pending_images() {
                needs_redraw = true;
            }

            // Poll at ~60 FPS while timers or animations are pending, otherwise wait for events
            if browser.has_pending_timers()
                || browser.is_animating()
                || browser.has_pending_image_fetches()
                || browser.is_loading()
            {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(16),
                ));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }

        if needs_redraw && let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested. Goodbye! 🥭");
                if let Some(browser) = &self.browser {
                    browser.persist_profile();
                }
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(pb) = &mut self.pixel_buffer {
                    pb.resize(size.width, size.height);
                }
                if let Some(browser) = &mut self.browser {
                    browser.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                self.redraw();
            }

            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.modifiers = Modifiers {
                    ctrl: state.control_key(),
                    alt: state.alt_key(),
                    shift: state.shift_key(),
                    meta: state.super_key(),
                };
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(browser) = &mut self.browser {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as f32,
                    };
                    browser.handle_scroll(dy);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let key_event = KeyEvent {
                    key: MangoKey::from_winit(&event.logical_key),
                    state: KeyState::from(event.state),
                    modifiers: self.modifiers,
                };

                if key_event.state == KeyState::Pressed {
                    // Global Application Shortcuts:
                    // 1. Ctrl+Q: Quit
                    if key_event.modifiers.ctrl
                        && matches!(&key_event.key, MangoKey::Char('q') | MangoKey::Char('Q'))
                    {
                        log::info!("Ctrl+Q — quitting Mango");
                        if let Some(browser) = &self.browser {
                            browser.persist_profile();
                        }
                        event_loop.exit();
                        return;
                    }

                    // 2. Ctrl+T: New Tab
                    if key_event.modifiers.ctrl
                        && matches!(&key_event.key, MangoKey::Char('t') | MangoKey::Char('T'))
                    {
                        if let Some(browser) = &mut self.browser {
                            browser.new_tab();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // 3. Ctrl+W: Close Tab
                    if key_event.modifiers.ctrl
                        && matches!(&key_event.key, MangoKey::Char('w') | MangoKey::Char('W'))
                    {
                        if let Some(browser) = &mut self.browser {
                            browser.close_active_tab();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // 4. Ctrl+Tab: Switch Tab
                    if key_event.modifiers.ctrl && matches!(&key_event.key, MangoKey::Tab) {
                        if let Some(browser) = &mut self.browser {
                            browser.next_tab();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // 5. F5 or Ctrl+R: Reload
                    if matches!(&key_event.key, MangoKey::F5)
                        || (key_event.modifiers.ctrl
                            && matches!(&key_event.key, MangoKey::Char('r') | MangoKey::Char('R')))
                    {
                        if let Some(browser) = &mut self.browser {
                            browser.reload();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // 6. Alt+Left: Go Back
                    if key_event.modifiers.alt && matches!(&key_event.key, MangoKey::ArrowLeft) {
                        if let Some(browser) = &mut self.browser {
                            browser.go_back();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // 7. Alt+Right: Go Forward
                    if key_event.modifiers.alt && matches!(&key_event.key, MangoKey::ArrowRight) {
                        if let Some(browser) = &mut self.browser {
                            browser.go_forward();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                        return;
                    }

                    // Forward to browser chrome
                    if let Some(browser) = &mut self.browser {
                        browser.handle_key_event(&key_event);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if let Some(browser) = &mut self.browser
                    && browser.handle_mouse_move(position.x as f32, position.y as f32)
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(browser) = &mut self.browser {
                    let key_state = KeyState::from(state);
                    let btn = mango_platform::input::MouseButton::from(button);
                    browser.handle_mouse_click(btn, key_state);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}
