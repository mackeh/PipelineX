use serde::{Deserialize, Serialize};

/// Represents a parsed Dockerfile instruction.
#[derive(Debug, Clone)]
pub struct DockerInstruction {
    pub instruction: String,
    pub arguments: String,
    pub line_number: usize,
}

/// Findings from Dockerfile analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerFinding {
    pub severity: DockerSeverity,
    pub title: String,
    pub description: String,
    pub line_number: Option<usize>,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockerSeverity {
    Critical,
    Warning,
    Info,
}

/// Result of Dockerfile analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerAnalysis {
    pub findings: Vec<DockerFinding>,
    pub optimized_dockerfile: Option<String>,
    pub estimated_build_time_before: f64,
    pub estimated_build_time_after: f64,
}

/// Analyze a Dockerfile for optimization opportunities.
pub fn analyze_dockerfile(content: &str) -> DockerAnalysis {
    let instructions = parse_dockerfile(content);
    let mut findings = Vec::new();

    // Detect non-slim base image
    check_base_image(&instructions, &mut findings);

    // Detect COPY . . before dependency install
    check_copy_before_install(&instructions, &mut findings);

    // Detect missing multi-stage build
    check_multi_stage(&instructions, &mut findings);

    // Detect running as root
    check_user(&instructions, &mut findings);

    // Detect apt-get without cleanup
    check_apt_cleanup(&instructions, &mut findings);

    // Detect npm/yarn using npm start instead of node directly
    check_cmd_optimization(&instructions, &mut findings);

    // Detect missing .dockerignore advice
    check_dockerignore(&instructions, &mut findings);

    // Detect multiple RUN commands that could be combined
    check_run_consolidation(&instructions, &mut findings);

    let optimized = generate_optimized_dockerfile(&instructions, &findings);

    let before = estimate_build_time(&instructions, false);
    let after = estimate_build_time(&instructions, true);

    DockerAnalysis {
        findings,
        optimized_dockerfile: Some(optimized),
        estimated_build_time_before: before,
        estimated_build_time_after: after,
    }
}

fn parse_dockerfile(content: &str) -> Vec<DockerInstruction> {
    let mut instructions = Vec::new();
    let mut continuation = String::new();
    let mut line_start = 0;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if continuation.is_empty() {
            line_start = i + 1;
        }

        // Handle line continuations
        if let Some(stripped) = trimmed.strip_suffix('\\') {
            continuation.push_str(stripped);
            continuation.push(' ');
            continue;
        }

        let full_line = if continuation.is_empty() {
            trimmed.to_string()
        } else {
            continuation.push_str(trimmed);
            let result = continuation.clone();
            continuation.clear();
            result
        };

        if let Some((instr, args)) = full_line.split_once(char::is_whitespace) {
            instructions.push(DockerInstruction {
                instruction: instr.to_uppercase(),
                arguments: args.trim().to_string(),
                line_number: line_start,
            });
        } else {
            instructions.push(DockerInstruction {
                instruction: full_line.to_uppercase(),
                arguments: String::new(),
                line_number: line_start,
            });
        }
    }

    instructions
}

fn check_base_image(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    for instr in instructions {
        if instr.instruction == "FROM" {
            let image = instr.arguments.split_whitespace().next().unwrap_or("");
            let image_lower = image.to_lowercase();

            if (image_lower.starts_with("node:") || image_lower.starts_with("python:")
                || image_lower.starts_with("ruby:") || image_lower.starts_with("golang:"))
                && !image_lower.contains("slim")
                && !image_lower.contains("alpine")
                && !image_lower.contains("distroless")
            {
                findings.push(DockerFinding {
                    severity: DockerSeverity::Warning,
                    title: "Non-slim base image".to_string(),
                    description: format!(
                        "Using '{}' — the full image is much larger than needed. \
                        Slim variants are 3-5x smaller and build faster.",
                        image
                    ),
                    line_number: Some(instr.line_number),
                    fix: format!(
                        "Use '{}-slim' or '{}-alpine' instead.",
                        image.split(':').next().unwrap_or(image),
                        image.split(':').next().unwrap_or(image)
                    ),
                });
            }
        }
    }
}

fn check_copy_before_install(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    let mut seen_copy_all = false;
    let mut copy_all_line = 0;

    for instr in instructions {
        if instr.instruction == "COPY" {
            let args = &instr.arguments;
            if args.starts_with(". ") || args == "." || args.starts_with("./ ") {
                seen_copy_all = true;
                copy_all_line = instr.line_number;
            }
        }

        if seen_copy_all && instr.instruction == "RUN" {
            let cmd = instr.arguments.to_lowercase();
            if cmd.contains("npm ci") || cmd.contains("npm install")
                || cmd.contains("pip install") || cmd.contains("yarn install")
                || cmd.contains("bundle install") || cmd.contains("composer install")
                || cmd.contains("go mod download") || cmd.contains("cargo build")
            {
                findings.push(DockerFinding {
                    severity: DockerSeverity::Critical,
                    title: "COPY . . before dependency install busts cache".to_string(),
                    description: format!(
                        "Line {}: COPY . . copies all files before installing dependencies. \
                        Any source code change invalidates the cache for dependency installation. \
                        Copy only lockfiles first, install deps, then copy the rest.",
                        copy_all_line
                    ),
                    line_number: Some(copy_all_line),
                    fix: "Copy only package.json/lockfile first, run install, then COPY . .".to_string(),
                });
                break;
            }
        }
    }
}

fn check_multi_stage(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    let from_count = instructions.iter()
        .filter(|i| i.instruction == "FROM")
        .count();

    let has_build_step = instructions.iter().any(|i| {
        i.instruction == "RUN" && {
            let cmd = i.arguments.to_lowercase();
            cmd.contains("npm run build") || cmd.contains("yarn build")
                || cmd.contains("cargo build") || cmd.contains("go build")
                || cmd.contains("mvn package") || cmd.contains("gradle build")
        }
    });

    if from_count <= 1 && has_build_step {
        findings.push(DockerFinding {
            severity: DockerSeverity::Warning,
            title: "No multi-stage build".to_string(),
            description: "This Dockerfile builds the application but uses a single stage. \
                Multi-stage builds separate build dependencies from the runtime image, \
                resulting in much smaller final images."
                .to_string(),
            line_number: None,
            fix: "Use a multi-stage build: build in one stage, copy only artifacts to a slim runtime stage.".to_string(),
        });
    }
}

fn check_user(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    let has_user = instructions.iter().any(|i| i.instruction == "USER");

    if !has_user {
        findings.push(DockerFinding {
            severity: DockerSeverity::Warning,
            title: "Container runs as root".to_string(),
            description: "No USER instruction found. The container will run as root, \
                which is a security risk."
                .to_string(),
            line_number: None,
            fix: "Add 'USER node' (or appropriate non-root user) before CMD.".to_string(),
        });
    }
}

fn check_apt_cleanup(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    for instr in instructions {
        if instr.instruction == "RUN" {
            let cmd = &instr.arguments;
            if (cmd.contains("apt-get install") || cmd.contains("apt install"))
                && !cmd.contains("rm -rf /var/lib/apt")
                && !cmd.contains("apt-get clean")
            {
                findings.push(DockerFinding {
                    severity: DockerSeverity::Info,
                    title: "apt-get install without cleanup".to_string(),
                    description: "Package cache is not cleaned after apt-get install, \
                        bloating the image layer."
                        .to_string(),
                    line_number: Some(instr.line_number),
                    fix: "Add '&& rm -rf /var/lib/apt/lists/*' after apt-get install.".to_string(),
                });
                break;
            }
        }
    }
}

fn check_cmd_optimization(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    for instr in instructions {
        if instr.instruction == "CMD" || instr.instruction == "ENTRYPOINT" {
            let args = instr.arguments.to_lowercase();
            if args.contains("npm start") || args.contains("npm run start")
                || (args.contains("npm") && args.contains("start")) {
                findings.push(DockerFinding {
                    severity: DockerSeverity::Info,
                    title: "Using npm to start the application".to_string(),
                    description: "CMD uses npm start, which spawns an extra process and \
                        doesn't forward signals properly. Use 'node' directly for faster \
                        startup and proper graceful shutdown."
                        .to_string(),
                    line_number: Some(instr.line_number),
                    fix: "Use CMD [\"node\", \"dist/index.js\"] instead of CMD [\"npm\", \"start\"].".to_string(),
                });
            }
        }
    }
}

fn check_dockerignore(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    let has_copy_all = instructions.iter().any(|i| {
        i.instruction == "COPY" && (i.arguments.starts_with(". ") || i.arguments.starts_with("./ "))
    });

    if has_copy_all {
        findings.push(DockerFinding {
            severity: DockerSeverity::Info,
            title: "Ensure .dockerignore exists".to_string(),
            description: "COPY . . is used — make sure .dockerignore excludes node_modules, \
                .git, and other unnecessary files to speed up the build context transfer."
                .to_string(),
            line_number: None,
            fix: "Create a .dockerignore with: node_modules, .git, *.md, .env, dist, coverage".to_string(),
        });
    }
}

fn check_run_consolidation(instructions: &[DockerInstruction], findings: &mut Vec<DockerFinding>) {
    let mut consecutive_runs = 0;
    let mut first_run_line = 0;

    for instr in instructions {
        if instr.instruction == "RUN" {
            if consecutive_runs == 0 {
                first_run_line = instr.line_number;
            }
            consecutive_runs += 1;
        } else {
            if consecutive_runs > 2 {
                findings.push(DockerFinding {
                    severity: DockerSeverity::Info,
                    title: format!("{} consecutive RUN instructions", consecutive_runs),
                    description: format!(
                        "Lines {}-{}: Multiple RUN instructions create separate layers. \
                        Combining them with '&&' reduces image size.",
                        first_run_line, instr.line_number - 1
                    ),
                    line_number: Some(first_run_line),
                    fix: "Combine RUN instructions using '&&' and line continuations '\\'.".to_string(),
                });
            }
            consecutive_runs = 0;
        }
    }
}

fn generate_optimized_dockerfile(instructions: &[DockerInstruction], findings: &[DockerFinding]) -> String {
    let has_copy_before_install = findings.iter()
        .any(|f| f.title.contains("COPY . . before dependency install"));
    let has_non_slim = findings.iter()
        .any(|f| f.title.contains("Non-slim base image"));
    let has_no_multistage = findings.iter()
        .any(|f| f.title.contains("No multi-stage build"));
    let has_no_user = findings.iter()
        .any(|f| f.title.contains("runs as root"));

    // Detect the ecosystem
    let is_node = instructions.iter().any(|i| {
        (i.instruction == "FROM" && i.arguments.to_lowercase().contains("node"))
            || (i.instruction == "RUN" && i.arguments.to_lowercase().contains("npm"))
    });

    let is_python = instructions.iter().any(|i| {
        (i.instruction == "FROM" && i.arguments.to_lowercase().contains("python"))
            || (i.instruction == "RUN" && i.arguments.to_lowercase().contains("pip"))
    });

    let _is_rust = instructions.iter().any(|i| {
        (i.instruction == "FROM" && i.arguments.to_lowercase().contains("rust"))
            || (i.instruction == "RUN" && i.arguments.to_lowercase().contains("cargo"))
    });

    let is_go = instructions.iter().any(|i| {
        (i.instruction == "FROM" && i.arguments.to_lowercase().contains("golang"))
            || (i.instruction == "RUN" && i.arguments.to_lowercase().contains("go build"))
    });

    // Generate optimized Dockerfile based on ecosystem
    if is_node && (has_copy_before_install || has_no_multistage) {
        return generate_node_dockerfile(instructions);
    }
    if is_python && (has_copy_before_install || has_no_multistage) {
        return generate_python_dockerfile(instructions);
    }
    if is_go && has_no_multistage {
        return generate_go_dockerfile(instructions);
    }

    // Fallback: annotated copy of original with fixes applied inline
    let mut lines: Vec<String> = Vec::new();
    lines.push("# Optimized by PipelineX".to_string());
    for instr in instructions {
        if instr.instruction == "FROM" && has_non_slim {
            let args = &instr.arguments;
            if let Some(colon) = args.find(':') {
                let (base, tag) = args.split_at(colon);
                if !tag.contains("slim") && !tag.contains("alpine") {
                    lines.push(format!("FROM {}-slim{}", base, tag));
                    continue;
                }
            }
        }
        lines.push(format!("{} {}", instr.instruction, instr.arguments));
    }
    if has_no_user && is_node {
        lines.push("USER node".to_string());
    }
    lines.join("\n")
}

fn generate_node_dockerfile(instructions: &[DockerInstruction]) -> String {
    let base_image = instructions.iter()
        .find(|i| i.instruction == "FROM")
        .map(|i| {
            let img = i.arguments.split_whitespace().next().unwrap_or("node:20");
            if img.contains("slim") || img.contains("alpine") {
                img.to_string()
            } else {
                format!("{}-slim", img)
            }
        })
        .unwrap_or_else(|| "node:20-slim".to_string());

    let workdir = instructions.iter()
        .find(|i| i.instruction == "WORKDIR")
        .map(|i| i.arguments.clone())
        .unwrap_or_else(|| "/app".to_string());

    let expose = instructions.iter()
        .find(|i| i.instruction == "EXPOSE")
        .map(|i| format!("EXPOSE {}", i.arguments))
        .unwrap_or_else(|| "EXPOSE 3000".to_string());

    format!(r#"# Optimized by PipelineX — multi-stage Node.js build
# Estimated build time: ~45s (cached), ~2:30 (cold)

# Stage 1: Install dependencies
FROM {base} AS deps
WORKDIR {workdir}
COPY package.json package-lock.json* yarn.lock* pnpm-lock.yaml* ./
RUN npm ci

# Stage 2: Build application
FROM deps AS build
COPY . .
RUN npm run build
RUN npm prune --production

# Stage 3: Production runtime
FROM {base} AS runtime
WORKDIR {workdir}
COPY --from=build {workdir}/node_modules ./node_modules
COPY --from=build {workdir}/dist ./dist
COPY --from=build {workdir}/package.json .
{expose}
USER node
CMD ["node", "dist/index.js"]
"#, base = base_image, workdir = workdir, expose = expose)
}

fn generate_python_dockerfile(instructions: &[DockerInstruction]) -> String {
    let base_image = instructions.iter()
        .find(|i| i.instruction == "FROM")
        .map(|i| {
            let img = i.arguments.split_whitespace().next().unwrap_or("python:3.12");
            if img.contains("slim") || img.contains("alpine") {
                img.to_string()
            } else {
                format!("{}-slim", img)
            }
        })
        .unwrap_or_else(|| "python:3.12-slim".to_string());

    format!(r#"# Optimized by PipelineX — Python multi-stage build

# Stage 1: Install dependencies
FROM {base} AS deps
WORKDIR /app
RUN pip install --upgrade pip
COPY requirements*.txt ./
RUN pip install --no-cache-dir -r requirements.txt

# Stage 2: Runtime
FROM {base} AS runtime
WORKDIR /app
COPY --from=deps /usr/local/lib/python3.12/site-packages /usr/local/lib/python3.12/site-packages
COPY --from=deps /usr/local/bin /usr/local/bin
COPY . .
RUN useradd -m appuser
USER appuser
EXPOSE 8000
CMD ["python", "-m", "gunicorn", "app:app", "--bind", "0.0.0.0:8000"]
"#, base = base_image)
}

fn generate_go_dockerfile(_instructions: &[DockerInstruction]) -> String {
    r#"# Optimized by PipelineX — Go multi-stage build

# Stage 1: Build
FROM golang:1.22-alpine AS build
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /app/server .

# Stage 2: Runtime (distroless for minimal attack surface)
FROM gcr.io/distroless/static-debian12
COPY --from=build /app/server /server
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/server"]
"#.to_string()
}

fn estimate_build_time(instructions: &[DockerInstruction], optimized: bool) -> f64 {
    let mut total = 0.0;
    for instr in instructions {
        if instr.instruction == "RUN" {
            let cmd = instr.arguments.to_lowercase();
            if cmd.contains("npm ci") || cmd.contains("npm install") {
                total += if optimized { 15.0 } else { 180.0 };
            } else if cmd.contains("npm run build") {
                total += if optimized { 60.0 } else { 240.0 };
            } else if cmd.contains("pip install") {
                total += if optimized { 10.0 } else { 120.0 };
            } else if cmd.contains("cargo build") {
                total += if optimized { 60.0 } else { 300.0 };
            } else if cmd.contains("go build") {
                total += if optimized { 30.0 } else { 120.0 };
            } else if cmd.contains("apt-get") || cmd.contains("apk add") {
                total += 30.0;
            } else {
                total += 10.0;
            }
        } else if instr.instruction == "COPY" {
            total += if optimized { 2.0 } else { 5.0 };
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_copy_before_install() {
        let dockerfile = r#"
FROM node:20
WORKDIR /app
COPY . .
RUN npm install
RUN npm run build
CMD ["npm", "start"]
"#;
        let analysis = analyze_dockerfile(dockerfile);
        assert!(analysis.findings.iter().any(|f| f.title.contains("COPY . . before")));
        assert!(analysis.findings.iter().any(|f| f.title.contains("Non-slim")));
        assert!(analysis.findings.iter().any(|f| f.title.contains("runs as root")));
        assert!(analysis.findings.iter().any(|f| f.title.contains("npm")));
        assert!(analysis.optimized_dockerfile.is_some());
    }

    #[test]
    fn test_optimized_node_dockerfile() {
        let dockerfile = r#"
FROM node:20
WORKDIR /app
COPY . .
RUN npm ci
RUN npm run build
EXPOSE 3000
CMD ["npm", "start"]
"#;
        let analysis = analyze_dockerfile(dockerfile);
        let optimized = analysis.optimized_dockerfile.unwrap();
        assert!(optimized.contains("multi-stage"));
        assert!(optimized.contains("AS deps"));
        assert!(optimized.contains("AS runtime"));
        assert!(optimized.contains("USER node"));
    }

    #[test]
    fn test_clean_dockerfile_fewer_findings() {
        let dockerfile = r#"
FROM node:20-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:20-slim
WORKDIR /app
COPY --from=build /app/dist ./dist
COPY --from=build /app/node_modules ./node_modules
USER node
CMD ["node", "dist/index.js"]
"#;
        let analysis = analyze_dockerfile(dockerfile);
        // Should have fewer critical findings
        let critical = analysis.findings.iter()
            .filter(|f| matches!(f.severity, DockerSeverity::Critical))
            .count();
        assert_eq!(critical, 0);
    }
}
