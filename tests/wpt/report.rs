//! Data models and JSON serializer for WPT test results compatible with wpt.fyi / wptrunner reports.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtestResult {
    pub name: String,
    pub status: String, // "PASS", "FAIL", "TIMEOUT", "NOTRUN", "PRECONDITION_FAILED"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFileResult {
    pub test: String,
    pub status: String, // "OK", "ERROR", "TIMEOUT", "CRASH"
    pub subtests: Vec<SubtestResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub product: String,
    pub browser_version: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WptReport {
    pub run_info: RunInfo,
    pub time_start: u64,
    pub time_end: u64,
    pub results: Vec<TestFileResult>,
}

impl WptReport {
    pub fn new(product: &str, browser_version: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            run_info: RunInfo {
                product: product.to_string(),
                browser_version: browser_version.to_string(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
            },
            time_start: now,
            time_end: now,
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: TestFileResult) {
        self.results.push(result);
        self.time_end = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    pub fn total_subtests(&self) -> usize {
        self.results.iter().map(|r| r.subtests.len()).sum()
    }

    pub fn passed_subtests(&self) -> usize {
        self.results
            .iter()
            .flat_map(|r| &r.subtests)
            .filter(|s| s.status == "PASS")
            .count()
    }

    pub fn failed_subtests(&self) -> usize {
        self.results
            .iter()
            .flat_map(|r| &r.subtests)
            .filter(|s| s.status == "FAIL" || s.status == "TIMEOUT")
            .count()
    }

    pub fn pass_percentage(&self) -> f64 {
        let total = self.total_subtests();
        if total == 0 {
            0.0
        } else {
            (self.passed_subtests() as f64 / total as f64) * 100.0
        }
    }

    pub fn write_json(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(path, json)
    }
}
