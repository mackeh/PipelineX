//! LLM-powered optimization explanations for pipeline findings.
//!
//! Supports Anthropic Claude, OpenAI, and template-based fallback.

use crate::analyzer::report::{Finding, Severity};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// LLM provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainerConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LLMProvider {
    Anthropic,
    OpenAI,
    Template,
}

/// An explanation for a single finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub finding_title: String,
    pub plain_english: String,
    pub why_it_matters: String,
    pub simplest_fix: String,
    pub estimated_impact: String,
}

/// The explainer that generates human-readable explanations.
pub struct Explainer {
    config: ExplainerConfig,
}

impl Explainer {
    /// Create an explainer from explicit config.
    pub fn new(config: ExplainerConfig) -> Self {
        Self { config }
    }

    /// Try to auto-detect config from environment variables.
    /// Falls back to template-based explanations if no API key found.
    pub fn from_env() -> Self {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            return Self::new(ExplainerConfig {
                provider: LLMProvider::Anthropic,
                model: "claude-sonnet-4-20250514".to_string(),
                api_key: key,
            });
        }

        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            return Self::new(ExplainerConfig {
                provider: LLMProvider::OpenAI,
                model: "gpt-4o".to_string(),
                api_key: key,
            });
        }

        Self::template()
    }

    /// Create a template-based explainer (no LLM, no API key required).
    pub fn template() -> Self {
        Self::new(ExplainerConfig {
            provider: LLMProvider::Template,
            model: "template".to_string(),
            api_key: String::new(),
        })
    }

    /// Generate explanations for all findings.
    pub async fn explain_all(
        &self,
        findings: &[Finding],
        context: &PipelineContext,
    ) -> Vec<Explanation> {
        let mut explanations = Vec::new();
        for finding in findings {
            let explanation = self.explain(finding, context).await;
            explanations.push(explanation);
        }
        explanations
    }

    /// Generate an explanation for a single finding.
    pub async fn explain(&self, finding: &Finding, context: &PipelineContext) -> Explanation {
        match self.config.provider {
            LLMProvider::Anthropic => self
                .explain_anthropic(finding, context)
                .await
                .unwrap_or_else(|_| self.explain_template(finding, context)),
            LLMProvider::OpenAI => self
                .explain_openai(finding, context)
                .await
                .unwrap_or_else(|_| self.explain_template(finding, context)),
            LLMProvider::Template => self.explain_template(finding, context),
        }
    }

    async fn explain_anthropic(
        &self,
        finding: &Finding,
        context: &PipelineContext,
    ) -> Result<Explanation> {
        let prompt = Self::build_prompt(finding, context);

        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 500,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to call Anthropic API")?;

        let json: serde_json::Value = resp.json().await.context("Failed to parse response")?;

        let text = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(Self::parse_llm_response(&text, finding))
    }

    async fn explain_openai(
        &self,
        finding: &Finding,
        context: &PipelineContext,
    ) -> Result<Explanation> {
        let prompt = Self::build_prompt(finding, context);

        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 500,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to call OpenAI API")?;

        let json: serde_json::Value = resp.json().await.context("Failed to parse response")?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(Self::parse_llm_response(&text, finding))
    }

    fn build_prompt(finding: &Finding, context: &PipelineContext) -> String {
        let savings = finding
            .estimated_savings_secs
            .map(|s| format!("{:.0} seconds per run", s))
            .unwrap_or_else(|| "unknown".to_string());

        format!(
            "Explain this CI/CD pipeline finding to a developer in clear, actionable language.\n\
             Respond in exactly 3 lines:\n\
             Line 1: WHY_IT_MATTERS: <why this matters in plain English>\n\
             Line 2: IMPACT: <concrete cost/time impact>\n\
             Line 3: FIX: <the simplest fix in one sentence>\n\n\
             Finding: {}\n\
             Severity: {:?}\n\
             Description: {}\n\
             Estimated savings: {}\n\
             Pipeline context: {} jobs, {} steps, provider: {}\n\
             Affected jobs: {}",
            finding.title,
            finding.severity,
            finding.description,
            savings,
            context.job_count,
            context.step_count,
            context.provider,
            finding.affected_jobs.join(", ")
        )
    }

    fn parse_llm_response(text: &str, finding: &Finding) -> Explanation {
        let lines: Vec<&str> = text.lines().collect();

        let why = lines
            .iter()
            .find(|l| l.contains("WHY_IT_MATTERS:"))
            .map(|l| l.trim_start_matches("WHY_IT_MATTERS:").trim().to_string())
            .unwrap_or_else(|| lines.first().unwrap_or(&"").trim().to_string());

        let impact = lines
            .iter()
            .find(|l| l.contains("IMPACT:"))
            .map(|l| l.trim_start_matches("IMPACT:").trim().to_string())
            .unwrap_or_else(|| {
                finding
                    .estimated_savings_secs
                    .map(|s| format!("~{:.0}s savings per run", s))
                    .unwrap_or_default()
            });

        let fix = lines
            .iter()
            .find(|l| l.contains("FIX:"))
            .map(|l| l.trim_start_matches("FIX:").trim().to_string())
            .unwrap_or_else(|| finding.recommendation.clone());

        Explanation {
            finding_title: finding.title.clone(),
            plain_english: why.clone(),
            why_it_matters: why,
            simplest_fix: fix,
            estimated_impact: impact,
        }
    }

    /// Template-based explanation (no API call needed).
    fn explain_template(&self, finding: &Finding, context: &PipelineContext) -> Explanation {
        let severity_impact = match finding.severity {
            Severity::Critical => {
                "This is a critical issue that significantly impacts your CI/CD performance."
            }
            Severity::High => {
                "This is a high-priority issue that noticeably slows down your pipeline."
            }
            Severity::Medium => {
                "This is a moderate issue that contributes to unnecessary pipeline time."
            }
            Severity::Low => "This is a minor optimization opportunity.",
            Severity::Info => "This is an informational observation about your pipeline.",
        };

        let savings_text = finding
            .estimated_savings_secs
            .map(|s| {
                let monthly = s * context.runs_per_month as f64;
                if monthly > 3600.0 {
                    format!(
                        "Fixing this saves ~{:.0}s per run, or ~{:.1} hours/month at {} runs/month.",
                        s,
                        monthly / 3600.0,
                        context.runs_per_month
                    )
                } else {
                    format!(
                        "Fixing this saves ~{:.0}s per run, or ~{:.0} minutes/month at {} runs/month.",
                        s,
                        monthly / 60.0,
                        context.runs_per_month
                    )
                }
            })
            .unwrap_or_else(|| "Impact varies depending on your pipeline configuration.".to_string());

        let why = format!(
            "{} In your {} pipeline with {} jobs and {} steps, {}",
            severity_impact,
            context.provider,
            context.job_count,
            context.step_count,
            finding.description.to_lowercase()
        );

        Explanation {
            finding_title: finding.title.clone(),
            plain_english: why.clone(),
            why_it_matters: why,
            simplest_fix: finding.recommendation.clone(),
            estimated_impact: savings_text,
        }
    }
}

/// Context about the pipeline for richer explanations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineContext {
    pub job_count: usize,
    pub step_count: usize,
    pub provider: String,
    pub pipeline_name: String,
    pub runs_per_month: u32,
}

impl PipelineContext {
    pub fn from_dag(dag: &crate::PipelineDag) -> Self {
        Self {
            job_count: dag.job_count(),
            step_count: dag.step_count(),
            provider: dag.provider.clone(),
            pipeline_name: dag.name.clone(),
            runs_per_month: 500, // Default
        }
    }
}

/// Format explanations for terminal display.
pub fn format_explanations(explanations: &[Explanation]) -> String {
    let mut out = String::new();
    for (i, exp) in explanations.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, exp.finding_title));
        out.push_str(&format!("   {}\n", exp.why_it_matters));
        out.push_str(&format!("   Impact: {}\n", exp.estimated_impact));
        out.push_str(&format!("   Fix: {}\n\n", exp.simplest_fix));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::report::FindingCategory;

    fn sample_finding() -> Finding {
        Finding {
            severity: Severity::Critical,
            category: FindingCategory::MissingCache,
            title: "No dependency caching for npm".to_string(),
            description: "Job 'build' installs npm packages without caching, adding ~2:30 per run."
                .to_string(),
            affected_jobs: vec!["build".to_string()],
            recommendation:
                "Add actions/cache with path node_modules and key based on package-lock.json hash."
                    .to_string(),
            fix_command: Some("pipelinex optimize ci.yml".to_string()),
            estimated_savings_secs: Some(150.0),
            confidence: 0.95,
            auto_fixable: true,
        }
    }

    fn sample_context() -> PipelineContext {
        PipelineContext {
            job_count: 5,
            step_count: 15,
            provider: "github-actions".to_string(),
            pipeline_name: "CI".to_string(),
            runs_per_month: 500,
        }
    }

    #[test]
    fn test_template_explanation() {
        let explainer = Explainer::template();
        let finding = sample_finding();
        let context = sample_context();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let explanation = rt.block_on(explainer.explain(&finding, &context));

        assert!(!explanation.plain_english.is_empty());
        assert!(!explanation.simplest_fix.is_empty());
        assert!(explanation.estimated_impact.contains("150"));
        assert_eq!(explanation.finding_title, "No dependency caching for npm");
    }

    #[test]
    fn test_explain_all_template() {
        let explainer = Explainer::template();
        let findings = vec![sample_finding(), sample_finding()];
        let context = sample_context();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let explanations = rt.block_on(explainer.explain_all(&findings, &context));

        assert_eq!(explanations.len(), 2);
    }

    #[test]
    fn test_format_explanations() {
        let explanations = vec![Explanation {
            finding_title: "Missing cache".to_string(),
            plain_english: "Your pipeline re-downloads dependencies every run.".to_string(),
            why_it_matters: "Your pipeline re-downloads dependencies every run.".to_string(),
            simplest_fix: "Add a cache step.".to_string(),
            estimated_impact: "Saves 2:30 per run.".to_string(),
        }];

        let formatted = format_explanations(&explanations);
        assert!(formatted.contains("Missing cache"));
        assert!(formatted.contains("Saves 2:30 per run"));
        assert!(formatted.contains("Add a cache step"));
    }

    #[test]
    fn test_from_env_fallback() {
        // With no env vars set, should fall back to template
        let explainer = Explainer::from_env();
        // We can't assert the provider easily since env vars may exist,
        // but the explainer should be created without error
        let finding = sample_finding();
        let context = sample_context();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let explanation = rt.block_on(explainer.explain(&finding, &context));
        assert!(!explanation.plain_english.is_empty());
    }

    #[test]
    fn test_parse_llm_response() {
        let text = "WHY_IT_MATTERS: Your build downloads all deps every run.\n\
                    IMPACT: Wastes 2.5 minutes per run, 20+ hours/month.\n\
                    FIX: Add actions/cache for node_modules.";

        let finding = sample_finding();
        let explanation = Explainer::parse_llm_response(text, &finding);

        assert!(explanation.why_it_matters.contains("downloads all deps"));
        assert!(explanation.estimated_impact.contains("2.5 minutes"));
        assert!(explanation.simplest_fix.contains("actions/cache"));
    }
}
