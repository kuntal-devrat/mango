//! Application state and the winit event loop handler.
//!
//! Implements `winit::application::ApplicationHandler` to receive window
//! events and drive the browser's multi-window main loop, timers, and rendering.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Window, WindowId};

use mango_platform::input::{KeyEvent, KeyState, MangoKey, Modifiers};
use mango_platform::window::{WindowSurface, create_window};

use crate::browser::BrowserChrome;

/// State associated with a single top-level window.
pub struct WindowState {
    pub surface: WindowSurface,
    pub browser: BrowserChrome,
    pub window: Arc<Window>,
    pending_resize: Option<(u32, u32)>,
}

impl WindowState {
    pub fn new(window: Arc<Window>) -> Self {
        let surface = WindowSurface::new(window.clone());
        let size = window.inner_size();
        let mut browser = BrowserChrome::new(size.width, size.height);
        browser.async_navigation = true;
        Self {
            surface,
            browser,
            window,
            pending_resize: None,
        }
    }

    /// Redraws the browser window.
    ///
    /// When backed by a hardware GPU compositor (`lgpu`), uses cached layer tiles
    /// and updates only the translation matrix for 60–120 FPS buttery-smooth scrolling
    /// with zero CPU re-rasterization.
    /// Falls back to direct display list presentation in software mode.
    pub fn redraw(&mut self) {
        let size = self.surface.size();
        if self.surface.is_gpu() {
            let raster_pending = self.browser.update_layers(size);
            if let Err(err) = self
                .surface
                .composite_and_present(&self.browser.layer_tree, self.browser.canvas_bg())
            {
                log::warn!("GPU composite error: {err}");
            }
            if raster_pending {
                // Continue bounded tile raster work across separate event-loop
                // turns so input and window events can run between batches.
                self.window.request_redraw();
            }
        } else {
            let dl = self.browser.build_display_list(size);
            if let Err(err) = self
                .surface
                .present_display_list(&dl, self.browser.canvas_bg())
            {
                log::warn!("Presentation error: {err}");
            }
        }
    }
}

/// The top-level multi-window application state.
pub struct MangoApp {
    /// Active windows keyed by winit WindowId.
    windows: HashMap<WindowId, WindowState>,
    /// Initial URL specified on command line.
    pub initial_url: Option<String>,
    /// Current modifier key state.
    modifiers: Modifiers,
    /// Timestamp of the previous animation tick, for frame-delta computation.
    last_frame: Option<Instant>,
}

impl Default for MangoApp {
    fn default() -> Self {
        Self::new(None)
    }
}

impl MangoApp {
    pub fn new(initial_url: Option<String>) -> Self {
        Self {
            windows: HashMap::new(),
            initial_url,
            modifiers: Modifiers::default(),
            last_frame: None,
        }
    }

    /// Spawns a new top-level window, optionally navigating to `url`.
    pub fn create_new_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        url: Option<&str>,
    ) -> WindowId {
        log::info!("Creating new Mango window...");
        let window = create_window(event_loop);
        let id = window.id();
        let mut state = WindowState::new(window.clone());
        if let Some(u) = url {
            state.browser.navigate(u);
        }
        window.request_redraw();
        self.windows.insert(id, state);
        id
    }
}

impl ApplicationHandler for MangoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return; // Already initialized
        }

        log::info!("Initializing Mango Browser...");
        let initial_url = self.initial_url.clone();
        self.create_new_window(event_loop, initial_url.as_deref());
        log::info!("Mango is ready with Multi-Window Rendering Engine! 🥭");
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Frame delta for the CSS animation/transition runtime.
        let now = Instant::now();
        let dt_ms = self
            .last_frame
            .map(|prev| now.duration_since(prev).as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        self.last_frame = Some(now);

        let mut any_animating = false;
        let mut any_pending_scripts = false;
        let mut any_smooth_scroll = false;
        let mut any_background_work = false;
        let mut earliest_timer_deadline: Option<Instant> = None;
        let mut earliest_script_resume: Option<Instant> = None;
        let mut windows_to_open: Vec<String> = Vec::new();

        for state in self.windows.values_mut() {
            let mut needs_redraw = false;

            if let Some((width, height)) = state.pending_resize.take() {
                state.surface.resize(width, height);
                state.browser.resize(width, height);
                needs_redraw = true;
            }

            if state.browser.tick_async_layout() {
                needs_redraw = true;
            }

            // Check for completed background navigation. Always poll: a
            // background tab may finish while the active tab is idle.
            if state.browser.tick_navigation() {
                needs_redraw = true;
            }
            if state.browser.has_pending_navigation() {
                any_background_work = true;
            }
            if state.browser.has_pending_layout() {
                any_background_work = true;
            }

            // Drain asynchronous messages from the isolated Out-of-Process Renderer
            if state.browser.tick_renderer() {
                needs_redraw = true;
            }
            if state
                .browser
                .renderer_actor
                .as_ref()
                .is_some_and(|r| r.is_busy())
            {
                any_background_work = true;
            }

            // Tick JS event loop (setTimeout/setInterval, and deferred/async scripts)
            // only when isolated renderer actor is not active (sync test mode)
            if state.browser.renderer_actor.is_none() {
                let had_scripts =
                    state.browser.has_pending_scripts() && !state.browser.has_pending_layout();
                if state.browser.has_pending_timers() || state.browser.is_loading() || had_scripts {
                    let mutated = state.browser.tick_js();
                    if mutated || had_scripts {
                        needs_redraw = true;
                    }
                }
            }

            // Check for pending window.open requests from scripts
            if let Some(open_req) = state
                .browser
                .js_runtime
                .as_mut()
                .and_then(|rt| rt.take_pending_open_window())
            {
                let nav_url = if let Some(base) = &state.browser.base_url {
                    base.resolve(&open_req.url)
                        .map(|u| u.to_string())
                        .unwrap_or(open_req.url.clone())
                } else {
                    open_req.url.clone()
                };
                if open_req.features.contains("width=") || open_req.features.contains("popup") {
                    windows_to_open.push(nav_url);
                } else {
                    state.browser.new_tab_and_navigate(&nav_url);
                    needs_redraw = true;
                }
            }

            // Drive CSS transitions and @keyframes animations
            if state.browser.is_animating() && state.browser.tick_animations(dt_ms) {
                needs_redraw = true;
            }

            // Coalesce all DOM mutations observed during this turn into one
            // layout. This keeps a page with many timers/microtasks from
            // relaying the entire document once per callback.
            if state.browser.flush_pending_layout() {
                needs_redraw = true;
            }

            // Keep draining the background image backlog
            if state.browser.has_pending_image_fetches() && state.browser.drain_pending_images() {
                needs_redraw = true;
            }

            if state.browser.is_animating() {
                any_animating = true;
            }
            if state.browser.has_pending_scripts() && !state.browser.has_pending_layout() {
                any_pending_scripts = true;
            }
            if let Some(deadline) = state.browser.pending_script_resume_deadline() {
                earliest_script_resume =
                    Some(earliest_script_resume.map_or(deadline, |prev| prev.min(deadline)));
            }
            if state.browser.has_pending_image_fetches() || state.browser.has_pending_font_fetches()
            {
                any_background_work = true;
            }
            if state.browser.smooth_scroll.is_some() {
                any_smooth_scroll = true;
            }

            if let Some(dl) = state.browser.next_timer_deadline() {
                earliest_timer_deadline = match earliest_timer_deadline {
                    Some(prev) => Some(prev.min(dl)),
                    None => Some(dl),
                };
            }

            if needs_redraw {
                state.window.request_redraw();
            }
        }

        // Spawn any requested popup windows
        for url in windows_to_open {
            self.create_new_window(event_loop, Some(&url));
        }

        // Keep script work incremental, but let the OS process events between
        // scripts. Network and image workers wake this loop through the event
        // loop proxy; a short bounded poll while they are active also lets us
        // advance their queues without waking continuously.
        let script_work_runnable = any_pending_scripts
            && earliest_script_resume.is_none_or(|deadline| deadline <= Instant::now());
        if script_work_runnable {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(4),
            ));
        } else if any_animating || any_smooth_scroll || any_background_work {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        } else if let Some(deadline) = earliest_timer_deadline
            .into_iter()
            .chain(earliest_script_resume)
            .min()
        {
            let now = Instant::now();
            if deadline <= now {
                event_loop.set_control_flow(ControlFlow::WaitUntil(now));
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            // All asynchronous producers use the proxy waker; the platform
            // event loop itself remains responsible for servicing window-system
            // traffic while idle.
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        // about_to_wait drains the result queues and requests redraw only for
        // windows whose visible state actually changed.
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window close requested for {:?}.", window_id);
                if let Some(state) = self.windows.remove(&window_id) {
                    state.browser.persist_profile();
                }
                if self.windows.is_empty() {
                    log::info!("All windows closed. Goodbye! 🥭");
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if let Some(state) = self.windows.get_mut(&window_id) {
                    // Window systems can emit a burst of resize events while
                    // dragging. Keep only the latest dimensions and rebuild
                    // layout/surfaces once at the next event-loop boundary.
                    state.pending_resize = Some((size.width, size.height));
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.redraw();
                }
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
                if let Some(state) = self.windows.get_mut(&window_id) {
                    // Keep winit's sign convention: positive delta scrolls
                    // towards the top/left of the document. Negating here was
                    // the source of the inverted-wheel report.
                    match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            state.browser.handle_wheel_scroll_smooth(x, y);
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            state.browser.handle_wheel_scroll_smooth(
                                (pos.x / 40.0) as f32,
                                (pos.y / 40.0) as f32,
                            );
                        }
                    }
                    state.window.request_redraw();
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
                    // 1. Ctrl+Q: Quit all windows
                    if key_event.modifiers.ctrl
                        && matches!(&key_event.key, MangoKey::Char('q') | MangoKey::Char('Q'))
                    {
                        log::info!("Ctrl+Q — quitting Mango");
                        for (_, s) in self.windows.drain() {
                            s.browser.persist_profile();
                        }
                        event_loop.exit();
                        return;
                    }

                    // 2. Ctrl+N: New Window
                    if key_event.modifiers.ctrl
                        && matches!(&key_event.key, MangoKey::Char('n') | MangoKey::Char('N'))
                    {
                        self.create_new_window(event_loop, None);
                        return;
                    }

                    // Window-specific shortcuts
                    if let Some(win_state) = self.windows.get_mut(&window_id) {
                        // 3. Ctrl+T: New Tab
                        if key_event.modifiers.ctrl
                            && matches!(&key_event.key, MangoKey::Char('t') | MangoKey::Char('T'))
                        {
                            win_state.browser.new_tab();
                            win_state.window.request_redraw();
                            return;
                        }

                        // 4. Ctrl+W: Close Tab
                        if key_event.modifiers.ctrl
                            && matches!(&key_event.key, MangoKey::Char('w') | MangoKey::Char('W'))
                        {
                            if win_state.browser.tab_count() <= 1 {
                                // Close window if last tab closed
                                win_state.browser.persist_profile();
                                self.windows.remove(&window_id);
                                if self.windows.is_empty() {
                                    event_loop.exit();
                                }
                            } else {
                                win_state.browser.close_active_tab();
                                win_state.window.request_redraw();
                            }
                            return;
                        }

                        // 5. Ctrl+Tab: Switch Tab
                        if key_event.modifiers.ctrl && matches!(&key_event.key, MangoKey::Tab) {
                            win_state.browser.next_tab();
                            win_state.window.request_redraw();
                            return;
                        }

                        // 6. F5 or Ctrl+R: Reload
                        if matches!(&key_event.key, MangoKey::F5)
                            || (key_event.modifiers.ctrl
                                && matches!(
                                    &key_event.key,
                                    MangoKey::Char('r') | MangoKey::Char('R')
                                ))
                        {
                            win_state.browser.reload();
                            win_state.window.request_redraw();
                            return;
                        }

                        // 7. Alt+Left: Go Back
                        if key_event.modifiers.alt && matches!(&key_event.key, MangoKey::ArrowLeft)
                        {
                            win_state.browser.go_back();
                            win_state.window.request_redraw();
                            return;
                        }

                        // 8. Alt+Right: Go Forward
                        if key_event.modifiers.alt && matches!(&key_event.key, MangoKey::ArrowRight)
                        {
                            win_state.browser.go_forward();
                            win_state.window.request_redraw();
                            return;
                        }

                        // 9. F11: Fullscreen Toggle
                        if key_event.key == MangoKey::F11 {
                            let is_full = win_state.window.fullscreen().is_some();
                            if is_full {
                                win_state.window.set_fullscreen(None);
                            } else {
                                win_state.window.set_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(None),
                                ));
                            }
                            win_state.browser.toggle_fullscreen();
                            win_state.window.request_redraw();
                            return;
                        }

                        // Forward to browser chrome
                        win_state.browser.handle_key_event(&key_event);
                        win_state.window.request_redraw();
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if let Some(win_state) = self.windows.get_mut(&window_id)
                    && win_state
                        .browser
                        .handle_mouse_move(position.x as f32, position.y as f32)
                {
                    win_state.window.request_redraw();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(win_state) = self.windows.get_mut(&window_id) {
                    let key_state = KeyState::from(state);
                    let btn = mango_platform::input::MouseButton::from(button);
                    win_state.browser.handle_mouse_click(btn, key_state);
                    win_state.window.request_redraw();
                }
            }

            WindowEvent::Touch(touch) => {
                use winit::event::TouchPhase;
                if let Some(win_state) = self.windows.get_mut(&window_id) {
                    let x = touch.location.x as f32;
                    let y = touch.location.y as f32;
                    match touch.phase {
                        TouchPhase::Started => {
                            win_state.browser.handle_mouse_move(x, y);
                            win_state.browser.handle_mouse_click(
                                mango_platform::input::MouseButton::Left,
                                KeyState::Pressed,
                            );
                        }
                        TouchPhase::Moved => {
                            let dy = (y - win_state.browser.mouse_y()) / 45.0;
                            win_state.browser.handle_scroll(dy);
                            win_state.browser.handle_mouse_move(x, y);
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            win_state.browser.handle_mouse_click(
                                mango_platform::input::MouseButton::Left,
                                KeyState::Released,
                            );
                        }
                    }
                    win_state.window.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        log::info!("Exiting Mango application...");
        for (_, state) in self.windows.drain() {
            state.browser.persist_profile();
            // Intentionally bypass manual destruction of GPU Vulkan surfaces and Wayland window handles
            // during process shutdown. If the Wayland compositor disconnected or closed the socket
            // (e.g. broken pipe / force quit), Mesa/libwayland-client can dereference invalid display pointers,
            // producing a secondary SIGSEGV. Memory and handles are cleanly reclaimed by the OS kernel on exit.
            std::mem::forget(state.surface);
            std::mem::forget(state.window);
        }
    }
}
