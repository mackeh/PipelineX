use crate::analyzer;
use crate::linter;
use crate::security;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

/// MCP tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// MCP JSON-RPC request.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC response.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// Define available MCP tools.
pub fn list_tools() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "pipelinex_analyze".to_string(),
            description: "Analyze a CI/CD pipeline configuration for bottlenecks and optimization opportunities.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "yaml_content": {
                        "type": "string",
                        "description": "The YAML content of the CI/CD pipeline configuration"
                    },
                    "provider": {
                        "type": "string",
                        "description": "CI provider (github-actions, gitlab-ci, circleci, etc.)",
                        "default": "github-actions"
                    }
                },
                "required": ["yaml_content"]
            }),
        },
        McpTool {
            name: "pipelinex_optimize".to_string(),
            description: "Generate an optimized version of a CI/CD pipeline configuration.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "yaml_content": {
                        "type": "string",
                        "description": "The YAML content of the pipeline configuration to optimize"
                    },
                    "provider": {
                        "type": "string",
                        "description": "CI provider",
                        "default": "github-actions"
                    }
                },
                "required": ["yaml_content"]
            }),
        },
        McpTool {
            name: "pipelinex_lint".to_string(),
            description: "Lint a CI/CD pipeline configuration for syntax errors, deprecations, and typos.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "yaml_content": {
                        "type": "string",
                        "description": "The YAML content of the pipeline configuration to lint"
                    },
                    "provider": {
                        "type": "string",
                        "description": "CI provider",
                        "default": "github-actions"
                    }
                },
                "required": ["yaml_content"]
            }),
        },
        McpTool {
            name: "pipelinex_security".to_string(),
            description: "Scan a CI/CD pipeline configuration for security issues (secrets, permissions, injection, supply chain).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "yaml_content": {
                        "type": "string",
                        "description": "The YAML content of the pipeline configuration to scan"
                    },
                    "provider": {
                        "type": "string",
                        "description": "CI provider",
                        "default": "github-actions"
                    }
                },
                "required": ["yaml_content"]
            }),
        },
        McpTool {
            name: "pipelinex_cost".to_string(),
            description: "Estimate CI/CD costs and potential savings for a pipeline configuration.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "yaml_content": {
                        "type": "string",
                        "description": "The YAML content of the pipeline configuration"
                    },
                    "runs_per_month": {
                        "type": "number",
                        "description": "Estimated pipeline runs per month",
                        "default": 500
                    },
                    "provider": {
                        "type": "string",
                        "description": "CI provider",
                        "default": "github-actions"
                    }
                },
                "required": ["yaml_content"]
            }),
        },
    ]
}

fn parse_yaml_to_dag(
    yaml_content: &str,
    provider: &str,
) -> Result<crate::parser::dag::PipelineDag, String> {
    use crate::parser::github::GitHubActionsParser;
    use crate::parser::gitlab::GitLabCIParser;

    match provider {
        "github-actions" | "github" => {
            GitHubActionsParser::parse_content(yaml_content, "mcp-input.yml")
                .map_err(|e| format!("Failed to parse GitHub Actions YAML: {}", e))
        }
        "gitlab-ci" | "gitlab" => GitLabCIParser::parse_content(yaml_content, "mcp-input.yml")
            .map_err(|e| format!("Failed to parse GitLab CI YAML: {}", e)),
        other => Err(format!(
            "Unsupported provider '{}'. Use 'github-actions' or 'gitlab-ci'.",
            other
        )),
    }
}

/// Handle a single MCP tool call.
pub fn handle_tool_call(
    tool_name: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let yaml_content = params
        .get("yaml_content")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: yaml_content")?;

    let provider = params
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("github-actions");

    let dag = parse_yaml_to_dag(yaml_content, provider)?;

    match tool_name {
        "pipelinex_analyze" => {
            let report = analyzer::analyze(&dag);
            serde_json::to_value(&report).map_err(|e| e.to_string())
        }
        "pipelinex_optimize" => {
            let report = analyzer::analyze(&dag);
            // Return analysis with optimization suggestions
            let result = serde_json::json!({
                "findings": report.findings.len(),
                "current_duration_secs": report.total_estimated_duration_secs,
                "optimized_duration_secs": report.optimized_duration_secs,
                "improvement_pct": report.potential_improvement_pct(),
                "recommendations": report.findings.iter().map(|f| {
                    serde_json::json!({
                        "severity": format!("{:?}", f.severity),
                        "title": f.title,
                        "recommendation": f.recommendation,
                        "auto_fixable": f.auto_fixable,
                    })
                }).collect::<Vec<_>>(),
            });
            Ok(result)
        }
        "pipelinex_lint" => {
            let lint_report = linter::lint(yaml_content, &dag);
            serde_json::to_value(&lint_report).map_err(|e| e.to_string())
        }
        "pipelinex_security" => {
            let findings = security::scan(&dag);
            serde_json::to_value(&findings).map_err(|e| e.to_string())
        }
        "pipelinex_cost" => {
            let report = analyzer::analyze(&dag);
            let runs_per_month = params
                .get("runs_per_month")
                .and_then(|v| v.as_u64())
                .unwrap_or(500) as u32;

            let runner_type = dag
                .graph
                .node_weights()
                .next()
                .map(|j| j.runs_on.as_str())
                .unwrap_or("ubuntu-latest");

            let estimate = crate::cost::estimate_costs(
                report.total_estimated_duration_secs,
                report.optimized_duration_secs,
                runs_per_month,
                runner_type,
                150.0,
                10,
            );
            serde_json::to_value(&estimate).map_err(|e| e.to_string())
        }
        other => Err(format!("Unknown tool: {}", other)),
    }
}

/// Run the MCP server on stdio, handling JSON-RPC messages.
pub fn run_stdio_server() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => process_request(&request),
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: serde_json::Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            },
        };

        let json = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
    }

    Ok(())
}

fn process_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "pipelinex",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },
        "tools/list" => {
            let tools = list_tools();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({ "tools": tools })),
                error: None,
            }
        }
        "tools/call" => {
            let tool_name = request
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match handle_tool_call(tool_name, &arguments) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                        }]
                    })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error: {}", e)
                        }],
                        "isError": true
                    })),
                    error: None,
                },
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WORKFLOW: &str = r#"
name: CI
on: push
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm ci
      - run: npm test
"#;

    #[test]
    fn test_list_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 5);
        assert!(tools.iter().any(|t| t.name == "pipelinex_analyze"));
        assert!(tools.iter().any(|t| t.name == "pipelinex_lint"));
        assert!(tools.iter().any(|t| t.name == "pipelinex_security"));
    }

    #[test]
    fn test_handle_analyze() {
        let params = serde_json::json!({
            "yaml_content": SAMPLE_WORKFLOW,
            "provider": "github-actions"
        });
        let result = handle_tool_call("pipelinex_analyze", &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_lint() {
        let params = serde_json::json!({
            "yaml_content": SAMPLE_WORKFLOW,
        });
        let result = handle_tool_call("pipelinex_lint", &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_security() {
        let params = serde_json::json!({
            "yaml_content": SAMPLE_WORKFLOW,
        });
        let result = handle_tool_call("pipelinex_security", &params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_unknown_tool() {
        let params = serde_json::json!({ "yaml_content": "test" });
        let result = handle_tool_call("unknown_tool", &params);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_initialize() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "initialize".into(),
            params: serde_json::json!({}),
        };
        let response = process_request(&request);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_process_tools_list() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "tools/list".into(),
            params: serde_json::json!({}),
        };
        let response = process_request(&request);
        assert!(response.result.is_some());
    }

    #[test]
    fn test_process_tools_call() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(3),
            method: "tools/call".into(),
            params: serde_json::json!({
                "name": "pipelinex_analyze",
                "arguments": {
                    "yaml_content": SAMPLE_WORKFLOW,
                    "provider": "github-actions"
                }
            }),
        };
        let response = process_request(&request);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }
}
