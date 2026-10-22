//! Image prefetching, decoding channel, and per-frame queue draining.

use super::BrowserChrome;
use super::types::assert_not_on_ui_thread;
use crate::chrome_ui::IMAGE_PREFETCH_PER_FRAME;
use mango_net::Url;
use mango_render::{cache_image, decode_data_uri, decode_image_bytes, get_cached_image};

/// Maximum number of queued image fetches retained in memory to prevent
/// unbounded backlog growth on malicious/image-heavy pages.
pub const MAX_PENDING_IMAGE_FETCHES: usize = 256;

impl BrowserChrome {
    /// Downloads and decodes one image into the shared render cache.
    ///
    /// Fast path only (memory cache + `data:` URIs). Remote fetches are never
    /// issued from the UI thread here; callers must queue them via
    /// [`Self::queue_image_backlog`] instead. Guards the UI-freeze P0.
    #[allow(dead_code)]
    pub(crate) fn prefetch_image(&self, base: &Url, src: &str) {
        assert_not_on_ui_thread("prefetch_image");
        if get_cached_image(src).is_some() {
            return;
        }
        if src.trim_start().starts_with("data:") {
            if let Some(decoded) = decode_data_uri(src) {
                cache_image(src, decoded);
            }
            return;
        }
        let Ok(img_bytes) = self.loader.fetch_image_bytes(base, src) else {
            return;
        };
        let Some(decoded) = decode_image_bytes(&img_bytes) else {
            return;
        };
        cache_image(src, decoded.clone());
        if let Ok(resolved) = base.resolve(src) {
            cache_image(&resolved.as_str(), decoded);
        }
    }

    /// Drains and closes any stale image channel from a previous navigation.
    pub(crate) fn close_stale_image_channel(&mut self) {
        if let Some(old_rx) = self.image_rx.take() {
            while old_rx.try_recv().is_ok() {}
        }
    }

    /// Queues background image URLs with epoch guarding and dedup.
    pub(crate) fn queue_image_backlog(&mut self, sources: Vec<String>, base: Url) {
        self.close_stale_image_channel();
        let epoch = self
            .image_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(sources.len().min(MAX_PENDING_IMAGE_FETCHES));
        for src in sources {
            if deduped.len() >= MAX_PENDING_IMAGE_FETCHES {
                break;
            }
            if get_cached_image(&src).is_some() {
                continue;
            }
            if seen.insert(src.clone()) {
                deduped.push(src);
            }
        }
        let mut remote = Vec::new();
        for src in deduped {
            if src.trim_start().starts_with("data:") {
                if let Some(decoded) = decode_data_uri(&src) {
                    cache_image(&src, decoded);
                }
            } else {
                remote.push(src);
            }
        }
        remote.truncate(MAX_PENDING_IMAGE_FETCHES);
        let (tx, rx) = std::sync::mpsc::channel();
        self.image_rx = Some(rx);
        self.image_fetch_base = Some(base.clone());
        self.pending_image_fetches = remote.clone();
        if remote.is_empty() {
            return;
        }
        let loader = self.loader.clone();
        let epoch_flag = self.image_epoch.clone();
        let work_queue = std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::VecDeque::from(remote),
        ));
        let num_workers = 4.min(work_queue.lock().unwrap().len());
        for worker_idx in 0..num_workers {
            let tx = tx.clone();
            let loader = loader.clone();
            let base = base.clone();
            let epoch_flag = epoch_flag.clone();
            let queue = work_queue.clone();
            if let Err(e) = std::thread::Builder::new()
                .name(format!("mango-img-worker-{worker_idx}"))
                .spawn(move || {
                    while let Some(src) = {
                        let mut q = queue.lock().unwrap();
                        q.pop_front()
                    } {
                        if epoch_flag.load(std::sync::atomic::Ordering::SeqCst) != epoch {
                            break;
                        }
                        if get_cached_image(&src).is_some() {
                            let _ = tx.send(src);
                            mango_core::wake_event_loop();
                            continue;
                        }
                        let fetch_res = loader.fetch_image_bytes(&base, &src);
                        let bytes = match fetch_res {
                            Ok(b) => b,
                            Err(e) => {
                                log::debug!("[mango-img] Image fetch failed for {src}: {e:?}");
                                let _ = tx.send(src);
                                mango_core::wake_event_loop();
                                continue;
                            }
                        };
                        if epoch_flag.load(std::sync::atomic::Ordering::SeqCst) != epoch {
                            break;
                        }
                        if let Some(decoded) = decode_image_bytes(&bytes) {
                            cache_image(&src, decoded.clone());
                            if let Ok(resolved) = base.resolve(&src) {
                                cache_image(&resolved.as_str(), decoded);
                            }
                        }
                        let _ = tx.send(src);
                        mango_core::wake_event_loop();
                    }
                })
            {
                log::warn!("Failed to spawn image worker {worker_idx}: {e}");
            }
        }
        drop(tx);
    }

    /// True while background image prefetch work remains.
    pub fn has_pending_image_fetches(&self) -> bool {
        self.image_rx.is_some() || !self.pending_image_fetches.is_empty()
    }

    /// Fetches a slice of the queued image backlog. Returns `true` when at
    /// least one newly cached image is available for painting.
    pub fn drain_pending_images(&mut self) -> bool {
        let mut loaded_any = false;
        let mut disconnected = false;
        if let Some(ref rx) = self.image_rx {
            loop {
                match rx.try_recv() {
                    Ok(src) => {
                        if get_cached_image(&src).is_some() {
                            loaded_any = true;
                        }
                        if let Some(pos) = self.pending_image_fetches.iter().position(|s| s == &src)
                        {
                            self.pending_image_fetches.remove(pos);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                self.image_rx = None;
                self.pending_image_fetches.clear();
            }
        } else if !self.pending_image_fetches.is_empty() {
            if let Some(base) = self.image_fetch_base.clone() {
                let take = IMAGE_PREFETCH_PER_FRAME.min(self.pending_image_fetches.len());
                let batch: Vec<String> = self.pending_image_fetches.drain(..take).collect();
                let mut remote_to_fetch = Vec::new();
                for src in batch {
                    if get_cached_image(&src).is_some() {
                        loaded_any = true;
                    } else if src.trim_start().starts_with("data:") {
                        if let Some(decoded) = decode_data_uri(&src) {
                            cache_image(&src, decoded);
                            loaded_any = true;
                        }
                    } else {
                        remote_to_fetch.push(src);
                    }
                }
                if !remote_to_fetch.is_empty() {
                    let mut requeue = remote_to_fetch.clone();
                    requeue.extend(self.pending_image_fetches.drain(..));
                    requeue.truncate(MAX_PENDING_IMAGE_FETCHES);
                    self.pending_image_fetches = requeue;
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.image_rx = Some(rx);
                    let loader = self.loader.clone();
                    let epoch = self.image_epoch.load(std::sync::atomic::Ordering::SeqCst);
                    let epoch_flag = self.image_epoch.clone();
                    let first_batch = remote_to_fetch;
                    if let Err(e) = std::thread::Builder::new()
                        .name("mango-image-drain".to_string())
                        .spawn(move || {
                            for src in first_batch {
                                if epoch_flag.load(std::sync::atomic::Ordering::SeqCst) != epoch {
                                    break;
                                }
                                if get_cached_image(&src).is_some() {
                                    continue;
                                }
                                let Ok(bytes) = loader.fetch_image_bytes(&base, &src) else {
                                    continue;
                                };
                                if epoch_flag.load(std::sync::atomic::Ordering::SeqCst) != epoch {
                                    break;
                                }
                                if let Some(decoded) = decode_image_bytes(&bytes) {
                                    cache_image(&src, decoded.clone());
                                    if let Ok(resolved) = base.resolve(&src) {
                                        cache_image(&resolved.as_str(), decoded);
                                    }
                                    let _ = tx.send(src);
                                    mango_core::wake_event_loop();
                                }
                            }
                        })
                    {
                        log::warn!("Failed to spawn image drain thread: {e}");
                    }
                }
            } else {
                self.pending_image_fetches.clear();
            }
        }
        if loaded_any {
            self.page_dirty = true;
            self.display_list_cache.lock().unwrap().clear();
        }
        loaded_any
    }
}
