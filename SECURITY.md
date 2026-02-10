# Security Policy

## Supported Versions

PipelineX is committed to providing security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 2.1.x   | ✅ Yes             |
| 2.0.x   | ✅ Yes             |
| 1.x     | 🛡️ Security Fixes Only |
| < 1.0   | ❌ No              |

## Reporting a Vulnerability

We take the security of PipelineX seriously. If you believe you have found a security vulnerability, please **do not** open a public issue. Instead, please report it privately.

Please send an email to **mackeh2010@gmail.com** with the following details:
- A description of the vulnerability.
- Steps to reproduce the issue.
- Potential impact of the vulnerability.
- Any suggested fixes (if applicable).

You can expect an acknowledgment of your report within 48 hours. We will work with you to resolve the issue and coordinate a disclosure timeline.

## Security Best Practices for Users

### 1. Minimize Token Permissions
When using `pipelinex history` or `pipelinex apply`, ensure your `GITHUB_TOKEN` or Personal Access Token (PAT) has the minimum required permissions:
- `history`: `metadata:read`, `actions:read`
- `apply`: `contents:write`, `pull_requests:write`

### 2. Run in Isolated Environments
For automated analysis in CI/CD, we recommend running PipelineX in a containerized environment (using our official Docker image) to isolate the process from your host system.

### 3. Review Optimized Configurations
While PipelineX's `optimize` command aims for safety, always review generated configurations before merging them to ensure they align with your organization's security policies.

### 4. Secret Management
PipelineX is designed to be **offline-first**. It analyzes your YAML/Groovy configurations locally and does not transmit your code or secrets to external servers (except when explicitly using the GitHub/GitLab API for history or PR creation).

## Security Features

- **No Data Collection**: PipelineX does not collect telemetry or usage data.
- **Local Execution**: Core analysis engines run entirely on your local machine.
- **Dependency Auditing**: We regularly run `cargo audit` and `npm audit` to ensure our dependencies are free of known vulnerabilities.
- **Static Analysis**: Our CI pipeline includes strict linting and static analysis to prevent common coding errors that could lead to security issues.

---

Thank you for helping keep PipelineX secure!