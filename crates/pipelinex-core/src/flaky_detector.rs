use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a test execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub timestamp: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

/// Represents a flaky test detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyTest {
    pub name: String,
    pub flakiness_score: f64,
    pub total_runs: usize,
    pub failures: usize,
    pub passes: usize,
    pub failure_rate: f64,
    pub recent_failures: Vec<String>,
    pub category: FlakyCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlakyCategory {
    /// Test fails <50% of the time (intermittent)
    Intermittent,
    /// Test alternates between pass/fail without code changes
    Unstable,
    /// Test fails only in specific environments
    EnvironmentSensitive,
    /// Test has timing dependencies
    TimingDependent,
}

/// Flaky test detection report.
#[derive(Debug, Serialize, Deserialize)]
pub struct FlakyReport {
    pub total_tests: usize,
    pub flaky_tests: Vec<FlakyTest>,
    pub flakiness_ratio: f64,
    pub confidence: String,
}

/// Flaky test detector engine.
pub struct FlakyDetector {
    /// Minimum runs required to detect flakiness
    min_runs: usize,
    /// Threshold for considering a test flaky (0.0-1.0)
    flaky_threshold: f64,
}

impl Default for FlakyDetector {
    fn default() -> Self {
        Self {
            min_runs: 10,
            flaky_threshold: 0.3,
        }
    }
}

impl FlakyDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create detector with custom thresholds.
    pub fn with_config(min_runs: usize, flaky_threshold: f64) -> Self {
        Self {
            min_runs,
            flaky_threshold,
        }
    }

    /// Analyze test results from JUnit XML files.
    pub fn analyze_junit_files(&self, paths: &[PathBuf]) -> Result<FlakyReport> {
        let mut test_history: HashMap<String, Vec<TestResult>> = HashMap::new();

        for path in paths {
            let results = self.parse_junit_xml(path)?;
            for result in results {
                test_history
                    .entry(result.name.clone())
                    .or_default()
                    .push(result);
            }
        }

        self.analyze_test_history(&test_history)
    }

    /// Analyze test history to detect flakiness.
    fn analyze_test_history(
        &self,
        history: &HashMap<String, Vec<TestResult>>,
    ) -> Result<FlakyReport> {
        let mut flaky_tests = Vec::new();
        let total_tests = history.len();

        for (test_name, results) in history {
            if results.len() < self.min_runs {
                continue;
            }

            let failures = results
                .iter()
                .filter(|r| r.status == TestStatus::Failed)
                .count();
            let passes = results
                .iter()
                .filter(|r| r.status == TestStatus::Passed)
                .count();
            let total_runs = failures + passes;

            if total_runs == 0 {
                continue;
            }

            let failure_rate = failures as f64 / total_runs as f64;

            // Detect flakiness: test has both passes and failures
            if failures > 0 && passes > 0 && failure_rate < 0.9 {
                let flakiness_score = self.calculate_flakiness_score(results);
                let category = self.categorize_flakiness(results, failure_rate);

                let recent_failures: Vec<String> = results
                    .iter()
                    .filter(|r| r.status == TestStatus::Failed)
                    .filter_map(|r| r.error_message.clone())
                    .take(3)
                    .collect();

                if flakiness_score >= self.flaky_threshold {
                    flaky_tests.push(FlakyTest {
                        name: test_name.clone(),
                        flakiness_score,
                        total_runs,
                        failures,
                        passes,
                        failure_rate,
                        recent_failures,
                        category,
                    });
                }
            }
        }

        // Sort by flakiness score descending
        flaky_tests.sort_by(|a, b| {
            b.flakiness_score
                .partial_cmp(&a.flakiness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let flakiness_ratio = if total_tests > 0 {
            flaky_tests.len() as f64 / total_tests as f64
        } else {
            0.0
        };

        let confidence = if history.values().all(|v| v.len() >= 20) {
            "High".to_string()
        } else if history.values().all(|v| v.len() >= 10) {
            "Medium".to_string()
        } else {
            "Low".to_string()
        };

        Ok(FlakyReport {
            total_tests,
            flaky_tests,
            flakiness_ratio,
            confidence,
        })
    }

    /// Calculate flakiness score (0.0 = stable, 1.0 = extremely flaky).
    fn calculate_flakiness_score(&self, results: &[TestResult]) -> f64 {
        if results.len() < 2 {
            return 0.0;
        }

        // Count transitions between pass/fail states
        let mut transitions = 0;
        for window in results.windows(2) {
            if window[0].status != window[1].status {
                transitions += 1;
            }
        }

        let transition_rate = transitions as f64 / (results.len() - 1) as f64;

        // Calculate failure clustering (lower clustering = more flaky)
        let failures = results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        let failure_rate = failures as f64 / results.len() as f64;

        // Flakiness combines transition rate and failure rate
        // Perfect flakiness: 50% failure rate with high transitions
        let failure_variance = ((failure_rate - 0.5).abs() - 0.5).abs();

        // Score: higher transitions + mid-range failure rate = higher flakiness
        (transition_rate * 0.7 + failure_variance * 0.3).min(1.0)
    }

    /// Categorize the type of flakiness.
    fn categorize_flakiness(&self, results: &[TestResult], failure_rate: f64) -> FlakyCategory {
        // Check for timing patterns (failures with varying durations)
        let durations: Vec<u64> = results.iter().map(|r| r.duration_ms).collect();
        let avg_duration = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
        let duration_variance = durations
            .iter()
            .map(|d| (*d as f64 - avg_duration).powi(2))
            .sum::<f64>()
            / durations.len() as f64;
        let duration_stddev = duration_variance.sqrt();

        if duration_stddev > avg_duration * 0.5 {
            return FlakyCategory::TimingDependent;
        }

        // Check for alternating pattern
        let mut alternations = 0;
        for window in results.windows(2) {
            if window[0].status != window[1].status {
                alternations += 1;
            }
        }
        let alternation_rate = alternations as f64 / (results.len() - 1) as f64;

        if alternation_rate > 0.6 {
            return FlakyCategory::Unstable;
        }

        // Check for error message patterns (environment issues)
        let error_messages: Vec<&str> = results
            .iter()
            .filter_map(|r| r.error_message.as_deref())
            .collect();

        if error_messages
            .iter()
            .any(|msg| msg.contains("timeout") || msg.contains("connection") || msg.contains("network"))
        {
            return FlakyCategory::EnvironmentSensitive;
        }

        if failure_rate < 0.5 {
            FlakyCategory::Intermittent
        } else {
            FlakyCategory::Unstable
        }
    }

    /// Parse JUnit XML test results.
    fn parse_junit_xml(&self, path: &Path) -> Result<Vec<TestResult>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read JUnit XML file: {}", path.display()))?;

        let doc: serde_json::Value = quick_xml::de::from_str(&content)
            .with_context(|| format!("Failed to parse XML: {}", path.display()))?;

        let mut results = Vec::new();

        // Parse testsuites/testsuite/testcase structure
        if let Some(testsuites) = doc.get("testsuites") {
            self.parse_testsuites(testsuites, &mut results);
        } else if let Some(testsuite) = doc.get("testsuite") {
            self.parse_testsuite(testsuite, &mut results);
        }

        Ok(results)
    }

    fn parse_testsuites(&self, testsuites: &serde_json::Value, results: &mut Vec<TestResult>) {
        if let Some(suites) = testsuites.get("testsuite").and_then(|v| v.as_array()) {
            for suite in suites {
                self.parse_testsuite(suite, results);
            }
        } else if let Some(suite) = testsuites.get("testsuite") {
            self.parse_testsuite(suite, results);
        }
    }

    fn parse_testsuite(&self, testsuite: &serde_json::Value, results: &mut Vec<TestResult>) {
        if let Some(testcases) = testsuite.get("testcase").and_then(|v| v.as_array()) {
            for testcase in testcases {
                if let Some(result) = self.parse_testcase(testcase) {
                    results.push(result);
                }
            }
        } else if let Some(testcase) = testsuite.get("testcase") {
            if let Some(result) = self.parse_testcase(testcase) {
                results.push(result);
            }
        }
    }

    fn parse_testcase(&self, testcase: &serde_json::Value) -> Option<TestResult> {
        let name = testcase
            .get("name")
            .or_else(|| testcase.get("@name"))
            .and_then(|v| v.as_str())?
            .to_string();

        let class = testcase
            .get("classname")
            .or_else(|| testcase.get("@classname"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let full_name = if !class.is_empty() {
            format!("{}::{}", class, name)
        } else {
            name
        };

        let duration_ms = testcase
            .get("time")
            .or_else(|| testcase.get("@time"))
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| v.as_f64())
            })
            .unwrap_or(0.0)
            * 1000.0;

        let (status, error_message) = if testcase.get("failure").is_some() {
            let msg = testcase
                .get("failure")
                .and_then(|f| f.get("message").or_else(|| f.get("@message")))
                .and_then(|v| v.as_str())
                .map(String::from);
            (TestStatus::Failed, msg)
        } else if testcase.get("error").is_some() {
            let msg = testcase
                .get("error")
                .and_then(|e| e.get("message").or_else(|| e.get("@message")))
                .and_then(|v| v.as_str())
                .map(String::from);
            (TestStatus::Failed, msg)
        } else if testcase.get("skipped").is_some() {
            (TestStatus::Skipped, None)
        } else {
            (TestStatus::Passed, None)
        };

        Some(TestResult {
            name: full_name,
            status,
            duration_ms: duration_ms as u64,
            timestamp: 0, // Would need to extract from file metadata or XML
            error_message,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_result(name: &str, status: TestStatus) -> TestResult {
        TestResult {
            name: name.to_string(),
            status,
            duration_ms: 100,
            timestamp: 0,
            error_message: None,
        }
    }

    #[test]
    fn test_flakiness_score_stable_test() {
        let detector = FlakyDetector::new();
        let results = vec![
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Passed),
        ];

        let score = detector.calculate_flakiness_score(&results);
        assert!(score < 0.3, "Stable test should have low flakiness score");
    }

    #[test]
    fn test_flakiness_score_alternating() {
        let detector = FlakyDetector::new();
        let results = vec![
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Failed),
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Failed),
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Failed),
        ];

        let score = detector.calculate_flakiness_score(&results);
        assert!(score > 0.6, "Alternating test should have high flakiness score");
    }

    #[test]
    fn test_categorize_timing_dependent() {
        let detector = FlakyDetector::new();
        let mut results = vec![
            create_test_result("test", TestStatus::Passed),
            create_test_result("test", TestStatus::Passed),
        ];
        results[0].duration_ms = 100;
        results[1].duration_ms = 1000;

        let category = detector.categorize_flakiness(&results, 0.0);
        assert_eq!(category, FlakyCategory::TimingDependent);
    }
}
