//! Image prefetching, decoding channel, and per-frame queue draining.

use super::BrowserChrome;
use super::types::assert_not_on_ui_thread;
use crate::chrome_ui::IMAGE_PREFETCH_PER_FRAME;
use mango_net::Url;
use mango_render::{cache_image, decode_data_uri, decode_image_bytes, get_cached_image};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed number of OS threads used to fetch and decode images. A fixed pool
/// prevents image-heavy pages from creating one thread per image (or one
/// thread per frame while draining).
const MAX_IMAGE_WORKERS: usize = 4;

/// Maximum number of per-frame drain batches that may be in flight at once.
const MAX_IMAGE_DRAIN_JOBS: usize = 4;

/// Result returned by an image worker. Keeping the byte count and decode
/// outcome together makes resource accounting truthful without retaining the
/// response buffer on the UI thread.
#[derive(Debug)]
pub struct ImageFetchResult {
    pub src: String,
    pub bytes: usize,
    pub decoded: bool,
}

static IMAGE_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn image_pool() -> &'static rayon::ThreadPool {
    IMAGE_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(MAX_IMAGE_WORKERS)
            .thread_name(|i| format!("mango-image-pool-{i}"))
            .build()
            .expect("failed to initialize image decode pool")
    })
}

/// RAII counter for queued image work. The counter is decremented even if a
/// worker panics, so a failed decode can never permanently exhaust the
/// per-frame in-flight budget.
struct InFlightGuard(Arc<AtomicUsize>);

impl InFlightGuard {
    fn acquire(counter: &Arc<AtomicUsize>, limit: usize) -> Option<Self> {
        if counter.fetch_add(1, Ordering::SeqCst) >= limit {
            counter.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(Arc::clone(counter)))
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl BrowserChrome {
    pub(crate) fn queue_additional_image_sources(&mut self, sources: Vec<String>, base: Url) {
        let mut queued = std::collections::HashSet::new();
        queued.extend(self.pending_image_fetches.iter().cloned());
        for raw in sources {
            let src = raw
                .trim()
                .strip_prefix("url(")
                .and_then(|value| value.strip_suffix(')'))
                .unwrap_or(raw.trim())
                .trim_matches(['\'', '"'])
                .trim();
            if src.is_empty() || get_cached_image(src).is_some() || !queued.insert(src.to_string())
            {
                continue;
            }
            let id = self.resource_scheduler.discover(
                crate::browser::resource_scheduler::ResourceKind::Image,
                crate::browser::resource_scheduler::ResourcePriority::Visible,
                src.to_string(),
            );
            self.resource_scheduler.transition(
                id,
                crate::browser::resource_scheduler::ResourceState::Queued,
            );
            self.pending_image_fetches.push(src.to_string());
        }
        self.image_fetch_base = Some(base);
    }

    /// Reorders pending image work from post-layout geometry. Only resource
    /// metadata and queue order change; image loads never trigger layout.
    pub(crate) fn reprioritize_visible_images(&mut self) {
        let visible = {
            let Some(root) = self.root_box.as_ref() else {
                return;
            };
            let document = self.document_view();
            let mut image_nodes = Vec::new();
            crate::browser::navigation::resources::collect_image_node_sources(
                &document,
                document.root(),
                &mut image_nodes,
            );
            let mut rects = std::collections::HashMap::new();
            mango_layout::collect_box_rects(root, &mut rects);
            let viewport_top = self.scroll_y;
            let viewport_bottom = viewport_top + self.content_height();
            let mut visible = std::collections::HashSet::new();
            for (node_id, src) in image_nodes {
                if let Some([_, y, _, height]) = rects.get(&node_id.raw())
                    && *y + *height >= viewport_top
                    && *y <= viewport_bottom
                {
                    visible.insert(src);
                }
            }
            visible
        };
        for src in &visible {
            self.resource_scheduler.set_priority_url(
                src,
                crate::browser::resource_scheduler::ResourcePriority::Visible,
            );
        }
        self.pending_image_fetches.sort_by_key(|src| {
            if visible.contains(src) {
                0usize
            } else {
                1usize
            }
        });
    }

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
        let _epoch = self
            .image_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(sources.len());
        for src in sources {
            if get_cached_image(&src).is_some() {
                continue;
            }
            if seen.insert(src.clone()) {
                deduped.push(src);
            }
        }
        let mut remote = Vec::new();
        for src in deduped {
            let id = self.resource_scheduler.discover(
                crate::browser::resource_scheduler::ResourceKind::Image,
                crate::browser::resource_scheduler::ResourcePriority::Speculative,
                src.clone(),
            );
            self.resource_scheduler.transition(
                id,
                crate::browser::resource_scheduler::ResourceState::Queued,
            );
            if src.trim_start().starts_with("data:") {
                if let Some(decoded) = decode_data_uri(&src) {
                    cache_image(&src, decoded);
                    self.resource_scheduler.finish_url(
                        &src,
                        crate::browser::resource_scheduler::ResourceState::Completed,
                        None,
                        0,
                    );
                } else {
                    self.resource_scheduler.finish_url(
                        &src,
                        crate::browser::resource_scheduler::ResourceState::Failed,
                        Some(crate::browser::resource_scheduler::RejectReason::DecodeFailure),
                        0,
                    );
                }
            } else {
                remote.push(src);
            }
        }
        // Keep all request metadata, but fetch/decode only bounded batches in
        // `drain_pending_images`. This removes compatibility truncation while
        // keeping network and decoded-pixel work bounded.
        self.image_rx = None;
        self.image_fetch_base = Some(base.clone());
        self.pending_image_fetches = remote.clone();
    }

    /// True while background image prefetch work remains.
    pub fn has_pending_image_fetches(&self) -> bool {
        self.image_rx.is_some() || !self.pending_image_fetches.is_empty()
    }

    /// Number of image requests still waiting for a worker result or retry.
    /// This is intentionally observable so headless callers never have to
    /// reduce an incomplete drain to a misleading success boolean.
    pub fn pending_image_fetch_count(&self) -> usize {
        self.pending_image_fetches.len()
    }

    /// Fetches a slice of the queued image backlog. Returns `true` when at
    /// least one newly cached image is available for painting.
    pub fn drain_pending_images(&mut self) -> bool {
        let mut loaded_any = false;
        let mut disconnected = false;
        if let Some(ref rx) = self.image_rx {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        let src = result.src;
                        if result.decoded && get_cached_image(&src).is_some() {
                            loaded_any = true;
                            self.resource_scheduler.finish_url(
                                &src,
                                crate::browser::resource_scheduler::ResourceState::Completed,
                                None,
                                result.bytes,
                            );
                        } else {
                            self.devtools.log(
                                crate::devtools::ConsoleLevel::Warn,
                                format!("Resource failed: image unavailable or undecodable: {src}"),
                            );
                            self.resource_scheduler.finish_url(
                                &src,
                                crate::browser::resource_scheduler::ResourceState::Failed,
                                Some(if result.decoded {
                                    crate::browser::resource_scheduler::RejectReason::NetworkFailure
                                } else if result.bytes > 0 {
                                    crate::browser::resource_scheduler::RejectReason::DecodeFailure
                                } else {
                                    crate::browser::resource_scheduler::RejectReason::NetworkFailure
                                }),
                                result.bytes,
                            );
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
                // The pending vector deliberately retains the in-flight
                // batch until each worker result is consumed.  A worker can
                // exit early on navigation cancellation or a panic; clearing
                // it here silently loses resources and makes later pages
                // appear to have loaded successfully.
            }
        } else if !self.pending_image_fetches.is_empty() {
            if let Some(base) = self.image_fetch_base.clone() {
                let Some(_guard) =
                    InFlightGuard::acquire(&self.image_drain_in_flight, MAX_IMAGE_DRAIN_JOBS)
                else {
                    // The pool is still busy with earlier per-frame batches;
                    // leave the backlog queued instead of spawning more work.
                    return loaded_any;
                };
                let take = IMAGE_PREFETCH_PER_FRAME.min(self.pending_image_fetches.len());
                // Keep ownership in the pending metadata while the worker is
                // running.  Results remove individual URLs below.  This
                // makes channel disconnects retryable instead of lossy.
                let batch: Vec<String> = self.pending_image_fetches[..take].to_vec();
                let mut remote_to_fetch = Vec::new();
                for src in batch {
                    if get_cached_image(&src).is_some() {
                        loaded_any = true;
                        self.pending_image_fetches.retain(|queued| queued != &src);
                        self.resource_scheduler.finish_url(
                            &src,
                            crate::browser::resource_scheduler::ResourceState::Completed,
                            None,
                            0,
                        );
                    } else if src.trim_start().starts_with("data:") {
                        if let Some(decoded) = decode_data_uri(&src) {
                            cache_image(&src, decoded);
                            loaded_any = true;
                            self.pending_image_fetches.retain(|queued| queued != &src);
                            self.resource_scheduler.finish_url(
                                &src,
                                crate::browser::resource_scheduler::ResourceState::Completed,
                                None,
                                0,
                            );
                        }
                    } else {
                        remote_to_fetch.push(src);
                    }
                }
                if !remote_to_fetch.is_empty() {
                    let (tx, rx) = std::sync::mpsc::channel::<ImageFetchResult>();
                    self.image_rx = Some(rx);
                    let epoch = self.image_epoch.load(Ordering::SeqCst);
                    // Submit one task per image to the fixed pool.  The old
                    // implementation submitted one pool task which fetched
                    // the entire batch serially, leaving three worker threads
                    // idle and making image-heavy pages appear capped.
                    // `IMAGE_PREFETCH_PER_FRAME` bounds this fan-out, while
                    // the pool bounds total OS threads and decoded work.
                    for src in remote_to_fetch {
                        let tx = tx.clone();
                        let loader = self.loader.clone();
                        let base = base.clone();
                        let epoch_flag = self.image_epoch.clone();
                        image_pool().spawn(move || {
                            if epoch_flag.load(Ordering::SeqCst) != epoch {
                                return;
                            }
                            if get_cached_image(&src).is_some() {
                                let _ = tx.send(ImageFetchResult {
                                    src,
                                    bytes: 0,
                                    decoded: true,
                                });
                                return;
                            }
                            let Ok(bytes) = loader.fetch_image_bytes(&base, &src) else {
                                log::debug!("[mango-img] Image fetch failed for {src}");
                                let _ = tx.send(ImageFetchResult {
                                    src,
                                    bytes: 0,
                                    decoded: false,
                                });
                                return;
                            };
                            let byte_len = bytes.len();
                            if epoch_flag.load(Ordering::SeqCst) != epoch {
                                return;
                            }
                            let decoded_ok = if let Some(decoded) = decode_image_bytes(&bytes) {
                                cache_image(&src, decoded.clone());
                                if let Ok(resolved) = base.resolve(&src) {
                                    cache_image(&resolved.as_str(), decoded);
                                }
                                true
                            } else {
                                false
                            };
                            // Completion is reported for both successful
                            // and failed fetch/decode attempts so the
                            // backlog cannot remain permanently pending.
                            let _ = tx.send(ImageFetchResult {
                                src,
                                bytes: byte_len,
                                decoded: decoded_ok,
                            });
                            mango_core::wake_event_loop();
                        });
                    }
                    // The receiver must close once every worker task exits;
                    // only the cloned senders are retained by the workers.
                    drop(tx);
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
