//! Clipboard access via `arboard`.
//!
//! Provides cross-platform get/set for UTF-8 text using the system clipboard.
//! On Windows this uses the Win32 clipboard API; on Linux it targets X11/Wayland;
//! on macOS it uses NSPasteboard — all through the `arboard` crate.

/// System clipboard backed by `arboard`.
///
/// `arboard::Clipboard` must be kept alive for the duration of clipboard use
/// (some backends require the object to persist while paste targets are active).
pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    /// Creates a new clipboard handle.
    ///
    /// Returns a valid instance on success, or a no-op shell if the platform
    /// clipboard is unavailable (e.g. running headless without a display server).
    pub fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(cb) => Self { inner: Some(cb) },
            Err(e) => {
                log::warn!("[clipboard] Failed to open system clipboard: {e}");
                Self { inner: None }
            }
        }
    }

    /// Gets the current UTF-8 text from the system clipboard.
    ///
    /// Returns `None` if the clipboard is empty, contains non-text data,
    /// or the platform clipboard is unavailable.
    pub fn get_text(&mut self) -> Option<String> {
        match self.inner.as_mut()?.get_text() {
            Ok(text) => Some(text),
            Err(arboard::Error::ContentNotAvailable) => None,
            Err(e) => {
                log::warn!("[clipboard] get_text error: {e}");
                None
            }
        }
    }

    /// Places UTF-8 text on the system clipboard.
    ///
    /// Does nothing if the platform clipboard is unavailable.
    pub fn set_text(&mut self, text: &str) {
        if let Some(cb) = self.inner.as_mut() {
            if let Err(e) = cb.set_text(text.to_owned()) {
                log::warn!("[clipboard] set_text error: {e}");
            }
        }
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}
