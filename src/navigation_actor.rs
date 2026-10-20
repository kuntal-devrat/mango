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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::browser::{PreparedDocument, prepare_document_pipeline_cancellable};
use mango_net::{ResourceLoader, Url};

/// Number of background navigation workers (fixed pool, no per-nav spawn).
pub const NAV_WORKER_COUNT: usize = 4;
/// Per-navigation fetch timeout applied inside the pipeline stages.
pub const NAV_FETCH_TIMEOUT: Duration = Duration::from_secs(60);

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
    current_epoch: Arc<AtomicU64>,
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
        let current_epoch = Arc::new(AtomicU64::new(1));
        let shared_rx = Arc::new(Mutex::new(cmd_rx));

        let mut workers = Vec::with_capacity(NAV_WORKER_COUNT);
        for worker_id in 0..NAV_WORKER_COUNT {
            let shared_rx = shared_rx.clone();
            let result_tx = result_tx.clone();
            let loader = loader.clone();
            let epoch_flag = current_epoch.clone();
            let handle = std::thread::Builder::new()
                .name(format!("mango-nav-worker-{worker_id}"))
                .spawn(move || {
                    Self::worker_loop(loader, shared_rx, result_tx, epoch_flag);
                })
                .expect("failed to spawn navigation worker");
            workers.push(handle);
        }

        Self {
            cmd_tx,
            result_rx,
            result_tx,
            current_epoch,
            _workers: workers,
        }
    }

    fn worker_loop(
        loader: ResourceLoader,
        shared_rx: Arc<Mutex<Receiver<NavCommand>>>,
        result_tx: Sender<NavResult>,
        epoch_flag: Arc<AtomicU64>,
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
                    if epoch_flag.load(Ordering::SeqCst) != epoch {
                        continue;
                    }
                    let should_cancel = {
                        let flag = epoch_flag.clone();
                        move || flag.load(Ordering::SeqCst) != epoch
                    };
                    let cancel_flag = epoch_flag.clone();
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
                                loader.fetch_document_streaming(&url, move |chunk| {
                                    if cancel_flag_cb.load(Ordering::SeqCst) != epoch {
                                        return false;
                                    }
                                    let _new_hints = inc_parser_cb
                                        .lock()
                                        .expect("inc_parser lock")
                                        .feed_bytes(chunk);

                                    // First Contentful Paint: check if <head> completed or <body> started
                                    if !early_fcp_cb.swap(true, Ordering::SeqCst) {
                                        let accumulated = {
                                            let guard =
                                                inc_parser_cb.lock().expect("inc_parser lock");
                                            guard.accumulated_html().to_ascii_lowercase()
                                        };
                                        if accumulated.len() >= 512
                                            && (accumulated.contains("</head>")
                                                || accumulated.contains("<body"))
                                        {
                                            let partial_doc = inc_parser_cb
                                                .lock()
                                                .expect("inc_parser lock")
                                                .current_document();
                                            let prep =
                                                crate::browser::prepare_document_pipeline_from_doc(
                                                    &loader_cb,
                                                    partial_doc,
                                                    url_cb.clone(),
                                                    &|| {
                                                        cancel_flag_cb.load(Ordering::SeqCst)
                                                            != epoch
                                                    },
                                                );
                                            if cancel_flag_cb.load(Ordering::SeqCst) == epoch {
                                                let _ = result_tx_cb.send(
                                                    NavResult::ProgressiveReady {
                                                        tab_id,
                                                        prep,
                                                        epoch,
                                                    },
                                                );
                                            }
                                        } else {
                                            early_fcp_cb.store(false, Ordering::SeqCst);
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
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::DocumentReady {
                                    tab_id,
                                    prep,
                                    push_history,
                                    epoch,
                                });
                            }
                        }
                        Ok(Err(e)) => {
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: e.to_string(),
                                    push_history,
                                    epoch,
                                });
                            }
                        }
                        Err(timeout_msg) => {
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: timeout_msg,
                                    push_history,
                                    epoch,
                                });
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
                    if epoch_flag.load(Ordering::SeqCst) != epoch {
                        continue;
                    }
                    let should_cancel = {
                        let flag = epoch_flag.clone();
                        move || flag.load(Ordering::SeqCst) != epoch
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
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::DocumentReady {
                                    tab_id,
                                    prep,
                                    push_history,
                                    epoch,
                                });
                            }
                        }
                        Ok(Err(e)) => {
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: e.to_string(),
                                    push_history,
                                    epoch,
                                });
                            }
                        }
                        Err(timeout_msg) => {
                            if epoch_flag.load(Ordering::SeqCst) == epoch {
                                let _ = result_tx.send(NavResult::Error {
                                    tab_id,
                                    url,
                                    message: timeout_msg,
                                    push_history,
                                    epoch,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Requests navigation to a new URL.
    pub fn navigate(&self, tab_id: usize, url: Url, push_history: bool) {
        let epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;
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
        let epoch = self.current_epoch.fetch_add(1, Ordering::SeqCst) + 1;
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
        self.current_epoch.fetch_add(1, Ordering::SeqCst);
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

    /// Returns the current active epoch.
    pub fn active_epoch(&self) -> u64 {
        self.current_epoch.load(Ordering::SeqCst)
    }

    /// Injects a navigation result directly into the result channel for testing.
    #[cfg(test)]
    pub fn inject_result_for_test(&self, res: NavResult) {
        let _ = self.result_tx.send(res);
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

/// Runs a blocking fetch on a short-lived thread bounded by `timeout`.
///
/// Returns `Err(message)` on timeout or thread panic instead of hanging the
/// navigation worker forever.
fn fetch_with_timeout<T, F>(f: F, timeout: Duration) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("mango-nav-fetch".to_string())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| format!("failed to spawn fetch thread: {e}"))?;
    rx.recv_timeout(timeout).map_err(|e| match e {
        RecvTimeoutError::Timeout => {
            format!("navigation fetch timed out after {:?}", timeout)
        }
        RecvTimeoutError::Disconnected => "navigation fetch worker disconnected".to_string(),
    })
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
}
