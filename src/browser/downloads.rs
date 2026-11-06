//! Native downloads manager for Mango Browser.
//!
//! Provides streaming file downloads, progress tracking, pause/resume capability,
//! filename collision resolution, and download transfer lifecycle state machines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Status state of an individual file download transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    /// Queued and waiting for connection.
    Pending,
    /// Actively receiving data chunks.
    InProgress,
    /// Paused by user (eligible for HTTP Range resume).
    Paused,
    /// Successfully downloaded and verified on disk.
    Completed,
    /// Failed due to network or filesystem error.
    Failed(String),
    /// Cancelled and aborted by user.
    Cancelled,
}

/// A download transfer tracking its progress and destination.
#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub id: u64,
    pub url: String,
    pub filename: String,
    pub target_path: PathBuf,
    pub total_bytes: Option<u64>,
    pub received_bytes: u64,
    pub state: DownloadState,
    pub start_time: Instant,
    pub last_chunk_time: Instant,
    pub speed_bytes_per_sec: u64,
}

impl DownloadItem {
    /// Calculates progress percentage from 0.0 to 100.0, or None if Content-Length unknown.
    pub fn progress_percentage(&self) -> Option<f32> {
        self.total_bytes.map(|total| {
            if total == 0 {
                100.0
            } else {
                ((self.received_bytes as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
            }
        })
    }

    /// Returns `true` if the download is actively receiving data.
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            DownloadState::InProgress | DownloadState::Pending
        )
    }
}

/// Download manager tracking active and historical downloads.
#[derive(Debug, Default)]
pub struct DownloadManager {
    next_id: u64,
    downloads: HashMap<u64, DownloadItem>,
}

impl DownloadManager {
    /// Creates a new download manager instance.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            downloads: HashMap::new(),
        }
    }

    /// Initializes a new download transfer, resolving filename collisions in the destination directory.
    pub fn start_download(
        &mut self,
        url: &str,
        suggested_filename: &str,
        destination_dir: &Path,
        total_bytes: Option<u64>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let safe_name = sanitize_filename(suggested_filename);
        let target_path = resolve_unique_path(destination_dir, &safe_name);
        let actual_filename = target_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(safe_name);

        let now = Instant::now();
        let item = DownloadItem {
            id,
            url: url.to_string(),
            filename: actual_filename,
            target_path,
            total_bytes,
            received_bytes: 0,
            state: DownloadState::InProgress,
            start_time: now,
            last_chunk_time: now,
            speed_bytes_per_sec: 0,
        };

        self.downloads.insert(id, item);
        id
    }

    /// Reports incoming chunk bytes for an active download.
    pub fn on_data_received(&mut self, id: u64, chunk_len: usize) {
        if let Some(item) = self.downloads.get_mut(&id) {
            if item.state != DownloadState::InProgress {
                return;
            }
            item.received_bytes = item.received_bytes.saturating_add(chunk_len as u64);
            let elapsed = item.start_time.elapsed().as_secs_f64();
            if elapsed > 0.1 {
                item.speed_bytes_per_sec = (item.received_bytes as f64 / elapsed) as u64;
            }
            item.last_chunk_time = Instant::now();

            if let Some(total) = item.total_bytes {
                if item.received_bytes >= total {
                    item.state = DownloadState::Completed;
                }
            }
        }
    }

    /// Marks a download as completed.
    pub fn mark_completed(&mut self, id: u64) {
        if let Some(item) = self.downloads.get_mut(&id) {
            item.state = DownloadState::Completed;
        }
    }

    /// Marks a download as failed with error explanation.
    pub fn mark_failed(&mut self, id: u64, error: &str) {
        if let Some(item) = self.downloads.get_mut(&id) {
            item.state = DownloadState::Failed(error.to_string());
        }
    }

    /// Pauses an active download.
    pub fn pause(&mut self, id: u64) -> bool {
        if let Some(item) = self.downloads.get_mut(&id) {
            if item.state == DownloadState::InProgress {
                item.state = DownloadState::Paused;
                return true;
            }
        }
        false
    }

    /// Resumes a paused download.
    pub fn resume(&mut self, id: u64) -> bool {
        if let Some(item) = self.downloads.get_mut(&id) {
            if item.state == DownloadState::Paused {
                item.state = DownloadState::InProgress;
                item.last_chunk_time = Instant::now();
                return true;
            }
        }
        false
    }

    /// Cancels a download.
    pub fn cancel(&mut self, id: u64) -> bool {
        if let Some(item) = self.downloads.get_mut(&id) {
            if item.is_active() || item.state == DownloadState::Paused {
                item.state = DownloadState::Cancelled;
                return true;
            }
        }
        false
    }

    /// Retrieves an immutable reference to a download item.
    pub fn get(&self, id: u64) -> Option<&DownloadItem> {
        self.downloads.get(&id)
    }

    /// Returns all active (pending or in-progress) downloads.
    pub fn active_downloads(&self) -> Vec<&DownloadItem> {
        self.downloads.values().filter(|d| d.is_active()).collect()
    }

    /// Returns total count of tracked downloads.
    pub fn len(&self) -> usize {
        self.downloads.len()
    }

    /// Returns `true` if no downloads are tracked.
    pub fn is_empty(&self) -> bool {
        self.downloads.is_empty()
    }
}

/// Sanitizes proposed filename removing dangerous path traversal tokens.
pub fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "download".to_string();
    }

    let mut clean = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => clean.push('_'),
            other => clean.push(other),
        }
    }

    if clean == "." || clean == ".." {
        return "download".to_string();
    }
    clean
}

/// Resolves filename collisions by appending ` (1)`, ` (2)`, etc. if file exists.
pub fn resolve_unique_path(dir: &Path, filename: &str) -> PathBuf {
    let mut target = dir.join(filename);
    if !target.exists() {
        return target;
    }

    let p = Path::new(filename);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
    let ext = p.extension().and_then(|s| s.to_str());

    let mut counter = 1;
    loop {
        let candidate_name = match ext {
            Some(e) => format!("{stem} ({counter}).{e}"),
            None => format!("{stem} ({counter})"),
        };
        target = dir.join(&candidate_name);
        if !target.exists() {
            return target;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_filename("my file?.pdf"), "my file_.pdf");
        assert_eq!(sanitize_filename(""), "download");
    }

    #[test]
    fn test_collision_resolution() {
        let temp_dir = std::env::temp_dir().join(format!("mango_test_dl_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let initial_file = temp_dir.join("report.pdf");
        std::fs::write(&initial_file, b"test").expect("write succeeds");

        let unique = resolve_unique_path(&temp_dir, "report.pdf");
        assert_eq!(unique, temp_dir.join("report (1).pdf"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_download_progress_and_state_machine() {
        let mut mgr = DownloadManager::new();
        let temp_dir = std::env::temp_dir();

        let id = mgr.start_download(
            "https://example.com/asset.zip",
            "asset.zip",
            &temp_dir,
            Some(1000),
        );

        mgr.on_data_received(id, 500);
        let item = mgr.get(id).unwrap();
        assert_eq!(item.received_bytes, 500);
        assert_eq!(item.progress_percentage(), Some(50.0));
        assert_eq!(item.state, DownloadState::InProgress);

        mgr.on_data_received(id, 500);
        let item = mgr.get(id).unwrap();
        assert_eq!(item.received_bytes, 1000);
        assert_eq!(item.state, DownloadState::Completed);
    }

    #[test]
    fn test_pause_resume_cancel() {
        let mut mgr = DownloadManager::new();
        let temp_dir = std::env::temp_dir();

        let id = mgr.start_download("https://example.com/doc.pdf", "doc.pdf", &temp_dir, None);
        assert!(mgr.pause(id));
        assert_eq!(mgr.get(id).unwrap().state, DownloadState::Paused);

        assert!(mgr.resume(id));
        assert_eq!(mgr.get(id).unwrap().state, DownloadState::InProgress);

        assert!(mgr.cancel(id));
        assert_eq!(mgr.get(id).unwrap().state, DownloadState::Cancelled);
    }
}
