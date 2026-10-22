//! Expectation-based WPT baseline tracking and regression detection.
//!
//! Mirrors Servo's WPT metadata architecture using modular JSON baseline files
//! organized by test suite (e.g. `tests/wpt/expectations/css/css-cascade.json`).
//!
//! A test run compares live WPT execution results against these baselines:
//! - **Expected Pass**: Verified passing behavior matches baseline.
//! - **Expected Fail**: Known spec gap (XFAIL); passes CI without false alarms.
//! - **Regression**: Test or subtest previously passing now fails -> **CI Failure**.
//! - **Unexpected Pass**: Test previously failing now passes -> **Alerts developer to update baseline**.

use crate::harness::{WptFileResult, WptFileStatus, WptSuiteResult};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Expected outcome for a single WPT test file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileExpectation {
    pub status: WptFileStatus,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub subtests: BTreeMap<String, String>,
}

/// Modular expectation file for a specific WPT suite or directory.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SuiteExpectations {
    pub suite: String,
    pub files: BTreeMap<String, FileExpectation>,
}

/// Outcome of comparing a live test result against expectations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOutcome {
    ExpectedPass,
    ExpectedFail,
    UnexpectedPass { details: String },
    Regression { details: String },
    MissingBaseline,
}

/// Comprehensive audit summary over an entire test suite.
#[derive(Debug, Clone, Default)]
pub struct ExpectationAuditResult {
    pub total: usize,
    pub expected_passes: usize,
    pub expected_fails: usize,
    pub regressions: Vec<String>,
    pub unexpected_passes: Vec<String>,
    pub missing_baselines: Vec<String>,
}

impl ExpectationAuditResult {
    /// True if no regressions occurred and no unexpected passes need committing.
    pub fn is_clean(&self) -> bool {
        self.regressions.is_empty() && self.unexpected_passes.is_empty()
    }

    /// Formats a human-readable audit report.
    pub fn summary(&self, title: &str) -> String {
        let mut out = String::new();
        out.push_str(
            "\n────────────────────────────────────────────────────────────────────────────────\n",
        );
        out.push_str(&format!("📋 WPT EXPECTATIONS AUDIT: {title}\n"));
        out.push_str(
            "────────────────────────────────────────────────────────────────────────────────\n",
        );
        out.push_str(&format!("Total tests audited   : {}\n", self.total));
        out.push_str(&format!(
            "  ✓ Expected Passes   : {}\n",
            self.expected_passes
        ));
        out.push_str(&format!(
            "  - Known Fails (XFAIL): {}\n",
            self.expected_fails
        ));

        if !self.unexpected_passes.is_empty() {
            out.push_str(&format!(
                "  ✨ Unexpected Passes : {} (run with --update-expectations to update baseline)\n",
                self.unexpected_passes.len()
            ));
            for up in self.unexpected_passes.iter().take(10) {
                out.push_str(&format!("     + {up}\n"));
            }
            if self.unexpected_passes.len() > 10 {
                out.push_str(&format!(
                    "     … and {} more\n",
                    self.unexpected_passes.len() - 10
                ));
            }
        }

        if !self.regressions.is_empty() {
            out.push_str(&format!(
                "  ❌ REGRESSIONS       : {} (must be fixed before merging!)\n",
                self.regressions.len()
            ));
            for reg in self.regressions.iter().take(15) {
                out.push_str(&format!("     ! {reg}\n"));
            }
            if self.regressions.len() > 15 {
                out.push_str(&format!(
                    "     … and {} more\n",
                    self.regressions.len() - 15
                ));
            }
        }

        if !self.missing_baselines.is_empty() {
            out.push_str(&format!(
                "  ? Missing baselines  : {}\n",
                self.missing_baselines.len()
            ));
        }
        out.push_str(
            "────────────────────────────────────────────────────────────────────────────────\n",
        );
        out
    }
}

/// Store holding all modular suite expectation JSON files.
#[derive(Debug, Clone, Default)]
pub struct ExpectationStore {
    pub suites: BTreeMap<String, SuiteExpectations>,
}

impl ExpectationStore {
    /// Loads all modular `.json` expectation files from the given root directory.
    pub fn load_from_dir(dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut store = Self::default();
        if !dir.exists() {
            return Ok(store);
        }

        let mut queue = vec![dir.to_path_buf()];
        while let Some(current_dir) = queue.pop() {
            let entries = match fs::read_dir(&current_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    queue.push(path);
                } else if path.is_file()
                    && path.extension().and_then(|s| s.to_str()) == Some("json")
                    && let Ok(content) = fs::read_to_string(&path)
                    && let Ok(suite_exp) = serde_json::from_str::<SuiteExpectations>(&content)
                {
                    store.suites.insert(suite_exp.suite.clone(), suite_exp);
                }
            }
        }
        Ok(store)
    }

    /// Derives the modular suite identifier from a test relative path.
    ///
    /// For example:
    /// `css/css-cascade/scope-proximity.html` -> `"css/css-cascade"`
    /// `css/css-text-decor/parsing/text-decoration-line-valid.html` -> `"css/css-text-decor"`
    /// `dom/nodes/CharacterData-appendData.html` -> `"dom/nodes"`
    pub fn suite_name_for_path(path: &str) -> String {
        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        if parts.len() >= 2 {
            format!("{}/{}", parts[0], parts[1])
        } else if let Some(first) = parts.first() {
            first.to_string()
        } else {
            "root".to_string()
        }
    }

    /// Look up expected result for a single test path.
    pub fn get_file_expectation(&self, path: &str) -> Option<&FileExpectation> {
        let suite_name = Self::suite_name_for_path(path);
        let suite = self.suites.get(&suite_name)?;
        let normalized = path.replace('\\', "/");
        suite.files.get(&normalized)
    }

    /// Compares a live test file result against the stored expectations.
    pub fn compare(&self, result: &WptFileResult) -> ComparisonOutcome {
        let Some(exp) = self.get_file_expectation(&result.path) else {
            return ComparisonOutcome::MissingBaseline;
        };

        // File-level status check
        let actual_status = result.status.clone();
        let expected_status = exp.status.clone();

        if expected_status == WptFileStatus::Pass && actual_status != WptFileStatus::Pass {
            return ComparisonOutcome::Regression {
                details: format!(
                    "{} regressed: expected {:?}, got {:?}",
                    result.path, expected_status, actual_status
                ),
            };
        }

        if expected_status != WptFileStatus::Pass && actual_status == WptFileStatus::Pass {
            return ComparisonOutcome::UnexpectedPass {
                details: format!(
                    "{} unexpectedly passed completely (expected {:?})",
                    result.path, expected_status
                ),
            };
        }

        // Subtest-level check
        let mut subtest_regressions = Vec::new();
        let mut subtest_unexpected_passes = Vec::new();

        for sub in &result.subtests {
            if let Some(expected_sub_status) = exp.subtests.get(&sub.name) {
                let actual_sub_status = &sub.status;
                if expected_sub_status == "PASS" && actual_sub_status != "PASS" {
                    subtest_regressions.push(format!(
                        "{} :: \"{}\" regressed from PASS to {}",
                        result.path, sub.name, actual_sub_status
                    ));
                } else if expected_sub_status != "PASS" && actual_sub_status == "PASS" {
                    subtest_unexpected_passes.push(format!(
                        "{} :: \"{}\" unexpectedly PASSED (was {})",
                        result.path, sub.name, expected_sub_status
                    ));
                }
            }
        }

        if !subtest_regressions.is_empty() {
            return ComparisonOutcome::Regression {
                details: subtest_regressions.join("; "),
            };
        }

        if !subtest_unexpected_passes.is_empty() {
            return ComparisonOutcome::UnexpectedPass {
                details: subtest_unexpected_passes.join("; "),
            };
        }

        if expected_status == WptFileStatus::Pass {
            ComparisonOutcome::ExpectedPass
        } else {
            ComparisonOutcome::ExpectedFail
        }
    }

    /// Audits all files in a test suite against expectations.
    pub fn audit_suite(&self, suite: &WptSuiteResult) -> ExpectationAuditResult {
        let mut audit = ExpectationAuditResult::default();
        for res in &suite.results {
            if !res.counts_as_run() {
                continue;
            }
            audit.total += 1;
            match self.compare(res) {
                ComparisonOutcome::ExpectedPass => audit.expected_passes += 1,
                ComparisonOutcome::ExpectedFail => audit.expected_fails += 1,
                ComparisonOutcome::Regression { details } => audit.regressions.push(details),
                ComparisonOutcome::UnexpectedPass { details } => {
                    audit.unexpected_passes.push(details)
                }
                ComparisonOutcome::MissingBaseline => {
                    audit.missing_baselines.push(res.path.clone())
                }
            }
        }
        audit
    }

    /// Inserts or updates the expectation entry for a test file.
    pub fn record_result(&mut self, result: &WptFileResult) {
        let suite_name = Self::suite_name_for_path(&result.path);
        let normalized = result.path.replace('\\', "/");

        let mut subtests = BTreeMap::new();
        for s in &result.subtests {
            subtests.insert(s.name.clone(), s.status.clone());
        }

        let file_exp = FileExpectation {
            status: result.status.clone(),
            subtests,
        };

        let suite = self
            .suites
            .entry(suite_name.clone())
            .or_insert_with(|| SuiteExpectations {
                suite: suite_name,
                files: BTreeMap::new(),
            });

        suite.files.insert(normalized, file_exp);
    }

    /// Saves all modular `.json` expectation files to the target directory.
    pub fn save_to_dir(&self, root_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(root_dir)?;
        for (suite_name, suite_data) in &self.suites {
            let relative_json_path = format!("{}.json", suite_name);
            let target_path = root_dir.join(relative_json_path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let serialized = serde_json::to_string_pretty(suite_data)?;
            fs::write(&target_path, serialized)?;
        }
        Ok(())
    }
}
