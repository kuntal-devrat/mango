//! Window creation and pixel buffer management.
//!
//! Wraps `winit` for cross-platform window creation and `softbuffer` for
//! presenting a CPU-rendered pixel buffer to the screen.

use std::num::NonZeroU32;
use std::sync::Arc;

use softbuffer::{Context, Surface};
use winit::dpi::LogicalSize;
use winit::window::Window;

use mango_core::Color;

/// Manages the pixel buffer that backs the window surface.
///
/// The rendering pipeline writes pixels into the buffer, then
/// [`PixelBuffer::present`] copies the buffer to the window.
pub struct PixelBuffer {
    _context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    width: u32,
    height: u32,
}

impl PixelBuffer {
    /// Creates a new pixel buffer backed by the given window.
    pub fn new(window: Arc<Window>) -> Self {
        let context = Context::new(window.clone()).expect("failed to create softbuffer context");
        let surface = Surface::new(&context, window.clone()).expect("failed to create surface");
        let size = window.inner_size();

        let mut pb = Self {
            _context: context,
            surface,
            width: size.width.max(1),
            height: size.height.max(1),
        };
        pb.resize(size.width, size.height);
        pb
    }

    /// Resizes the backing buffer. Call this when the window is resized.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.width = width;
        self.height = height;

        self.surface
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .expect("failed to resize surface");
    }

    /// Returns the current buffer dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Gets a mutable buffer, executes the drawing closure, then presents.
    ///
    /// The closure receives a mutable slice of `u32` pixels (0xRRGGBB format)
    /// and the (width, height) of the buffer.
    pub fn draw<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [u32], u32, u32),
    {
        let mut buffer = self.surface.buffer_mut().expect("failed to get buffer");
        f(&mut buffer, self.width, self.height);
        buffer.present().expect("failed to present buffer");
    }
}

/// Creates the Mango browser window with default settings.
///
/// Returns the window and a suggested initial size.
pub fn create_window(event_loop: &winit::event_loop::ActiveEventLoop) -> Arc<Window> {
    let attrs = Window::default_attributes()
        .with_title("🥭 Mango")
        .with_inner_size(LogicalSize::new(1024.0, 768.0))
        .with_min_inner_size(LogicalSize::new(400.0, 300.0));

    let window = event_loop
        .create_window(attrs)
        .expect("failed to create window");
    Arc::new(window)
}

/// Fills a pixel buffer with a solid color.
pub fn fill_solid(buffer: &mut [u32], _width: u32, _height: u32, color: Color) {
    let pixel = color.to_rgb_u32();
    buffer.fill(pixel);
}

/// Draws a filled rectangle into the pixel buffer.
#[allow(clippy::too_many_arguments)]
pub fn fill_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    if w == 0 || h == 0 || buf_width == 0 || buf_height == 0 {
        return;
    }
    let pixel = color.to_rgb_u32();

    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x.saturating_add(w as i32)).clamp(0, buf_width as i32);
    let y1 = (y.saturating_add(h as i32)).clamp(0, buf_height as i32);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let x_start = x0 as u32;
    let y_start = y0 as u32;
    let x_end = x1 as u32;
    let y_end = y1 as u32;

    for py in y_start..y_end {
        let row_offset = (py * buf_width) as usize;
        for px in x_start..x_end {
            let idx = row_offset + px as usize;
            if idx < buffer.len() {
                buffer[idx] = pixel;
            }
        }
    }
}

/// Draws a 1px border rectangle (outline only) into the pixel buffer.
#[allow(clippy::too_many_arguments)]
pub fn stroke_rect(
    buffer: &mut [u32],
    buf_width: u32,
    buf_height: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    color: Color,
) {
    // Top edge
    fill_rect(buffer, buf_width, buf_height, x, y, w, 1, color);
    // Bottom edge
    fill_rect(
        buffer,
        buf_width,
        buf_height,
        x,
        y + h as i32 - 1,
        w,
        1,
        color,
    );
    // Left edge
    fill_rect(buffer, buf_width, buf_height, x, y, 1, h, color);
    // Right edge
    fill_rect(
        buffer,
        buf_width,
        buf_height,
        x + w as i32 - 1,
        y,
        1,
        h,
        color,
    );
}
