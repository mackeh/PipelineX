use crate::analyzer::report::{AnalysisReport, Severity};
use serde::{Deserialize, Serialize};

/// Health score result for badge generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BadgeInfo {
    pub score: u8,
    pub grade: String,
    pub color: String,
    pub optimization_pct: f64,
    pub markdown: String,
    pub shields_url: String,
}

/// Calculate a pipeline health score and generate badge info.
pub fn generate_badge(report: &AnalysisReport) -> BadgeInfo {
    let score = calculate_score(report);
    let grade = score_to_grade(score);
    let color = grade_to_color(&grade);

    let optimization_pct = if report.total_estimated_duration_secs > 0.0 {
        ((report.total_estimated_duration_secs - report.optimized_duration_secs)
            / report.total_estimated_duration_secs
            * 100.0)
            .clamp(0.0, 100.0)
    } else {
        0.0
    };

    let label = format!("PipelineX: {} | {}/100", grade, score);
    let shields_url = format!(
        "https://img.shields.io/badge/{}-{}-{}",
        url_encode(&label),
        url_encode(&format!("{:.0}% optimized", optimization_pct)),
        color
    );

    let markdown = format!(
        "[![PipelineX]({})](https://github.com/mackeh/PipelineX)",
        shields_url
    );

    BadgeInfo {
        score,
        grade,
        color,
        optimization_pct,
        markdown,
        shields_url,
    }
}

fn calculate_score(report: &AnalysisReport) -> u8 {
    let base: i32 = 100;

    let deductions: i32 = report
        .findings
        .iter()
        .map(|f| match f.severity {
            Severity::Critical => 25,
            Severity::High => 10,
            Severity::Medium => 3,
            Severity::Low => 1,
            Severity::Info => 0,
        })
        .sum();

    (base - deductions).clamp(0, 100) as u8
}

fn score_to_grade(score: u8) -> String {
    match score {
        95..=100 => "A+".to_string(),
        85..=94 => "A".to_string(),
        70..=84 => "B".to_string(),
        50..=69 => "C".to_string(),
        25..=49 => "D".to_string(),
        _ => "F".to_string(),
    }
}

fn grade_to_color(grade: &str) -> String {
    match grade {
        "A+" => "brightgreen".to_string(),
        "A" => "green".to_string(),
        "B" => "yellowgreen".to_string(),
        "C" => "yellow".to_string(),
        "D" => "orange".to_string(),
        _ => "red".to_string(),
    }
}

fn url_encode(s: &str) -> String {
    s.replace(' ', "%20")
        .replace(':', "%3A")
        .replace('|', "%7C")
        .replace('/', "%2F")
        .replace('%', "%25")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::report::{Finding, FindingCategory};

    fn make_report(findings: Vec<Finding>) -> AnalysisReport {
        AnalysisReport {
            pipeline_name: "CI".to_string(),
            source_file: "ci.yml".to_string(),
            provider: "github-actions".to_string(),
            job_count: 3,
            step_count: 10,
            max_parallelism: 2,
            critical_path: vec!["build".to_string(), "test".to_string()],
            critical_path_duration_secs: 120.0,
            total_estimated_duration_secs: 300.0,
            optimized_duration_secs: 150.0,
            findings,
            health_score: None,
        }
    }

    #[test]
    fn test_perfect_score() {
        let report = make_report(vec![]);
        let badge = generate_badge(&report);
        assert_eq!(badge.score, 100);
        assert_eq!(badge.grade, "A+");
        assert_eq!(badge.color, "brightgreen");
    }

    #[test]
    fn test_score_with_critical() {
        let report = make_report(vec![Finding {
            severity: Severity::Critical,
            category: FindingCategory::MissingCache,
            title: "test".into(),
            description: "test".into(),
            affected_jobs: vec![],
            recommendation: "test".into(),
            fix_command: None,
            estimated_savings_secs: None,
            confidence: 0.9,
            auto_fixable: false,
        }]);
        let badge = generate_badge(&report);
        assert_eq!(badge.score, 75);
        assert_eq!(badge.grade, "B");
    }

    #[test]
    fn test_badge_markdown() {
        let report = make_report(vec![]);
        let badge = generate_badge(&report);
        assert!(badge.markdown.contains("shields.io"));
        assert!(badge.markdown.contains("PipelineX"));
    }

    #[test]
    fn test_optimization_pct() {
        let report = make_report(vec![]);
        let badge = generate_badge(&report);
        assert!((badge.optimization_pct - 50.0).abs() < 0.1);
    }
}
