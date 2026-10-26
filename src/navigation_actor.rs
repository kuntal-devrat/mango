//! Dedicated Navigation Actor for Mango Browser.
//!
//! Decouples network fetching, HTML parsing, and resource pipeline preparation
//! onto background worker threads, communicating with the UI thread via message
//! passing. Supports sequence-based navigation cancellation so newer
//! navigations supersede older in-flight requests.
//!
//! Concurrency model: a fixed pool of 4 workers drains a shared command queue.
//! No per-navigation OS threads are spawned; each worker processes one
//! navigation at a time (fetch → prepare → emit). Cancellation is cooperative:
//! bumping the epoch abandons stale queued commands immediately and the
//! document pipeline checks the epoch between each bounded fetch batch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::browser::types::fetch_with_timeout;
use crate::browser::{PreparedDocument, prepare_document_pipeline_cancellable};
use mango_net::{ResourceLoader, Url};

/// Number of background navigation workers (fixed pool, no per-nav spawn).
pub const NAV_WORKER_COUNT: usize = 4;
/// Per-navigation fetch timeout applied inside the pipeline stages.
pub const NAV_FETCH_TIMEOUT: Duration = Duration::from_secs(60);

static PROGRESSIVE_PREP_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

fn progressive_prep_pool() -> &'static rayon::ThreadPool {
    PROGRESSIVE_PREP_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|i| format!("mango-progressive-prep-{i}"))
            .build()
            .expect("failed to initialize progressive preparation pool")
    })
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Commands sent from the UI thread to the Navigation Actor.
#[derive(Debug)]
pub enum NavCommand {
    /// Start navigating to a new URL.
    Navigate {
        tab_id: usize,
        url: Url,
        push_history: bool,
        epoch: u64,
    },
    /// Submit form data via POST.
    Post {
        tab_id: usize,
        url: Url,
        body: Vec<u8>,
        content_type: String,
        push_history: bool,
        epoch: u64,
    },
    /// Cancel any in-flight navigation.
    Cancel,
    /// Gracefully terminate worker threads.
    Shutdown,
}

/// Per-tab navigation epochs.
///
/// A global epoch counter is wrong for a tabbed browser: navigating tab B must
/// not cancel tab A's in-flight navigation. Each tab owns the latest epoch
/// allocated for it, and workers only abort when *their* tab's epoch changes.
struct EpochTracker {
    next: AtomicU64,
    /// Latest epoch allocated or cancellation generation, used for diagnostics
    /// and the legacy `active_epoch()` API.
    global: AtomicU64,
    per_tab: Mutex<HashMap<usize, u64>>,
}

impl EpochTracker {
    fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
            global: AtomicU64::new(1),
            per_tab: Mutex::new(HashMap::new()),
        }
    }

    fn begin(&self, tab_id: usize) -> u64 {
        let epoch = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.global.fetch_max(epoch, Ordering::SeqCst);
        if let Ok(mut map) = self.per_tab.lock() {
            map.insert(tab_id, epoch);
        }
        epoch
    }

    fn is_current(&self, tab_id: usize, epoch: u64) -> bool {
        self.per_tab
            .lock()
            .ok()
            .and_then(|map| map.get(&tab_id).copied())
            == Some(epoch)
    }

    #[cfg(test)]
    fn force_current(&self, tab_id: usize, epoch: u64) {
        self.global.fetch_max(epoch, Ordering::SeqCst);
        if let Ok(mut map) = self.per_tab.lock() {
            map.insert(tab_id, epoch);
        }
    }

    fn invalidate(&self, tab_id: usize) {
        if let Ok(mut map) = self.per_tab.lock() {
            map.remove(&tab_id);
        }
    }

    fn invalidate_all(&self) {
        self.global.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut map) = self.per_tab.lock() {
            map.clear();
        }
    }

    fn for_tab(&self, tab_id: usize) -> u64 {
        self.per_tab
            .lock()
            .ok()
            .and_then(|map| map.get(&tab_id).copied())
            .unwrap_or(0)
    }

    fn latest(&self) -> u64 {
        self.global.load(Ordering::SeqCst)
    }
}

/// Results emitted by the Navigation Actor back to the UI thread.
pub enum NavResult {
    /// Document pipeline finished successfully on the background actor.
    DocumentReady {
        tab_id: usize,
        prep: PreparedDocument,
        push_history: bool,
        epoch: u64,
    },
    /// Progressive first contentful paint (early snapshot while downloading).
    ProgressiveReady {
        tab_id: usize,
        prep: PreparedDocument,
        epoch: u64,
    },
    /// Navigation failed with a network or protocol error.
    Error {
        tab_id: usize,
        url: Url,
        message: String,
        push_history: bool,
        epoch: u64,
    },
}

/// Handle to the Navigation Actor owned by `BrowserChrome`.
pub struct NavigationActorHandle {
    cmd_tx: Sender<NavCommand>,
    result_rx: Receiver<NavResult>,
    #[allow(dead_code)]
    result_tx: Sender<NavResult>,
    epochs: Arc<EpochTracker>,
    _workers: Vec<JoinHandle<()>>,
}

impl NavigationActorHandle {
    /// Spawns a new navigation actor with the given resource loader.
    ///
    /// Starts a fixed pool of [`NAV_WORKER_COUNT`] workers sharing one command
    /// queue. Workers process navigations synchronously (fetch + prepare) and
    /// cooperatively abort when their epoch is superseded.
    pub fn spawn(loader: ResourceLoader) -> Self {
        let (cmd_tx, cmd_rx) = channel::<NavCommand>();
        let (result_tx, result_rx) = channel::<NavResult>();
        let epochs = Arc::new(EpochTracker::new());
        let shared_rx = Arc::new(Mutex::new(cmd_rx));

        let mut workers = Vec::with_capacity(NAV_WORKER_COUNT);
        for worker_id in 0..NAV_WORKER_COUNT {
            let shared_rx = shared_rx.clone();
            let result_tx = result_tx.clone();
            let loader = loader.clone();
            let epochs = epochs.clone();
            let handle = std::thread::Builder::new()
                .name(format!("mango-nav-worker-{worker_id}"))
                .spawn(move || {
                    Self::worker_loop(loader, shared_rx, result_tx, epochs);
                })
                .expect("failed to spawn navigation worker");
            workers.push(handle);
        }

        Self {
            cmd_tx,
            result_rx,
            result_tx,
            epochs,
            _workers: workers,
        }
    }

    fn worker_loop(
        loader: ResourceLoader,
        shared_rx: Arc<Mutex<Receiver<NavCommand>>>,
        result_tx: Sender<NavResult>,
        epochs: Arc<EpochTracker>,
    ) {
        loop {
            // Receive the next command with bounded timeout so the Mutex is not
            // held indefinitely, preventing worker starvation across the pool.
            let cmd = {
                let Ok(guard) = shared_rx.lock() else {
                    break;
                };
                match guard.recv_timeout(Duration::from_millis(50)) {
                    Ok(c) => Some(c),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            };
            let Some(cmd) = cmd else {
                continue;
            };
            match cmd {
                NavCommand::Shutdown => break,
                NavCommand::Cancel => {
                    // Epoch already bumped by `cancel()`; queued stale
                    // commands are discarded by their epoch check below.
                }
                NavCommand::Navigate {
                    tab_id,
                    url,
                    push_history,
                    epoch,
                } => {
                    if !epochs.is_current(tab_id, epoch) {
                        continue;
                    }
                    let should_cancel = {
                        let epochs = epochs.clone();
                        move || !epochs.is_current(tab_id, epoch)
                    };
                    let cancel_flag = epochs.clone();
                    let inc_parser = Arc::new(Mutex::new(mango_html::IncrementalParser::new()));
                    let early_fcp_emitted = Arc::new(std::sync::atomic::AtomicBool::new(false));

                    let inc_parser_cb = inc_parser.clone();
                    let early_fcp_cb = early_fcp_emitted.clone();
                    let loader_cb = loader.clone();
                    let url_cb = url.clone();
                    let result_tx_cb = result_tx.clone();
                    let cancel_flag_cb = cancel_flag.clone();

                    match fetch_with_timeout(
                        {
                            let loader = loader.clone();
                            let url = url.clone();
                            move || {
                                let mut boundary_tail = Vec::new();
                                let mut paint_boundary_seen = false;
                                loader.fetch_document_streaming(&url, move |chunk| {
                                    if !cancel_flag_cb.is_current(tab_id, epoch) {
                                        return false;
                                    }
                                    let _new_hints = inc_parser_cb
                                        .lock()
                                        .expect("inc_parser lock")
                                        .feed_bytes(chunk);

                                    // Take one cheap source snapshot when the
                                    // first paint boundary arrives. Parsing and
                                    // resource preparation run on a separate
                                    // bounded pool so the network reader can
                                    // continue consuming response chunks.
                                    if !early_fcp_cb.load(Ordering::Relaxed) {
                                        if !paint_boundary_seen {
                                            let mut scan = Vec::with_capacity(
                                                boundary_tail.len() + chunk.len(),
                                            );
                                            scan.extend_from_slice(&boundary_tail);
                                            scan.extend_from_slice(chunk);
                                            paint_boundary_seen =
                                                contains_ascii_case_insensitive(&scan, b"</head>")
                                                    || contains_ascii_case_insensitive(
                                                        &scan,
                                                        b"<body",
                                                    );
                                            if !paint_boundary_seen {
                                                let keep = scan.len().min(6);
                                                boundary_tail.clear();
                                                boundary_tail.extend_from_slice(
                                                    &scan[scan.len() - keep..],
                                                );
                                            }
                                        }
                                        let snapshot = {
                                            let guard =
                                                inc_parser_cb.lock().expect("inc_parser lock");
                                            let source = guard.accumulated_html().as_bytes();
                                            if source.len() < 512 || !paint_boundary_seen {
                                                None
                                            } else {
                                                Some(guard.snapshot_html())
                                            }
                                        };
                                        if let Some(snapshot) = snapshot
                                            && early_fcp_cb
                                                .compare_exchange(
                                                    false,
                                                    true,
                                                    Ordering::AcqRel,
                                                    Ordering::Relaxed,
                                                )
                                                .is_ok()
                                        {
                                            let loader = loader_cb.clone();
                                            let page_url = url_cb.clone();
                                            let result_tx = result_tx_cb.clone();
                                            let cancel_flag = cancel_flag_cb.clone();
                                            progressive_prep_pool().spawn(move || {
                                                let partial_doc =
                                                    mango_html::parse_html(&snapshot);
                                                let prep =
                                                    crate::browser::prepare_document_pipeline_from_doc(
                                                        &loader,
                                                        partial_doc,
                                                        page_url,
                                                        &|| !cancel_flag.is_current(tab_id, epoch),
                                                    );
                                                if cancel_flag.is_current(tab_id, epoch) {
                                                    let _ = result_tx.send(
                                                        NavResult::ProgressiveReady {
                                                            tab_id,
                                                            prep,
                                                            epoch,
                                                        },
                                                    );
                                                    mango_core::wake_event_loop();
                                                }
                                            });
                                        }
                                    }

                                    true
                                })
                            }
                        },
                        NAV_FETCH_TIMEOUT,
                    ) {
                        Ok(Ok(doc_res)) => {
                            if should_cancel() {
                                continue;
                            }
                            let prep = prepare_document_pipeline_cancellable(
                                &loader,
                                doc_res,
                                &should_cancel,
                            );
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::DocumentReady {
                                    tab_id,
                                    prep,
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                        Ok(Err(e)) => {
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: e.to_string(),
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                        Err(timeout_msg) => {
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: timeout_msg,
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                    }
                }
                NavCommand::Post {
                    tab_id,
                    url,
                    body,
                    content_type,
                    push_history,
                    epoch,
                } => {
                    if !epochs.is_current(tab_id, epoch) {
                        continue;
                    }
                    let should_cancel = {
                        let epochs = epochs.clone();
                        move || !epochs.is_current(tab_id, epoch)
                    };
                    match fetch_with_timeout(
                        {
                            let loader = loader.clone();
                            let url = url.clone();
                            move || loader.post_document(&url, &body, &content_type)
                        },
                        NAV_FETCH_TIMEOUT,
                    ) {
                        Ok(Ok(doc_res)) => {
                            if should_cancel() {
                                continue;
                            }
                            let prep = prepare_document_pipeline_cancellable(
                                &loader,
                                doc_res,
                                &should_cancel,
                            );
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::DocumentReady {
                                    tab_id,
                                    prep,
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                        Ok(Err(e)) => {
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: e.to_string(),
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                        Err(timeout_msg) => {
                            if epochs.is_current(tab_id, epoch) {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: timeout_msg,
                                    push_history,
                                    epoch,
                                });
                                mango_core::wake_event_loop();
                            }
                        }
                    }
                }
            }
        }
    }

    /// Requests navigation to a new URL.
    pub fn navigate(&self, tab_id: usize, url: Url, push_history: bool) {
        let epoch = self.epochs.begin(tab_id);
        let _ = self.cmd_tx.send(NavCommand::Navigate {
            tab_id,
            url,
            push_history,
            epoch,
        });
    }

    /// Requests a form POST navigation.
    pub fn post(
        &self,
        tab_id: usize,
        url: Url,
        body: Vec<u8>,
        content_type: String,
        push_history: bool,
    ) {
        let epoch = self.epochs.begin(tab_id);
        let _ = self.cmd_tx.send(NavCommand::Post {
            tab_id,
            url,
            body,
            content_type,
            push_history,
            epoch,
        });
    }

    /// Cancels any in-flight navigation.
    ///
    /// Bumps the epoch so queued stale commands are discarded without
    /// fetching and in-flight pipelines abort at their next epoch check.
    ///
    /// Note: we discard the return value of `fetch_add` (the *old* epoch) and
    /// do NOT add 1 — the post-increment value is what in-flight workers already
    /// stored in their `epoch` field, so simply making the global counter
    /// different is sufficient to invalidate them.
    pub fn cancel(&self) {
        self.epochs.invalidate_all();
        let _ = self.cmd_tx.send(NavCommand::Cancel);
    }

    /// Polls for completed navigation results without blocking.
    pub fn try_recv(&self) -> Result<NavResult, TryRecvError> {
        self.result_rx.try_recv()
    }

    /// Waits synchronously for the next navigation result.
    pub fn recv(&self) -> Result<NavResult, std::sync::mpsc::RecvError> {
        self.result_rx.recv()
    }

    /// Waits up to `timeout` for the next navigation result.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<NavResult, RecvTimeoutError> {
        self.result_rx.recv_timeout(timeout)
    }

    /// Returns the newest epoch allocated to any tab (used by diagnostics/tests).
    pub fn active_epoch(&self) -> u64 {
        self.epochs.latest()
    }

    /// Returns the latest epoch allocated to a specific tab.
    pub fn active_epoch_for(&self, tab_id: usize) -> u64 {
        self.epochs.for_tab(tab_id)
    }

    /// Returns true if `epoch` is still the current navigation for `tab_id`.
    pub fn is_current(&self, tab_id: usize, epoch: u64) -> bool {
        self.epochs.is_current(tab_id, epoch)
    }

    /// Cancels only the in-flight navigation for one tab.
    pub fn cancel_tab(&self, tab_id: usize) {
        self.epochs.invalidate(tab_id);
        let _ = self.cmd_tx.send(NavCommand::Cancel);
    }

    /// Injects a navigation result directly into the result channel for testing.
    #[cfg(test)]
    pub fn inject_result_for_test(&self, res: NavResult) {
        let (tab_id, epoch) = match &res {
            NavResult::DocumentReady { tab_id, epoch, .. }
            | NavResult::ProgressiveReady { tab_id, epoch, .. }
            | NavResult::Error { tab_id, epoch, .. } => (*tab_id, *epoch),
        };
        // Tests inject synthetic results directly; make the injected epoch the
        // current epoch for that tab so the result is not discarded as stale.
        self.epochs.force_current(tab_id, epoch);
        let _ = self.result_tx.send(res);
        mango_core::wake_event_loop();
    }

    /// Gracefully signals all navigation workers to terminate and awaits their completion.
    pub fn shutdown(&mut self) {
        self.cancel();
        for _ in 0..self._workers.len() {
            let _ = self.cmd_tx.send(NavCommand::Shutdown);
        }
        while let Some(worker) = self._workers.pop() {
            let _ = worker.join();
        }
    }
}

impl Drop for NavigationActorHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_actor_data_uri_and_cancellation() {
        let loader = ResourceLoader::new();
        let actor = NavigationActorHandle::spawn(loader);

        let initial_epoch = actor.active_epoch();
        assert!(initial_epoch >= 1);

        // Cancel increments epoch
        actor.cancel();
        assert_eq!(actor.active_epoch(), initial_epoch + 1);

        // Navigate to a data: HTML URL
        let url =
            Url::parse("data:text/html,<html><body><h1>Actor Test</h1></body></html>").unwrap();
        actor.navigate(0, url.clone(), true);

        // Wait for result
        loop {
            match actor.recv().expect("actor should return NavResult") {
                NavResult::ProgressiveReady { .. } => {
                    continue;
                }
                NavResult::DocumentReady {
                    prep,
                    push_history,
                    epoch,
                    ..
                } => {
                    assert!(push_history);
                    assert_eq!(epoch, actor.active_epoch());
                    assert!(prep.html.contains("Actor Test"));
                    break;
                }
                NavResult::Error { message, .. } => {
                    panic!("Navigation failed unexpectedly: {message}");
                }
            }
        }
    }

    #[test]
    fn test_navigation_actor_superseded_request_ignored() {
        let loader = ResourceLoader::new();
        let actor = NavigationActorHandle::spawn(loader);

        let url1 = Url::parse("data:text/html,<html><body><h1>Page 1</h1></body></html>").unwrap();
        let url2 = Url::parse("data:text/html,<html><body><h1>Page 2</h1></body></html>").unwrap();

        // Dispatch page 1 then immediately supersede with page 2
        actor.navigate(0, url1, true);
        actor.navigate(0, url2, true);

        // Receive result(s) until the latest epoch arrives; stale Page 1
        // results (if any) are discarded.
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let latest_epoch = actor.active_epoch();
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for page 2"
            );
            let res = actor
                .recv_timeout(Duration::from_secs(10))
                .expect("actor should deliver page 2");
            match res {
                NavResult::ProgressiveReady { .. } => continue,
                NavResult::DocumentReady { prep, epoch, .. } => {
                    if epoch != latest_epoch {
                        continue;
                    }
                    assert!(prep.html.contains("Page 2"));
                    break;
                }
                NavResult::Error { message, epoch, .. } => {
                    if epoch != latest_epoch {
                        continue;
                    }
                    panic!("Unexpected error: {message}");
                }
            }
        }
    }
    #[test]
    fn test_navigating_one_tab_does_not_cancel_another() {
        let loader = ResourceLoader::new();
        let actor = NavigationActorHandle::spawn(loader);

        let url_a = Url::parse("data:text/html,<html><body><h1>Tab A</h1></body></html>").unwrap();
        let url_b = Url::parse("data:text/html,<html><body><h1>Tab B</h1></body></html>").unwrap();

        actor.navigate(0, url_a, true);
        let epoch_a = actor.active_epoch_for(0);
        actor.navigate(1, url_b, true);
        let epoch_b = actor.active_epoch_for(1);

        assert!(
            actor.is_current(0, epoch_a),
            "navigating tab 1 must not invalidate tab 0"
        );
        assert!(actor.is_current(1, epoch_b));

        let mut saw_a = false;
        let mut saw_b = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !(saw_a && saw_b) {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for both tab navigations"
            );
            match actor
                .recv_timeout(Duration::from_secs(10))
                .expect("both tab navigations should complete")
            {
                NavResult::ProgressiveReady { .. } => {}
                NavResult::DocumentReady {
                    tab_id,
                    prep,
                    epoch,
                    ..
                } => {
                    assert_eq!(epoch, actor.active_epoch_for(tab_id));
                    match tab_id {
                        0 => {
                            assert!(prep.html.contains("Tab A"));
                            saw_a = true;
                        }
                        1 => {
                            assert!(prep.html.contains("Tab B"));
                            saw_b = true;
                        }
                        other => panic!("unexpected tab id {other}"),
                    }
                }
                NavResult::Error { message, .. } => panic!("navigation failed: {message}"),
            }
        }
    }
}
