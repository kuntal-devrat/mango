//! Cookie and localStorage profile persistence on disk.

use std::path::Path;
use super::BrowserChrome;

/// Restores a `localStorage` profile (`<key>\t<value>` or `<origin>\t<key>\t<value>` per line) into the store.
pub fn load_local_storage(path: &Path, store: &mango_js::web_apis::SharedLocalStorage) {
    match mango_js::web_apis::load_local_storage_from_file(path, store) {
        Ok(count) if count > 0 => {
            log::info!(
                "Restored {count} localStorage entries from {}",
                path.display()
            );
        }
        _ => {}
    }
}

/// Writes the `localStorage` store to its profile file (GAP-017, Phase 6.4 / PRD 7.6).
pub fn save_local_storage(path: &Path, store: &mango_js::web_apis::SharedLocalStorage) {
    if let Err(e) = mango_js::web_apis::save_local_storage_to_file(path, store) {
        log::warn!("Could not persist localStorage: {e}");
    }
}

impl BrowserChrome {
    /// Flushes the shared cookie jar to the profile file asynchronously on a background thread.
    pub fn persist_cookies(&self) {
        let jar = self.cookie_jar.clone();
        let path = self.cookie_store_path.clone();
        std::thread::spawn(move || {
            if let Ok(jar) = jar.lock() {
                if let Err(e) = jar.save_to_file(&path) {
                    log::warn!("Could not persist cookies: {e}");
                }
            }
        });
    }

    /// Synchronously flushes the cookie jar to disk. Used on browser shutdown.
    pub fn persist_cookies_sync(&self) {
        let Ok(jar) = self.cookie_jar.lock() else {
            return;
        };
        if let Err(e) = jar.save_to_file(&self.cookie_store_path) {
            log::warn!("Could not persist cookies: {e}");
        }
    }

    /// Flushes the shared localStorage store to the profile file (GAP-017).
    pub fn persist_local_storage(&self) {
        save_local_storage(&self.local_storage_path, &self.local_storage);
    }

    /// Flushes both profile-backed web storage stores. Call on shutdown.
    pub fn persist_profile(&self) {
        self.persist_cookies_sync();
        self.persist_local_storage();
    }
}
