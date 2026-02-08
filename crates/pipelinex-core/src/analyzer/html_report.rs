use crate::analyzer::report::AnalysisReport;
use crate::parser::dag::PipelineDag;

/// Generate a self-contained HTML report with interactive visualizations.
#[allow(clippy::format_in_format_args)]
pub fn generate_html_report(report: &AnalysisReport, dag: &PipelineDag) -> String {
    let critical_path_json =
        serde_json::to_string(&report.critical_path).unwrap_or_else(|_| "[]".to_string());
    let findings_json =
        serde_json::to_string(&report.findings).unwrap_or_else(|_| "[]".to_string());

    // Generate DAG data for visualization
    let dag_nodes = generate_dag_nodes_json(dag);
    let dag_edges = generate_dag_edges_json(dag);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>PipelineX Analysis Report - {pipeline_name}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}

        :root {{
            --bg-primary: #ffffff;
            --bg-secondary: #f8fafc;
            --bg-card: #ffffff;
            --text-primary: #1e293b;
            --text-secondary: #64748b;
            --border-color: #e2e8f0;
            --accent-color: #3b82f6;
            --success-color: #22c55e;
            --warning-color: #f59e0b;
            --danger-color: #ef4444;
            --shadow: 0 1px 3px rgba(0,0,0,0.1);
            --shadow-lg: 0 10px 15px -3px rgba(0,0,0,0.1);
        }}

        [data-theme="dark"] {{
            --bg-primary: #0f172a;
            --bg-secondary: #1e293b;
            --bg-card: #1e293b;
            --text-primary: #f1f5f9;
            --text-secondary: #94a3b8;
            --border-color: #334155;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            padding: 2rem;
            transition: background 0.3s, color 0.3s;
        }}

        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}

        .header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 2rem;
            padding-bottom: 1rem;
            border-bottom: 2px solid var(--border-color);
        }}

        .header h1 {{
            font-size: 2rem;
            font-weight: 700;
        }}

        .theme-toggle {{
            background: var(--bg-secondary);
            border: 1px solid var(--border-color);
            padding: 0.5rem 1rem;
            border-radius: 0.5rem;
            cursor: pointer;
            transition: all 0.2s;
        }}

        .theme-toggle:hover {{
            background: var(--border-color);
        }}

        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 1.5rem;
            margin-bottom: 2rem;
        }}

        .stat-card {{
            background: var(--bg-card);
            padding: 1.5rem;
            border-radius: 0.75rem;
            border: 1px solid var(--border-color);
            box-shadow: var(--shadow);
        }}

        .stat-label {{
            font-size: 0.875rem;
            color: var(--text-secondary);
            margin-bottom: 0.5rem;
        }}

        .stat-value {{
            font-size: 2rem;
            font-weight: 700;
            color: var(--accent-color);
        }}

        .section {{
            background: var(--bg-card);
            padding: 2rem;
            border-radius: 0.75rem;
            border: 1px solid var(--border-color);
            margin-bottom: 2rem;
            box-shadow: var(--shadow);
        }}

        .section-title {{
            font-size: 1.5rem;
            font-weight: 600;
            margin-bottom: 1.5rem;
        }}

        .finding {{
            padding: 1rem;
            margin-bottom: 1rem;
            border-left: 4px solid;
            border-radius: 0.5rem;
            background: var(--bg-secondary);
        }}

        .finding.critical {{ border-color: var(--danger-color); }}
        .finding.high {{ border-color: #f97316; }}
        .finding.medium {{ border-color: var(--warning-color); }}
        .finding.low {{ border-color: #10b981; }}
        .finding.info {{ border-color: var(--accent-color); }}

        .finding-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 0.5rem;
        }}

        .finding-title {{
            font-weight: 600;
            font-size: 1.125rem;
        }}

        .severity-badge {{
            padding: 0.25rem 0.75rem;
            border-radius: 1rem;
            font-size: 0.75rem;
            font-weight: 600;
            text-transform: uppercase;
        }}

        .severity-badge.critical {{ background: var(--danger-color); color: white; }}
        .severity-badge.high {{ background: #f97316; color: white; }}
        .severity-badge.medium {{ background: var(--warning-color); color: white; }}
        .severity-badge.low {{ background: #10b981; color: white; }}
        .severity-badge.info {{ background: var(--accent-color); color: white; }}

        .finding-description {{
            color: var(--text-secondary);
            margin-bottom: 0.75rem;
        }}

        .finding-meta {{
            display: flex;
            gap: 1rem;
            font-size: 0.875rem;
            color: var(--text-secondary);
        }}

        .dag-container {{
            overflow-x: auto;
            padding: 1rem;
        }}

        #dagCanvas {{
            border: 1px solid var(--border-color);
            border-radius: 0.5rem;
        }}

        .critical-path {{
            margin-top: 1rem;
            padding: 1rem;
            background: var(--bg-secondary);
            border-radius: 0.5rem;
        }}

        .critical-path-flow {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            flex-wrap: wrap;
        }}

        .path-node {{
            padding: 0.5rem 1rem;
            background: var(--accent-color);
            color: white;
            border-radius: 0.25rem;
            font-weight: 500;
        }}

        .path-arrow {{
            color: var(--text-secondary);
        }}

        @media print {{
            body {{ background: white; color: black; }}
            .theme-toggle {{ display: none; }}
            .section {{ break-inside: avoid; }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div>
                <h1>🚀 PipelineX Analysis Report</h1>
                <p style="color: var(--text-secondary); margin-top: 0.5rem;">
                    {pipeline_name} • {provider}
                </p>
            </div>
            <button class="theme-toggle" onclick="toggleTheme()">
                🌓 Toggle Theme
            </button>
        </div>

        <div class="stats-grid">
            <div class="stat-card">
                <div class="stat-label">Pipeline Duration</div>
                <div class="stat-value">{duration}</div>
            </div>
            <div class="stat-card">
                <div class="stat-label">Potential Savings</div>
                <div class="stat-value" style="color: var(--success-color);">{savings}%</div>
            </div>
            <div class="stat-card">
                <div class="stat-label">Jobs / Steps</div>
                <div class="stat-value">{job_count} / {step_count}</div>
            </div>
            <div class="stat-card">
                <div class="stat-label">Max Parallelism</div>
                <div class="stat-value">{max_parallelism}</div>
            </div>
        </div>

        <div class="section">
            <h2 class="section-title">📊 Pipeline Visualization</h2>
            <div class="dag-container">
                <canvas id="dagCanvas" width="1000" height="600"></canvas>
            </div>
            <div class="critical-path">
                <strong>Critical Path:</strong>
                <div class="critical-path-flow" id="criticalPathFlow"></div>
            </div>
        </div>

        <div class="section">
            <h2 class="section-title">🔍 Findings ({findings_count})</h2>
            <div id="findingsContainer"></div>
        </div>

        <div class="section">
            <h2 class="section-title">💡 Recommendations</h2>
            <div style="color: var(--text-secondary); line-height: 1.8;">
                <p><strong>1. Run optimization:</strong> <code>pipelinex optimize {source_file}</code></p>
                <p><strong>2. Visualize DAG:</strong> <code>pipelinex graph {source_file}</code></p>
                <p><strong>3. Simulate timing:</strong> <code>pipelinex simulate {source_file}</code></p>
                <p><strong>4. Estimate costs:</strong> <code>pipelinex cost {source_file}</code></p>
            </div>
        </div>
    </div>

    <script>
        // Data
        const findings = {findings_json};
        const criticalPath = {critical_path_json};
        const dagNodes = {dag_nodes};
        const dagEdges = {dag_edges};

        // Theme toggle
        function toggleTheme() {{
            const current = document.documentElement.getAttribute('data-theme');
            const next = current === 'dark' ? 'light' : 'dark';
            document.documentElement.setAttribute('data-theme', next);
            localStorage.setItem('theme', next);
            renderDAG();
        }}

        // Load saved theme
        const savedTheme = localStorage.getItem('theme') || 'light';
        document.documentElement.setAttribute('data-theme', savedTheme);

        // Render findings
        function renderFindings() {{
            const container = document.getElementById('findingsContainer');
            if (findings.length === 0) {{
                container.innerHTML = '<p style="color: var(--text-secondary);">No findings - your pipeline looks great! ✨</p>';
                return;
            }}

            container.innerHTML = findings.map(f => `
                <div class="finding ${{f.severity.toLowerCase()}}">
                    <div class="finding-header">
                        <div class="finding-title">${{f.title}}</div>
                        <div class="severity-badge ${{f.severity.toLowerCase()}}">${{f.severity}}</div>
                    </div>
                    <div class="finding-description">${{f.description}}</div>
                    <div class="finding-meta">
                        <span>💾 Savings: ${{f.estimated_savings_secs ? Math.floor(f.estimated_savings_secs / 60) + ' min' : 'N/A'}}</span>
                        <span>🎯 Confidence: ${{Math.round(f.confidence * 100)}}%</span>
                        ${{f.auto_fixable ? '<span>🔧 Auto-fixable</span>' : ''}}
                    </div>
                    ${{f.recommendation ? `<div style="margin-top: 0.75rem; padding: 0.75rem; background: var(--bg-primary); border-radius: 0.25rem; font-size: 0.875rem;"><strong>💡 Recommendation:</strong> ${{f.recommendation}}</div>` : ''}}
                </div>
            `).join('');
        }}

        // Render critical path
        function renderCriticalPath() {{
            const container = document.getElementById('criticalPathFlow');
            if (criticalPath.length === 0) return;

            container.innerHTML = criticalPath.map((node, i) =>
                `<span class="path-node">${{node}}</span>${{i < criticalPath.length - 1 ? '<span class="path-arrow">→</span>' : ''}}`
            ).join('');
        }}

        // Simple DAG renderer
        function renderDAG() {{
            const canvas = document.getElementById('dagCanvas');
            const ctx = canvas.getContext('2d');
            const isDark = document.documentElement.getAttribute('data-theme') === 'dark';

            // Clear
            ctx.clearRect(0, 0, canvas.width, canvas.height);

            if (dagNodes.length === 0) return;

            // Calculate layout (simple hierarchical)
            const levels = {{}};
            const nodeWidth = 140;
            const nodeHeight = 50;
            const levelHeight = 100;
            const startY = 50;

            dagNodes.forEach(node => {{
                const level = node.level || 0;
                if (!levels[level]) levels[level] = [];
                levels[level].push(node);
            }});

            // Position nodes
            const positions = {{}};
            Object.keys(levels).forEach(level => {{
                const nodesInLevel = levels[level];
                const totalWidth = nodesInLevel.length * (nodeWidth + 40);
                const startX = (canvas.width - totalWidth) / 2;

                nodesInLevel.forEach((node, i) => {{
                    positions[node.id] = {{
                        x: startX + i * (nodeWidth + 40) + nodeWidth / 2,
                        y: startY + parseInt(level) * levelHeight
                    }};
                }});
            }});

            // Draw edges
            ctx.strokeStyle = isDark ? '#64748b' : '#cbd5e1';
            ctx.lineWidth = 2;
            dagEdges.forEach(edge => {{
                const from = positions[edge.from];
                const to = positions[edge.to];
                if (from && to) {{
                    ctx.beginPath();
                    ctx.moveTo(from.x, from.y + nodeHeight / 2);
                    ctx.lineTo(to.x, to.y - nodeHeight / 2);
                    ctx.stroke();
                }}
            }});

            // Draw nodes
            dagNodes.forEach(node => {{
                const pos = positions[node.id];
                if (!pos) return;

                const x = pos.x - nodeWidth / 2;
                const y = pos.y - nodeHeight / 2;

                // Background
                ctx.fillStyle = isDark ? '#1e293b' : '#ffffff';
                ctx.strokeStyle = isDark ? '#475569' : '#e2e8f0';
                ctx.lineWidth = 2;
                ctx.fillRect(x, y, nodeWidth, nodeHeight);
                ctx.strokeRect(x, y, nodeWidth, nodeHeight);

                // Text
                ctx.fillStyle = isDark ? '#f1f5f9' : '#1e293b';
                ctx.font = 'bold 14px sans-serif';
                ctx.textAlign = 'center';
                ctx.textBaseline = 'middle';
                ctx.fillText(node.name, pos.x, pos.y - 8);

                // Duration
                ctx.font = '12px sans-serif';
                ctx.fillStyle = isDark ? '#94a3b8' : '#64748b';
                ctx.fillText(node.duration, pos.x, pos.y + 10);
            }});
        }}

        // Initialize
        renderFindings();
        renderCriticalPath();
        renderDAG();

        // Re-render on window resize
        window.addEventListener('resize', renderDAG);
    </script>
</body>
</html>"#,
        pipeline_name = escape_html(&report.pipeline_name),
        provider = escape_html(&report.provider),
        duration = format_duration(report.total_estimated_duration_secs),
        savings = format!("{:.1}", report.potential_improvement_pct()),
        job_count = report.job_count,
        step_count = report.step_count,
        max_parallelism = report.max_parallelism,
        findings_count = report.findings.len(),
        source_file = escape_html(&report.source_file),
        findings_json = findings_json,
        critical_path_json = critical_path_json,
        dag_nodes = dag_nodes,
        dag_edges = dag_edges,
    )
}

fn generate_dag_nodes_json(dag: &PipelineDag) -> String {
    // Simple level calculation - just use index as a rough approximation
    let nodes: Vec<serde_json::Value> = dag
        .graph
        .node_indices()
        .enumerate()
        .map(|(level, idx)| {
            let node = &dag.graph[idx];
            serde_json::json!({
                "id": node.id,
                "name": node.name,
                "duration": format_duration(node.estimated_duration_secs),
                "level": level
            })
        })
        .collect();

    serde_json::to_string(&nodes).unwrap_or_else(|_| "[]".to_string())
}

fn generate_dag_edges_json(dag: &PipelineDag) -> String {
    let edges: Vec<serde_json::Value> = dag
        .graph
        .edge_indices()
        .filter_map(|idx| {
            let (from, to) = dag.graph.edge_endpoints(idx)?;
            let from_node = &dag.graph[from];
            let to_node = &dag.graph[to];
            Some(serde_json::json!({
                "from": from_node.id,
                "to": to_node.id
            }))
        })
        .collect();

    serde_json::to_string(&edges).unwrap_or_else(|_| "[]".to_string())
}

fn format_duration(secs: f64) -> String {
    let minutes = (secs / 60.0).floor() as u32;
    let seconds = (secs % 60.0).floor() as u32;

    if minutes > 0 {
        format!("{}:{:02}", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
