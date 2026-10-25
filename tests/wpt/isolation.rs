//! Process isolation for WPT file execution.
//!
//! `wptrunner` executes every WPT file in its own browser process so that a
//! crashing or hanging test cannot take down the run. Mango does the same:
//! the `wpt-runner` binary is invoked as a child process for each batch of
//! files, and any file that fails to report back (because it crashed the
//! process) is re-run in isolation to capture its exact failure mode.
//!
//! This module is only used by the integration tests; the `wpt-runner` binary
//! itself runs the harness in-process.

use crate::harness::{WptFileResult, WptFileStatus};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Number of files executed in a single child process before falling back to
/// one process per file. Large enough to amortise process start-up, small
/// enough that a crash only costs one batch re-run.
const BATCH_SIZE: usize = 25;

/// Returns the path of the `wpt-runner` binary to spawn.
///
/// Cargo exposes `CARGO_BIN_EXE_<name>` to integration tests. If it is not
/// available (for example when the harness is used from a unit test), the
/// caller falls back to running in-process.
pub fn runner_binary() -> Option<&'static str> {
    option_env!("CARGO_BIN_EXE_wpt-runner")
}

/// Number of child processes to run concurrently.
fn parallelism() -> usize {
    if let Ok(val) = std::env::var("WPT_PARALLELISM")
        && let Ok(n) = val.parse::<usize>()
    {
        return n.max(1);
    }
    if let Ok(val) = std::env::var("RUST_TEST_THREADS")
        && let Ok(n) = val.parse::<usize>()
    {
        return n.max(1);
    }
    1
}

static STREAM_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Writes raw bytes directly to file descriptor 1 (stdout), bypassing Rust
/// libtest's thread-local capture buffer so output streams live to the terminal
/// even when `--nocapture` is not passed.
pub fn stream_write(msg: &str) {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::io::FromRawFd;
        // SAFETY: File descriptor 1 is stdout, standard open file descriptor.
        // We write to it and immediately invoke `std::mem::forget(file)` to ensure fd 1 is not closed when `file` is dropped.
        let mut file = unsafe { std::fs::File::from_raw_fd(1) };
        let _ = file.write_all(msg.as_bytes());
        let _ = file.flush();
        std::mem::forget(file);
    }
    #[cfg(not(unix))]
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(msg.as_bytes());
        let _ = std::io::stdout().flush();
    }
}

/// Streams a single test file's pass/fail status immediately to the terminal.
pub fn stream_result(r: &WptFileResult) {
    let _guard = STREAM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut line = String::new();
    match r.status {
        WptFileStatus::Pass => {
            let count = r.subtests.len();
            line.push_str(&format!(
                "  \x1b[32m✓ PASS\x1b[0m: {} ({} assertion{}, {:.1?})\n",
                r.path,
                count,
                if count == 1 { "" } else { "s" },
                r.duration
            ));
        }
        WptFileStatus::Fail => {
            let detail = r
                .subtests
                .iter()
                .find(|t| !t.is_pass())
                .map(|t| {
                    if let Some(msg) = &t.message {
                        format!("{}: {}", t.name, msg)
                    } else {
                        format!("{}: {}", t.name, t.status)
                    }
                })
                .or_else(|| r.message.clone())
                .unwrap_or_else(|| "assertion failed".to_string());
            let first_line = detail.lines().next().unwrap_or(&detail);
            line.push_str(&format!(
                "  \x1b[31m✗ FAIL\x1b[0m: {} — {}\n",
                r.path, first_line
            ));
        }
        WptFileStatus::Error => {
            let msg = r.message.as_deref().unwrap_or("error");
            let first_line = msg.lines().next().unwrap_or(msg);
            line.push_str(&format!(
                "  \x1b[33m⚠ ERROR\x1b[0m: {} — {}\n",
                r.path, first_line
            ));
        }
        WptFileStatus::Timeout => {
            line.push_str(&format!(
                "  \x1b[35m⏱ TIMEOUT\x1b[0m: {} ({:.1?})\n",
                r.path, r.duration
            ));
        }
        WptFileStatus::Skipped => {
            line.push_str(&format!("  \x1b[90m○ SKIP\x1b[0m: {}\n", r.path));
        }
        _ => {
            line.push_str(&format!("  [{}] {}\n", r.status.as_str(), r.path));
        }
    }
    stream_write(&line);
}

/// Runs a list of WPT files, isolating them in child processes.
///
/// Returns one result per input file, in the same order. Files whose child
/// process crashed are reported with [`WptFileStatus::Error`].
pub fn run_files_isolated(root: &Path, files: &[String]) -> Vec<WptFileResult> {
    let Some(bin) = runner_binary() else {
        // No child process available: run everything in-process.
        return files
            .iter()
            .map(|f| {
                let r = crate::harness::run_wpt_file(root, f, crate::harness::DEFAULT_FILE_TIMEOUT);
                stream_result(&r);
                r
            })
            .collect();
    };

    // Split the work into batches and run the batches concurrently.
    let batches: Vec<&[String]> = files.chunks(BATCH_SIZE).collect();
    let workers = parallelism().min(batches.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Vec<WptFileResult>>> =
        std::sync::Mutex::new((0..batches.len()).map(|_| Vec::new()).collect());

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let idx = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if idx >= batches.len() {
                        break;
                    }
                    let batch = batches[idx];
                    let batch_results = run_batch_isolated(bin, root, batch);
                    let mut guard = results.lock().unwrap();
                    guard[idx] = batch_results;
                }
            });
        }
    });

    let mut out = Vec::with_capacity(files.len());
    for batch_results in results.into_inner().unwrap() {
        out.extend(batch_results);
    }
    out
}

/// Runs a batch of files, re-running individually any file that crashed the
/// batch process.
fn run_batch_isolated(bin: &str, root: &Path, files: &[String]) -> Vec<WptFileResult> {
    match run_batch(bin, root, files) {
        Some(mut batch_results) => {
            // A batch that produced fewer results than requested means at
            // least one file crashed the process. Re-run the missing ones
            // individually so that the failure is attributed correctly.
            let missing: Vec<String> = files
                .iter()
                .filter(|f| !batch_results.iter().any(|r| r.path == **f))
                .cloned()
                .collect();
            for f in missing {
                batch_results.push(run_single(bin, root, &f));
            }
            batch_results
        }
        None => {
            // The whole batch died; re-run every file individually.
            files.iter().map(|f| run_single(bin, root, f)).collect()
        }
    }
}

/// Runs a batch of files in one child process, streaming results line-by-line.
/// Returns `None` if the child process itself failed without producing any output.
fn run_batch(bin: &str, root: &Path, files: &[String]) -> Option<Vec<WptFileResult>> {
    let list_path = std::env::temp_dir().join(format!(
        "mango-wpt-batch-{}-{:?}.txt",
        std::process::id(),
        files.first().map(|f| f.len()).unwrap_or(0)
    ));
    {
        let mut f = std::fs::File::create(&list_path).ok()?;
        for file in files {
            let _ = writeln!(f, "{file}");
        }
    }

    let mut child = Command::new(bin)
        .arg("--json-batch")
        .arg(&list_path)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut results = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(r) = serde_json::from_str::<WptFileResult>(line) {
                stream_result(&r);
                results.push(r);
            }
        }
    }

    let status = child.wait().ok()?;
    let _ = std::fs::remove_file(&list_path);

    if !status.success() && results.is_empty() {
        return None;
    }

    Some(results)
}

/// Runs a single file in its own child process.
fn run_single(bin: &str, root: &Path, file: &str) -> WptFileResult {
    let output = Command::new(bin)
        .arg("--json")
        .arg(file)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let res = match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut found = None;
            for line in text.lines().rev() {
                let line = line.trim();
                if line.starts_with('{')
                    && let Ok(r) = serde_json::from_str::<WptFileResult>(line)
                {
                    found = Some(r);
                    break;
                }
            }
            found.unwrap_or_else(|| WptFileResult {
                path: file.to_string(),
                status: WptFileStatus::Error,
                subtests: Vec::new(),
                message: Some(format!(
                    "test process terminated abnormally ({}): {}",
                    out.status,
                    text.lines().last().unwrap_or("no output")
                )),
                duration: std::time::Duration::ZERO,
            })
        }
        Err(e) => WptFileResult {
            path: file.to_string(),
            status: WptFileStatus::Error,
            subtests: Vec::new(),
            message: Some(format!("could not spawn test process: {e}")),
            duration: std::time::Duration::ZERO,
        },
    };
    stream_result(&res);
    res
}
