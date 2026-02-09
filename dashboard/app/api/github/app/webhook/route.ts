import { createHmac, timingSafeEqual } from "node:crypto";
import { NextResponse } from "next/server";
import {
  analyzePipelineContent,
  isSupportedPipelineFile,
  type AnalysisReport,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

const COMMENT_MARKER = "<!-- pipelinex-pr-analysis -->";
const HANDLED_ACTIONS = new Set(["opened", "reopened", "synchronize", "ready_for_review"]);

interface PullRequestWebhookPayload {
  action?: string;
  repository?: {
    full_name?: string;
  };
  pull_request?: {
    number?: number;
    html_url?: string;
    title?: string;
    draft?: boolean;
    head?: {
      sha?: string;
    };
  };
}

interface PullRequestFile {
  filename: string;
}

interface IssueComment {
  id: number;
  body?: string;
}

function isValidGithubSignature(
  body: string,
  signatureHeader: string | null,
  secret: string,
): boolean {
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) {
    return false;
  }

  const computed = `sha256=${createHmac("sha256", secret).update(body).digest("hex")}`;
  const expectedBuffer = Buffer.from(computed);
  const receivedBuffer = Buffer.from(signatureHeader);

  if (expectedBuffer.length !== receivedBuffer.length) {
    return false;
  }

  return timingSafeEqual(expectedBuffer, receivedBuffer);
}

function githubHeaders(token: string): HeadersInit {
  return {
    Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
    "User-Agent": "PipelineX-GitHub-App",
  };
}

function encodeGitHubPath(filePath: string): string {
  return filePath
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

async function githubRequestJson<T>(
  url: string,
  token: string,
  init: RequestInit = {},
): Promise<T> {
  const response = await fetch(url, {
    ...init,
    headers: {
      ...githubHeaders(token),
      ...(init.headers || {}),
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`GitHub API request failed (${response.status}): ${text.slice(0, 500)}`);
  }

  return (await response.json()) as T;
}

async function listPullRequestFiles(
  repo: string,
  pullNumber: number,
  token: string,
): Promise<PullRequestFile[]> {
  const url = `https://api.github.com/repos/${repo}/pulls/${pullNumber}/files?per_page=100`;
  return githubRequestJson<PullRequestFile[]>(url, token);
}

async function fetchRepoFileContent(
  repo: string,
  filePath: string,
  ref: string,
  token: string,
): Promise<string> {
  const encodedPath = encodeGitHubPath(filePath);
  const url = `https://api.github.com/repos/${repo}/contents/${encodedPath}?ref=${encodeURIComponent(
    ref,
  )}`;

  const response = await fetch(url, {
    headers: {
      ...githubHeaders(token),
      Accept: "application/vnd.github.raw",
    },
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Failed to fetch file '${filePath}' (${response.status}): ${text.slice(0, 300)}`);
  }

  return response.text();
}

function formatDuration(seconds: number): string {
  const safeSeconds = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  const remainder = safeSeconds % 60;
  return `${minutes}m ${remainder}s`;
}

function countBySeverity(report: AnalysisReport): {
  critical: number;
  high: number;
  medium: number;
} {
  let critical = 0;
  let high = 0;
  let medium = 0;
  for (const finding of report.findings) {
    const severity = finding.severity.toLowerCase();
    if (severity === "critical") {
      critical += 1;
    } else if (severity === "high") {
      high += 1;
    } else if (severity === "medium") {
      medium += 1;
    }
  }
  return { critical, high, medium };
}

function buildCommentBody(
  repo: string,
  pullNumber: number,
  analyses: Array<{ file: string; report: AnalysisReport }>,
  failures: Array<{ file: string; error: string }>,
): string {
  const lines: string[] = [];
  lines.push(COMMENT_MARKER);
  lines.push("## PipelineX PR Analysis");
  lines.push("");
  lines.push(`Repository: \`${repo}\``);
  lines.push(`Pull Request: #${pullNumber}`);
  lines.push("");

  if (analyses.length === 0) {
    lines.push("No supported CI workflow files were analyzed for this PR.");
  } else {
    for (const item of analyses) {
      const report = item.report;
      const savings = Math.max(0, report.total_estimated_duration_secs - report.optimized_duration_secs);
      const savingsPct =
        (savings / Math.max(1, report.total_estimated_duration_secs)) * 100;
      const severities = countBySeverity(report);

      lines.push(`### \`${item.file}\``);
      lines.push(
        `- Provider: **${report.provider}** | Current: **${formatDuration(
          report.total_estimated_duration_secs,
        )}** | Optimized: **${formatDuration(report.optimized_duration_secs)}**`,
      );
      lines.push(
        `- Potential savings: **${formatDuration(savings)}** (${savingsPct.toFixed(
          1,
        )}%) | Findings: **${report.findings.length}** (${severities.critical} critical / ${severities.high} high / ${severities.medium} medium)`,
      );

      const top = report.findings
        .filter((finding) => {
          const severity = finding.severity.toLowerCase();
          return severity === "critical" || severity === "high";
        })
        .slice(0, 3);

      if (top.length > 0) {
        lines.push("- Top hotspots:");
        for (const finding of top) {
          lines.push(`  - [${finding.severity}] ${finding.title}`);
        }
      } else {
        lines.push("- No critical/high findings detected.");
      }

      lines.push("");
    }
  }

  if (failures.length > 0) {
    lines.push("### Analysis Warnings");
    for (const failure of failures.slice(0, 5)) {
      lines.push(`- \`${failure.file}\`: ${failure.error}`);
    }
    if (failures.length > 5) {
      lines.push(`- ... and ${failures.length - 5} more`);
    }
    lines.push("");
  }

  lines.push("_Automated by PipelineX GitHub App integration_");
  return lines.join("\n");
}

async function listPullRequestComments(
  repo: string,
  pullNumber: number,
  token: string,
): Promise<IssueComment[]> {
  const url = `https://api.github.com/repos/${repo}/issues/${pullNumber}/comments?per_page=100`;
  return githubRequestJson<IssueComment[]>(url, token);
}

async function upsertPullRequestComment(
  repo: string,
  pullNumber: number,
  token: string,
  body: string,
): Promise<"created" | "updated"> {
  const comments = await listPullRequestComments(repo, pullNumber, token);
  const existing = comments.find((comment) => comment.body?.includes(COMMENT_MARKER));

  if (existing) {
    const patchUrl = `https://api.github.com/repos/${repo}/issues/comments/${existing.id}`;
    await githubRequestJson(patchUrl, token, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ body }),
    });
    return "updated";
  }

  const createUrl = `https://api.github.com/repos/${repo}/issues/${pullNumber}/comments`;
  await githubRequestJson(createUrl, token, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ body }),
  });
  return "created";
}

export async function POST(request: Request) {
  const event = request.headers.get("x-github-event") || "unknown";
  const deliveryId = request.headers.get("x-github-delivery") || undefined;
  const signatureHeader = request.headers.get("x-hub-signature-256");
  const rawBody = await request.text();

  const webhookSecret =
    process.env.GITHUB_APP_WEBHOOK_SECRET?.trim() ||
    process.env.GITHUB_WEBHOOK_SECRET?.trim() ||
    "";

  if (webhookSecret && !isValidGithubSignature(rawBody, signatureHeader, webhookSecret)) {
    return NextResponse.json({ error: "Invalid webhook signature." }, { status: 401 });
  }

  let payload: PullRequestWebhookPayload = {};
  try {
    payload = JSON.parse(rawBody) as PullRequestWebhookPayload;
  } catch {
    return NextResponse.json({ error: "Invalid JSON payload." }, { status: 400 });
  }

  if (event !== "pull_request") {
    return NextResponse.json({
      ok: true,
      event,
      ignored: true,
      reason: "Only pull_request events are handled.",
    });
  }

  const action = payload.action || "unknown";
  if (!HANDLED_ACTIONS.has(action)) {
    return NextResponse.json({
      ok: true,
      event,
      action,
      ignored: true,
      reason: "Action is not in handled PR actions set.",
    });
  }

  const repository = payload.repository?.full_name;
  const pullNumber = payload.pull_request?.number;
  const headSha = payload.pull_request?.head?.sha;
  const draft = payload.pull_request?.draft ?? false;

  if (!repository || !pullNumber || !headSha) {
    return NextResponse.json(
      {
        ok: false,
        event,
        action,
        error: "Missing repository, pull_request.number, or pull_request.head.sha.",
      },
      { status: 400 },
    );
  }

  if (draft) {
    return NextResponse.json({
      ok: true,
      event,
      action,
      ignored: true,
      reason: "Draft pull requests are ignored.",
    });
  }

  const githubToken =
    process.env.GITHUB_APP_TOKEN?.trim() || process.env.GITHUB_TOKEN?.trim() || "";
  if (!githubToken) {
    return NextResponse.json(
      {
        ok: false,
        event,
        action,
        error: "Missing GITHUB_APP_TOKEN (or fallback GITHUB_TOKEN) for PR comment actions.",
      },
      { status: 503 },
    );
  }

  try {
    const changedFiles = await listPullRequestFiles(repository, pullNumber, githubToken);
    const candidateFiles = changedFiles
      .map((file) => file.filename)
      .filter((filename) => isSupportedPipelineFile(filename));

    if (candidateFiles.length === 0) {
      return NextResponse.json({
        ok: true,
        event,
        action,
        repository,
        pull_number: pullNumber,
        delivery_id: deliveryId,
        analyzed_files: 0,
        commented: false,
        reason: "No supported workflow files changed in this PR.",
      });
    }

    const analyses: Array<{ file: string; report: AnalysisReport }> = [];
    const failures: Array<{ file: string; error: string }> = [];

    for (const filename of candidateFiles.slice(0, 10)) {
      try {
        const content = await fetchRepoFileContent(repository, filename, headSha, githubToken);
        const report = await analyzePipelineContent(filename, content);
        analyses.push({ file: filename, report });
      } catch (error) {
        failures.push({
          file: filename,
          error:
            error instanceof Error
              ? error.message.replace(/\s+/g, " ").slice(0, 240)
              : "Failed to analyze workflow file.",
        });
      }
    }

    const commentBody = buildCommentBody(repository, pullNumber, analyses, failures);
    const commentResult = await upsertPullRequestComment(
      repository,
      pullNumber,
      githubToken,
      commentBody,
    );

    return NextResponse.json({
      ok: true,
      event,
      action,
      repository,
      pull_number: pullNumber,
      delivery_id: deliveryId,
      analyzed_files: analyses.length,
      failed_files: failures.length,
      comment: commentResult,
    });
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        event,
        action,
        repository,
        pull_number: pullNumber,
        error:
          error instanceof Error
            ? error.message
            : "Failed to process GitHub App webhook.",
      },
      { status: 500 },
    );
  }
}
