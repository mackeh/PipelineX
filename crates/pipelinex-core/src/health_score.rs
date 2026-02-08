use serde::{Deserialize, Serialize};

/// Pipeline health score calculator
///
/// Evaluates pipeline quality based on multiple factors:
/// - Duration efficiency
/// - Success rate
/// - Parallelization
/// - Caching strategy
/// - Test coverage
/// - Flaky test count
#[derive(Debug, Clone)]
pub struct HealthScoreCalculator {
    weights: HealthScoreWeights,
}

/// Configurable weights for health score components
#[derive(Debug, Clone)]
pub struct HealthScoreWeights {
    pub duration_efficiency: f64,
    pub success_rate: f64,
    pub parallelization: f64,
    pub caching: f64,
    pub issue_severity: f64,
}

/// Health score result with detailed breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    /// Overall health score (0-100)
    pub total_score: f64,

    /// Individual component scores
    pub duration_score: f64,
    pub success_rate_score: f64,
    pub parallelization_score: f64,
    pub caching_score: f64,
    pub issue_score: f64,

    /// Health grade
    pub grade: HealthGrade,

    /// Recommendations for improvement
    pub recommendations: Vec<String>,
}

/// Health grade categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthGrade {
    Excellent,  // 90-100
    Good,       // 75-89
    Fair,       // 60-74
    Poor,       // 40-59
    Critical,   // 0-39
}

impl Default for HealthScoreWeights {
    fn default() -> Self {
        Self {
            duration_efficiency: 0.25,
            success_rate: 0.30,
            parallelization: 0.20,
            caching: 0.15,
            issue_severity: 0.10,
        }
    }
}

impl HealthScoreCalculator {
    pub fn new() -> Self {
        Self {
            weights: HealthScoreWeights::default(),
        }
    }

    pub fn with_weights(weights: HealthScoreWeights) -> Self {
        Self { weights }
    }

    /// Calculate health score from pipeline metrics
    pub fn calculate(
        &self,
        duration_secs: f64,
        optimal_duration_secs: f64,
        success_rate: f64,
        parallelization_ratio: f64,
        has_caching: bool,
        critical_issues: usize,
        high_issues: usize,
        medium_issues: usize,
    ) -> HealthScore {
        // Duration efficiency score (0-100)
        let duration_score = if optimal_duration_secs > 0.0 {
            let efficiency = optimal_duration_secs / duration_secs.max(1.0);
            (efficiency * 100.0).min(100.0)
        } else {
            let baseline_duration = 600.0; // 10 minutes baseline
            let efficiency = baseline_duration / duration_secs.max(1.0);
            (efficiency * 100.0).min(100.0)
        };

        // Success rate score (0-100)
        let success_rate_score = success_rate * 100.0;

        // Parallelization score (0-100)
        // 0.0 = fully serial, 1.0 = fully parallel
        let parallelization_score = parallelization_ratio * 100.0;

        // Caching score (0-100)
        let caching_score = if has_caching { 100.0 } else { 0.0 };

        // Issue score (0-100) - deduct points for issues
        let issue_score = 100.0
            - (critical_issues as f64 * 15.0)
            - (high_issues as f64 * 8.0)
            - (medium_issues as f64 * 3.0);
        let issue_score = issue_score.max(0.0);

        // Calculate weighted total
        let total_score = (duration_score * self.weights.duration_efficiency)
            + (success_rate_score * self.weights.success_rate)
            + (parallelization_score * self.weights.parallelization)
            + (caching_score * self.weights.caching)
            + (issue_score * self.weights.issue_severity);

        let grade = Self::score_to_grade(total_score);
        let recommendations = self.generate_recommendations(
            duration_score,
            success_rate_score,
            parallelization_score,
            caching_score,
            issue_score,
            critical_issues,
            high_issues,
        );

        HealthScore {
            total_score,
            duration_score,
            success_rate_score,
            parallelization_score,
            caching_score,
            issue_score,
            grade,
            recommendations,
        }
    }

    fn score_to_grade(score: f64) -> HealthGrade {
        match score as i32 {
            90..=100 => HealthGrade::Excellent,
            75..=89 => HealthGrade::Good,
            60..=74 => HealthGrade::Fair,
            40..=59 => HealthGrade::Poor,
            _ => HealthGrade::Critical,
        }
    }

    fn generate_recommendations(
        &self,
        duration_score: f64,
        success_rate_score: f64,
        parallelization_score: f64,
        caching_score: f64,
        issue_score: f64,
        critical_issues: usize,
        high_issues: usize,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Priority: Critical issues first
        if critical_issues > 0 {
            recommendations.push(format!(
                "🔴 Fix {} critical issues immediately - they have severe performance impact",
                critical_issues
            ));
        }

        // Success rate
        if success_rate_score < 90.0 {
            recommendations.push(format!(
                "🔴 Improve success rate (currently {:.1}%) - investigate flaky tests and unstable jobs",
                success_rate_score
            ));
        }

        // High issues
        if high_issues > 0 && critical_issues == 0 {
            recommendations.push(format!(
                "🟠 Address {} high-priority issues for significant improvements",
                high_issues
            ));
        }

        // Duration
        if duration_score < 60.0 {
            recommendations.push(
                "🟠 Pipeline duration is suboptimal - consider parallelization and caching"
                    .to_string(),
            );
        }

        // Caching
        if caching_score < 50.0 {
            recommendations.push(
                "🟡 Add caching for dependencies to reduce build times".to_string(),
            );
        }

        // Parallelization
        if parallelization_score < 50.0 {
            recommendations.push(
                "🟡 Increase parallelization - many jobs could run concurrently".to_string(),
            );
        }

        // General improvement
        if issue_score < 80.0 && recommendations.is_empty() {
            recommendations.push(
                "💡 Run 'pipelinex optimize' to generate improved configuration".to_string(),
            );
        }

        // If everything is good
        if recommendations.is_empty() {
            recommendations.push("✅ Pipeline is well-optimized! Keep monitoring for regressions.".to_string());
        }

        recommendations
    }
}

impl Default for HealthScoreCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthGrade {
    pub fn emoji(&self) -> &'static str {
        match self {
            HealthGrade::Excellent => "🌟",
            HealthGrade::Good => "✅",
            HealthGrade::Fair => "🟡",
            HealthGrade::Poor => "🟠",
            HealthGrade::Critical => "🔴",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            HealthGrade::Excellent => "Excellent",
            HealthGrade::Good => "Good",
            HealthGrade::Fair => "Fair",
            HealthGrade::Poor => "Poor",
            HealthGrade::Critical => "Critical",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            HealthGrade::Excellent => "Pipeline is excellently optimized with minimal issues",
            HealthGrade::Good => "Pipeline performs well with minor room for improvement",
            HealthGrade::Fair => "Pipeline is functional but has notable optimization opportunities",
            HealthGrade::Poor => "Pipeline has significant performance issues requiring attention",
            HealthGrade::Critical => "Pipeline requires immediate optimization",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_score() {
        let calculator = HealthScoreCalculator::new();
        let score = calculator.calculate(
            300.0,   // duration
            300.0,   // optimal
            1.0,     // 100% success rate
            1.0,     // fully parallel
            true,    // has caching
            0,       // no critical issues
            0,       // no high issues
            0,       // no medium issues
        );

        assert!(score.total_score >= 95.0);
        assert_eq!(score.grade, HealthGrade::Excellent);
    }

    #[test]
    fn test_poor_score() {
        let calculator = HealthScoreCalculator::new();
        let score = calculator.calculate(
            1800.0,  // 30 min duration
            300.0,   // 5 min optimal
            0.7,     // 70% success rate
            0.2,     // mostly serial
            false,   // no caching
            3,       // 3 critical issues
            5,       // 5 high issues
            10,      // 10 medium issues
        );

        assert!(score.total_score < 50.0);
        assert!(matches!(score.grade, HealthGrade::Poor | HealthGrade::Critical));
        assert!(!score.recommendations.is_empty());
    }

    #[test]
    fn test_grade_assignment() {
        assert_eq!(HealthScoreCalculator::score_to_grade(95.0), HealthGrade::Excellent);
        assert_eq!(HealthScoreCalculator::score_to_grade(85.0), HealthGrade::Good);
        assert_eq!(HealthScoreCalculator::score_to_grade(65.0), HealthGrade::Fair);
        assert_eq!(HealthScoreCalculator::score_to_grade(45.0), HealthGrade::Poor);
        assert_eq!(HealthScoreCalculator::score_to_grade(25.0), HealthGrade::Critical);
    }
}
