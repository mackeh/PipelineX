import { constants } from "node:fs";
import { access, appendFile, mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";

export interface HealthScore {
  total_score: number;
  duration_score: number;
  success_rate_score: number;
  parallelization_score: number;
  caching_score: number;
  issue_score: number;
  grade: "Excellent" | "Good" | "Fair" | "Poor" | "Critical" | string;
  recommendations: string[];
}

export interface Finding {
  severity: "Critical" | "High" | "Medium" | "Low" | "Info" | string;
  category: string;
  title: string;
  description: string;
  affected_jobs: string[];
  recommendation: string;
  fix_command: string | null;
  estimated_savings_secs: number | null;
  confidence: number;
  auto_fixable: boolean;
}

export interface AnalysisReport {
  pipeline_name: string;
  source_file: string;
  provider: string;
  job_count: number;
  step_count: number;
  max_parallelism: number;
  critical_path: string[];
  critical_path_duration_secs: number;
  total_estimated_duration_secs: number;
  optimized_duration_secs: number;
  findings: Finding[];
  health_score: HealthScore | null;
}

export interface JobTimingData {
  job_name: string;
  durations_sec: number[];
  success_count: number;
  failure_count: number;
  avg_duration_sec: number;
  p50_duration_sec: number;
  p90_duration_sec: number;
  p99_duration_sec: number;
  variance: number;
}

export interface PipelineStatistics {
  workflow_name: string;
  total_runs: number;
  success_rate: number;
  avg_duration_sec: number;
  p50_duration_sec: number;
  p90_duration_sec: number;
  p99_duration_sec: number;
  job_timings: JobTimingData[];
  flaky_jobs: string[];
}

export interface HistorySnapshot {
  repo: string;
  workflow: string;
  provider?: string;
  runs: number;
  refreshed_at: string;
  source: "manual" | "webhook";
  stats: PipelineStatistics;
  delivery_id?: string;
  workflow_run_id?: number;
}

export interface BenchmarkEntry {
  id: string;
  schema_version: number;
  submitted_at: string;
  source: string;
  provider: string;
  job_bucket: string;
  step_bucket: string;
  job_count: number;
  step_count: number;
  max_parallelism: number;
  finding_count: number;
  critical_count: number;
  high_count: number;
  medium_count: number;
  total_duration_secs: number;
  optimized_duration_secs: number;
  improvement_pct: number;
  health_score: number | null;
}

export interface BenchmarkStats {
  cohort: "provider+job+step" | "provider" | "global";
  sample_count: number;
  provider: string;
  job_bucket: string;
  step_bucket: string;
  duration_median_secs: number;
  duration_p75_secs: number;
  optimized_median_secs: number;
  improvement_median_pct: number;
  health_score_median: number | null;
  finding_median: number;
}

export interface OptimizationImpactEntry {
  id: string;
  schema_version: number;
  tracked_at: string;
  source: string;
  provider: string;
  baseline_duration_secs: number;
  optimized_duration_secs: number;
  savings_per_run_secs: number;
  savings_pct: number;
  runs_per_month: number;
  minutes_saved_per_month: number;
  hours_saved_per_month: number;
}

export interface OptimizationImpactStats {
  sample_count: number;
  source: string;
  provider: string;
  avg_minutes_saved_per_month: number;
  median_minutes_saved_per_month: number;
  p75_minutes_saved_per_month: number;
  total_minutes_saved_per_month: number;
  total_hours_saved_per_month: number;
}

export type AlertMetric =
  | "avg_duration_sec"
  | "failure_rate_pct"
  | "monthly_opportunity_cost_usd";

export type AlertOperator = "gt" | "gte" | "lt" | "lte";

export interface AlertRule {
  id: string;
  name: string;
  enabled: boolean;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  repo?: string;
  workflow?: string;
  provider?: string;
  created_at: string;
  updated_at: string;
}

export interface AlertRuleInput {
  id?: string;
  name: string;
  enabled?: boolean;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  repo?: string;
  workflow?: string;
  provider?: string;
}

export interface AlertTrigger {
  rule_id: string;
  rule_name: string;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  actual_value: number;
  repo: string;
  workflow: string;
  provider: string;
  severity: "medium" | "high" | "critical";
  message: string;
  evaluated_at: string;
}

export interface AlertEvaluationSummary {
  evaluated_at: string;
  default_runs_per_month: number;
  default_developer_hourly_rate: number;
  total_rules: number;
  enabled_rules: number;
  snapshots_considered: number;
  triggered_count: number;
  triggers: AlertTrigger[];
}

export interface WeeklyDigestPipelineSummary {
  repo: string;
  workflow: string;
  provider: string;
  refreshed_at: string;
  runs: number;
  avg_duration_sec: number;
  failure_rate_pct: number;
  estimated_monthly_opportunity_cost_usd: number;
  flaky_jobs: string[];
}

export interface WeeklyDigestSummary {
  generated_at: string;
  window_days: number;
  snapshot_count: number;
  total_runs: number;
  avg_duration_sec: number;
  failure_rate_pct: number;
  estimated_monthly_opportunity_cost_usd: number;
  top_flaky_jobs: Array<{ job: string; count: number }>;
  top_slowest_pipelines: WeeklyDigestPipelineSummary[];
  action_items: string[];
}

export interface WeeklyDigestDeliveryOptions {
  slackWebhookUrl?: string;
  teamsWebhookUrl?: string;
  emailRecipients?: string[];
  emailOutboxPath?: string;
  dryRun?: boolean;
}

export interface WeeklyDigestDeliveryResult {
  dry_run: boolean;
  slack_sent: boolean;
  teams_sent: boolean;
  email_queued: number;
  email_outbox_path?: string;
  errors: string[];
}

export type FlakyJobStatus = "open" | "quarantined" | "resolved";

export interface FlakyJobEntry {
  id: string;
  repo: string;
  workflow: string;
  provider: string;
  job_name: string;
  status: FlakyJobStatus;
  observed_count: number;
  first_seen_at: string;
  last_seen_at: string;
  owner?: string;
  notes?: string;
  updated_at: string;
}

export interface FlakyJobUpdateInput {
  repo: string;
  workflow: string;
  job_name: string;
  status: FlakyJobStatus;
  owner?: string;
  notes?: string;
}

export interface FlakyManagementSummary {
  updated_at: string;
  total: number;
  open: number;
  quarantined: number;
  resolved: number;
  jobs: FlakyJobEntry[];
}

export type UserRole = "admin" | "member" | "viewer";

export interface TeamMember {
  user_id: string;
  email: string;
  name?: string;
  role: UserRole;
  joined_at: string;
}

export interface Team {
  id: string;
  name: string;
  description?: string;
  created_at: string;
  updated_at: string;
  members: TeamMember[];
  settings: {
    pipeline_paths?: string[];
    default_runs_per_month?: number;
    default_developer_rate?: number;
    alert_channels?: {
      slack_webhook?: string;
      teams_webhook?: string;
      email_recipients?: string[];
    };
  };
}

export interface TeamCreateInput {
  name: string;
  description?: string;
  settings?: Team["settings"];
}

export interface TeamUpdateInput {
  name?: string;
  description?: string;
  settings?: Team["settings"];
}

export interface AddTeamMemberInput {
  user_id: string;
  email: string;
  name?: string;
  role: UserRole;
}

export interface OrgLevelMetrics {
  total_teams: number;
  total_pipelines: number;
  total_findings: number;
  avg_health_score: number;
  total_monthly_cost: number;
  total_time_saved_per_month: number;
  teams_summary: {
    team_id: string;
    team_name: string;
    pipeline_count: number;
    avg_duration_secs: number;
    total_findings: number;
    health_score: number;
    monthly_cost: number;
  }[];
}

const PIPELINE_EXTENSIONS = [".yml", ".yaml", ".groovy", ".jenkinsfile"];
const SEARCH_ROOTS = [".github/workflows", "tests/fixtures"];
const HISTORY_CACHE_RELATIVE_DIR = ".pipelinex/history-cache";
const BENCHMARK_REGISTRY_RELATIVE_PATH = ".pipelinex/benchmark-registry.json";
const IMPACT_REGISTRY_RELATIVE_PATH = ".pipelinex/optimization-impact-registry.json";
const ALERT_RULES_RELATIVE_PATH = ".pipelinex/alert-rules.json";
const DIGEST_EMAIL_OUTBOX_RELATIVE_PATH = ".pipelinex/digest-email-outbox.jsonl";
const FLAKY_MANAGEMENT_RELATIVE_PATH = ".pipelinex/flaky-management.json";
const TEAMS_REGISTRY_RELATIVE_PATH = ".pipelinex/teams-registry.json";

function pathExists(filePath: string): Promise<boolean> {
  return access(filePath, constants.F_OK)
    .then(() => true)
    .catch(() => false);
}

export async function getRepoRoot(): Promise<string> {
  const configuredRoot = process.env.PIPELINEX_REPO_ROOT?.trim();
  if (configuredRoot) {
    const resolved = path.resolve(configuredRoot);
    const exists = await pathExists(resolved);
    if (exists) {
      return resolved;
    }
    throw new Error(`PIPELINEX_REPO_ROOT does not exist: ${resolved}`);
  }

  const cwd = process.cwd();
  const cwdHasCargo = await pathExists(path.join(cwd, "Cargo.toml"));
  if (cwdHasCargo) {
    return cwd;
  }

  const parent = path.resolve(cwd, "..");
  const parentHasCargo = await pathExists(path.join(parent, "Cargo.toml"));
  if (parentHasCargo) {
    return parent;
  }

  throw new Error(
    `Unable to locate repository root from "${cwd}" (Cargo.toml not found in cwd or parent).`,
  );
}

export function isSupportedPipelineFile(filePath: string): boolean {
  const normalized = filePath.toLowerCase();
  const baseName = path.basename(normalized);

  if (baseName === "jenkinsfile" || baseName === "bitbucket-pipelines.yml") {
    return true;
  }

  return PIPELINE_EXTENSIONS.some((ext) => normalized.endsWith(ext));
}

async function walkPipelineFiles(dirPath: string, files: string[]): Promise<void> {
  const entries = await readdir(dirPath, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = path.join(dirPath, entry.name);

    if (entry.isDirectory()) {
      await walkPipelineFiles(absolutePath, files);
      continue;
    }

    if (entry.isFile() && isSupportedPipelineFile(absolutePath)) {
      files.push(absolutePath);
    }
  }
}

export async function listPipelineFiles(): Promise<string[]> {
  const repoRoot = await getRepoRoot();
  const discovered: string[] = [];

  for (const relativeRoot of SEARCH_ROOTS) {
    const absoluteRoot = path.join(repoRoot, relativeRoot);
    if (!(await pathExists(absoluteRoot))) {
      continue;
    }

    await walkPipelineFiles(absoluteRoot, discovered);
  }

  return discovered
    .map((absolutePath) => path.relative(repoRoot, absolutePath))
    .sort((a, b) => a.localeCompare(b))
    .map((relativePath) => relativePath.split(path.sep).join("/"));
}

export async function resolveRepoPath(inputPath: string): Promise<string> {
  if (!inputPath || inputPath.trim().length === 0) {
    throw new Error("pipelinePath is required.");
  }

  if (inputPath.includes("\0")) {
    throw new Error("Invalid path.");
  }

  const repoRoot = await getRepoRoot();
  const absoluteCandidate = path.isAbsolute(inputPath)
    ? path.resolve(inputPath)
    : path.resolve(repoRoot, inputPath);
  const relative = path.relative(repoRoot, absoluteCandidate);
  const outsideRepo = relative.startsWith("..") || path.isAbsolute(relative);

  if (outsideRepo) {
    throw new Error("Path must be inside repository root.");
  }

  const pipelineStats = await stat(absoluteCandidate).catch(() => null);
  if (!pipelineStats || !pipelineStats.isFile()) {
    throw new Error(`Pipeline file not found: ${inputPath}`);
  }

  if (!isSupportedPipelineFile(absoluteCandidate)) {
    throw new Error(`Unsupported pipeline format: ${inputPath}`);
  }

  return absoluteCandidate;
}

export async function findPipelinexCommand(repoRoot: string): Promise<string[]> {
  const localBinaries = [
    path.join(repoRoot, "target", "debug", "pipelinex"),
    path.join(repoRoot, "target", "release", "pipelinex"),
  ];

  for (const binPath of localBinaries) {
    try {
      await access(binPath, constants.X_OK);
      return [binPath];
    } catch {
      // Keep checking fallbacks.
    }
  }

  return ["cargo", "run", "--quiet", "-p", "pipelinex-cli", "--"];
}

function runCommand(
  cmdWithArgs: string[],
  repoRoot: string,
  timeoutMs = 60_000,
): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const [command, ...args] = cmdWithArgs;
    const child = spawn(command, args, { cwd: repoRoot, env: process.env });
    let stdout = "";
    let stderr = "";
    let timedOut = false;

    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, timeoutMs);

    child.stdout.on("data", (chunk: Buffer | string) => {
      stdout += chunk.toString();
    });

    child.stderr.on("data", (chunk: Buffer | string) => {
      stderr += chunk.toString();
    });

    child.on("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });

    child.on("close", (code) => {
      clearTimeout(timer);

      if (timedOut) {
        reject(new Error(`Pipeline analysis timed out after ${timeoutMs}ms.`));
        return;
      }

      if (code !== 0) {
        reject(
          new Error(
            `Analyzer command failed with exit code ${code}.\n${stderr || stdout}`.trim(),
          ),
        );
        return;
      }

      resolve({ stdout, stderr });
    });
  });
}

async function runPipelinexJsonCommand(
  commandSuffix: string[],
  timeoutMs = 120_000,
): Promise<string> {
  const repoRoot = await getRepoRoot();
  const commandPrefix = await findPipelinexCommand(repoRoot);
  const command = [...commandPrefix, ...commandSuffix];
  const { stdout } = await runCommand(command, repoRoot, timeoutMs);
  return stdout;
}

async function runPipelinexCommand(
  commandSuffix: string[],
  timeoutMs = 120_000,
): Promise<string> {
  const repoRoot = await getRepoRoot();
  const commandPrefix = await findPipelinexCommand(repoRoot);
  const command = [...commandPrefix, ...commandSuffix];
  const { stdout } = await runCommand(command, repoRoot, timeoutMs);
  return stdout;
}

export async function analyzePipelineFile(inputPath: string): Promise<AnalysisReport> {
  const absolutePath = await resolveRepoPath(inputPath);
  const stdout = await runPipelinexJsonCommand([
    "analyze",
    absolutePath,
    "--format",
    "json",
  ]);

  try {
    return JSON.parse(stdout) as AnalysisReport;
  } catch (error) {
    const preview = stdout.slice(0, 4000);
    throw new Error(
      `Failed to parse analyzer JSON output: ${
        error instanceof Error ? error.message : "Unknown parse error"
      }\nOutput preview:\n${preview}`,
    );
  }
}

export async function optimizePipelineFile(inputPath: string): Promise<string> {
  const absolutePath = await resolveRepoPath(inputPath);
  const stdout = await runPipelinexCommand(["optimize", absolutePath]);
  const normalized = stdout.trim();
  if (!normalized) {
    throw new Error("Optimizer did not return any output.");
  }
  return normalized.endsWith("\n") ? normalized : `${normalized}\n`;
}

export async function analyzePipelineContent(
  sourcePath: string,
  content: string,
): Promise<AnalysisReport> {
  if (!sourcePath || sourcePath.trim().length === 0) {
    throw new Error("sourcePath is required.");
  }
  if (typeof content !== "string" || content.trim().length === 0) {
    throw new Error("content is required.");
  }

  const extension = path.extname(sourcePath) || ".yml";
  const tempDir = await mkdtemp(path.join(tmpdir(), "pipelinex-pr-analysis-"));
  const tempPath = path.join(tempDir, `workflow${extension}`);

  try {
    await writeFile(tempPath, content, "utf8");
    const stdout = await runPipelinexJsonCommand([
      "analyze",
      tempPath,
      "--format",
      "json",
    ]);

    try {
      return JSON.parse(stdout) as AnalysisReport;
    } catch (error) {
      const preview = stdout.slice(0, 4000);
      throw new Error(
        `Failed to parse analyzer JSON output: ${
          error instanceof Error ? error.message : "Unknown parse error"
        }\nOutput preview:\n${preview}`,
      );
    }
  } finally {
    await rm(tempDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

function validateRepoIdentifier(repo: string): void {
  if (!repo || repo.trim().length === 0) {
    throw new Error("repo is required in namespace/project format.");
  }

  const parts = repo
    .split("/")
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0);

  if (parts.length < 2) {
    throw new Error("repo must be in namespace/project format.");
  }
}

function validateGithubRepoFullName(repo: string): void {
  const parts = repo
    .split("/")
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0);

  if (parts.length !== 2) {
    throw new Error("GitHub history refresh requires repo in owner/repo format.");
  }
}

function normalizeWorkflow(workflow: string): string {
  if (!workflow || workflow.trim().length === 0) {
    throw new Error("workflow is required.");
  }
  return workflow.trim();
}

function historyCacheFileName(repo: string, workflow: string): string {
  const safeRepo = encodeURIComponent(repo);
  const safeWorkflow = encodeURIComponent(workflow);
  return `${safeRepo}__${safeWorkflow}.json`;
}

async function historyCacheDir(): Promise<string> {
  const repoRoot = await getRepoRoot();
  return path.join(repoRoot, HISTORY_CACHE_RELATIVE_DIR);
}

async function historyCacheFilePath(repo: string, workflow: string): Promise<string> {
  const cacheDir = await historyCacheDir();
  return path.join(cacheDir, historyCacheFileName(repo, workflow));
}

async function writeHistorySnapshot(snapshot: HistorySnapshot): Promise<void> {
  const cacheDir = await historyCacheDir();
  await mkdir(cacheDir, { recursive: true });
  const cachePath = await historyCacheFilePath(snapshot.repo, snapshot.workflow);
  await writeFile(cachePath, JSON.stringify(snapshot, null, 2), "utf8");
}

export async function readHistorySnapshot(
  repo: string,
  workflow: string,
): Promise<HistorySnapshot | null> {
  validateRepoIdentifier(repo);
  const normalizedWorkflow = normalizeWorkflow(workflow);
  const cachePath = await historyCacheFilePath(repo, normalizedWorkflow);
  const exists = await pathExists(cachePath);

  if (!exists) {
    return null;
  }

  const raw = await readFile(cachePath, "utf8");
  return JSON.parse(raw) as HistorySnapshot;
}

export async function listHistorySnapshots(): Promise<HistorySnapshot[]> {
  const cacheDir = await historyCacheDir();
  const exists = await pathExists(cacheDir);
  if (!exists) {
    return [];
  }

  const files = await readdir(cacheDir, { withFileTypes: true });
  const snapshots: HistorySnapshot[] = [];

  for (const entry of files) {
    if (!entry.isFile() || !entry.name.endsWith(".json")) {
      continue;
    }

    const filePath = path.join(cacheDir, entry.name);
    try {
      const raw = await readFile(filePath, "utf8");
      snapshots.push(JSON.parse(raw) as HistorySnapshot);
    } catch {
      // Ignore malformed cache entries.
    }
  }

  snapshots.sort((a, b) => {
    const left = new Date(a.refreshed_at).getTime();
    const right = new Date(b.refreshed_at).getTime();
    return right - left;
  });

  return snapshots;
}

interface RefreshHistoryOptions {
  repo: string;
  workflow: string;
  runs?: number;
  token?: string;
  source?: "manual" | "webhook";
  deliveryId?: string;
  workflowRunId?: number;
}

interface StoreHistorySnapshotOptions {
  repo: string;
  workflow: string;
  provider?: string;
  runs?: number;
  source?: "manual" | "webhook";
  stats: PipelineStatistics;
  deliveryId?: string;
  workflowRunId?: number;
}

export async function storeHistorySnapshot(
  options: StoreHistorySnapshotOptions,
): Promise<HistorySnapshot> {
  validateRepoIdentifier(options.repo);
  const workflow = normalizeWorkflow(options.workflow);
  const runs = options.runs && options.runs > 0 ? options.runs : 1;

  const snapshot: HistorySnapshot = {
    repo: options.repo,
    workflow,
    provider: options.provider?.trim() || undefined,
    runs,
    refreshed_at: new Date().toISOString(),
    source: options.source ?? "manual",
    stats: options.stats,
    delivery_id: options.deliveryId,
    workflow_run_id: options.workflowRunId,
  };

  await writeHistorySnapshot(snapshot);
  return snapshot;
}

export async function refreshHistorySnapshot(
  options: RefreshHistoryOptions,
): Promise<HistorySnapshot> {
  validateRepoIdentifier(options.repo);
  validateGithubRepoFullName(options.repo);
  const workflow = normalizeWorkflow(options.workflow);
  const runs = options.runs && options.runs > 0 ? options.runs : 100;

  const command = [
    "history",
    "--repo",
    options.repo,
    "--workflow",
    workflow,
    "--runs",
    String(runs),
    "--format",
    "json",
  ];

  if (options.token && options.token.trim().length > 0) {
    command.push("--token", options.token.trim());
  }

  const stdout = await runPipelinexJsonCommand(command, 180_000);
  let stats: PipelineStatistics;

  try {
    stats = JSON.parse(stdout) as PipelineStatistics;
  } catch (error) {
    const preview = stdout.slice(0, 4000);
    throw new Error(
      `Failed to parse history JSON output: ${
        error instanceof Error ? error.message : "Unknown parse error"
      }\nOutput preview:\n${preview}`,
    );
  }

  const snapshot: HistorySnapshot = {
    repo: options.repo,
    workflow,
    provider: "github-actions",
    runs,
    refreshed_at: new Date().toISOString(),
    source: options.source ?? "manual",
    stats,
    delivery_id: options.deliveryId,
    workflow_run_id: options.workflowRunId,
  };

  await writeHistorySnapshot(snapshot);
  return snapshot;
}

function countBucket(count: number, bounds: number[]): string {
  if (!Number.isFinite(count) || count <= 0) {
    return "0";
  }

  for (let index = 0; index < bounds.length; index += 1) {
    const upper = bounds[index];
    const lower = index === 0 ? 1 : bounds[index - 1] + 1;
    if (count <= upper) {
      return `${lower}-${upper}`;
    }
  }

  return `${bounds[bounds.length - 1] + 1}+`;
}

function percentile(values: number[], pct: number): number {
  if (values.length === 0) {
    return 0;
  }

  const sorted = [...values].sort((a, b) => a - b);
  const rank = (pct / 100) * (sorted.length - 1);
  const lower = Math.floor(rank);
  const upper = Math.ceil(rank);

  if (lower === upper) {
    return sorted[lower];
  }

  const weight = rank - lower;
  return sorted[lower] * (1 - weight) + sorted[upper] * weight;
}

function median(values: number[]): number {
  return percentile(values, 50);
}

function benchmarkRegistryPath(repoRoot: string): string {
  return path.join(repoRoot, BENCHMARK_REGISTRY_RELATIVE_PATH);
}

async function readBenchmarkRegistry(): Promise<BenchmarkEntry[]> {
  const repoRoot = await getRepoRoot();
  const filePath = benchmarkRegistryPath(repoRoot);

  if (!(await pathExists(filePath))) {
    return [];
  }

  const raw = await readFile(filePath, "utf8");
  if (!raw.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw) as BenchmarkEntry[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed;
  } catch {
    return [];
  }
}

async function writeBenchmarkRegistry(entries: BenchmarkEntry[]): Promise<void> {
  const repoRoot = await getRepoRoot();
  const filePath = benchmarkRegistryPath(repoRoot);
  const parentDir = path.dirname(filePath);
  await mkdir(parentDir, { recursive: true });

  // Keep the local registry bounded to avoid unbounded growth.
  const retained = entries.slice(-2000);
  await writeFile(filePath, JSON.stringify(retained, null, 2), "utf8");
}

function impactRegistryPath(repoRoot: string): string {
  return path.join(repoRoot, IMPACT_REGISTRY_RELATIVE_PATH);
}

async function readImpactRegistry(): Promise<OptimizationImpactEntry[]> {
  const repoRoot = await getRepoRoot();
  const filePath = impactRegistryPath(repoRoot);

  if (!(await pathExists(filePath))) {
    return [];
  }

  const raw = await readFile(filePath, "utf8");
  if (!raw.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw) as OptimizationImpactEntry[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed;
  } catch {
    return [];
  }
}

async function writeImpactRegistry(entries: OptimizationImpactEntry[]): Promise<void> {
  const repoRoot = await getRepoRoot();
  const filePath = impactRegistryPath(repoRoot);
  const parentDir = path.dirname(filePath);
  await mkdir(parentDir, { recursive: true });

  // Keep the local registry bounded to avoid unbounded growth.
  const retained = entries.slice(-5000);
  await writeFile(filePath, JSON.stringify(retained, null, 2), "utf8");
}

function buildBenchmarkEntry(report: AnalysisReport, source: string): BenchmarkEntry {
  const criticalCount = report.findings.filter(
    (finding) => finding.severity.toLowerCase() === "critical",
  ).length;
  const highCount = report.findings.filter(
    (finding) => finding.severity.toLowerCase() === "high",
  ).length;
  const mediumCount = report.findings.filter(
    (finding) => finding.severity.toLowerCase() === "medium",
  ).length;

  const improvementPct =
    ((report.total_estimated_duration_secs - report.optimized_duration_secs) /
      Math.max(report.total_estimated_duration_secs, 1)) *
    100;

  return {
    id: randomUUID(),
    schema_version: 1,
    submitted_at: new Date().toISOString(),
    source,
    provider: report.provider,
    job_bucket: countBucket(report.job_count, [5, 10, 20, 40]),
    step_bucket: countBucket(report.step_count, [20, 50, 100, 200]),
    job_count: report.job_count,
    step_count: report.step_count,
    max_parallelism: report.max_parallelism,
    finding_count: report.findings.length,
    critical_count: criticalCount,
    high_count: highCount,
    medium_count: mediumCount,
    total_duration_secs: report.total_estimated_duration_secs,
    optimized_duration_secs: report.optimized_duration_secs,
    improvement_pct: Math.max(0, improvementPct),
    health_score: report.health_score?.total_score ?? null,
  };
}

function summarizeBenchmarkStats(
  entries: BenchmarkEntry[],
  cohort: BenchmarkStats["cohort"],
  provider: string,
  jobBucket: string,
  stepBucket: string,
): BenchmarkStats {
  const durationValues = entries.map((entry) => entry.total_duration_secs);
  const optimizedValues = entries.map((entry) => entry.optimized_duration_secs);
  const improvementValues = entries.map((entry) => entry.improvement_pct);
  const findingValues = entries.map((entry) => entry.finding_count);
  const healthValues = entries
    .map((entry) => entry.health_score)
    .filter((value): value is number => value !== null);

  return {
    cohort,
    sample_count: entries.length,
    provider,
    job_bucket: jobBucket,
    step_bucket: stepBucket,
    duration_median_secs: median(durationValues),
    duration_p75_secs: percentile(durationValues, 75),
    optimized_median_secs: median(optimizedValues),
    improvement_median_pct: median(improvementValues),
    health_score_median: healthValues.length > 0 ? median(healthValues) : null,
    finding_median: median(findingValues),
  };
}

function cohortEntries(
  entries: BenchmarkEntry[],
  provider: string,
  jobBucket: string,
  stepBucket: string,
): { cohort: BenchmarkStats["cohort"]; entries: BenchmarkEntry[] } {
  const exact = entries.filter(
    (entry) =>
      entry.provider === provider &&
      entry.job_bucket === jobBucket &&
      entry.step_bucket === stepBucket,
  );

  if (exact.length >= 5) {
    return { cohort: "provider+job+step", entries: exact };
  }

  const providerOnly = entries.filter((entry) => entry.provider === provider);
  if (providerOnly.length >= 5) {
    return { cohort: "provider", entries: providerOnly };
  }

  return { cohort: "global", entries };
}

interface BenchmarkQuery {
  provider: string;
  jobCount: number;
  stepCount: number;
}

interface TrackOptimizationImpactInput {
  source?: string;
  provider?: string;
  beforeDurationSecs: number;
  afterDurationSecs: number;
  runsPerMonth: number;
}

interface OptimizationImpactQuery {
  source?: string;
  provider?: string;
}

export async function queryBenchmarkStats(
  query: BenchmarkQuery,
): Promise<BenchmarkStats | null> {
  const provider = query.provider.trim();
  if (!provider) {
    throw new Error("provider is required.");
  }

  const jobBucket = countBucket(query.jobCount, [5, 10, 20, 40]);
  const stepBucket = countBucket(query.stepCount, [20, 50, 100, 200]);
  const entries = await readBenchmarkRegistry();

  if (entries.length === 0) {
    return null;
  }

  const { cohort, entries: matching } = cohortEntries(
    entries,
    provider,
    jobBucket,
    stepBucket,
  );

  if (matching.length === 0) {
    return null;
  }

  return summarizeBenchmarkStats(matching, cohort, provider, jobBucket, stepBucket);
}

export async function submitBenchmarkReport(
  report: AnalysisReport,
  source = "dashboard",
): Promise<{ entry: BenchmarkEntry; stats: BenchmarkStats }> {
  const entry = buildBenchmarkEntry(report, source);
  const entries = await readBenchmarkRegistry();
  entries.push(entry);
  await writeBenchmarkRegistry(entries);

  const stats = await queryBenchmarkStats({
    provider: entry.provider,
    jobCount: entry.job_count,
    stepCount: entry.step_count,
  });

  if (!stats) {
    throw new Error("Failed to compute benchmark stats after submission.");
  }

  return { entry, stats };
}

function assertPositiveFinite(value: number, fieldName: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${fieldName} must be a positive number.`);
  }
  return value;
}

function buildOptimizationImpactEntry(
  input: TrackOptimizationImpactInput,
): OptimizationImpactEntry {
  const source = input.source?.trim() || "dashboard";
  const provider = input.provider?.trim() || "unknown";
  const beforeDurationSecs = assertPositiveFinite(
    input.beforeDurationSecs,
    "beforeDurationSecs",
  );
  const afterDurationSecs = assertPositiveFinite(input.afterDurationSecs, "afterDurationSecs");
  const runsPerMonth = Math.floor(
    assertPositiveFinite(input.runsPerMonth, "runsPerMonth"),
  );

  const savingsPerRunSecs = Math.max(0, beforeDurationSecs - afterDurationSecs);
  const savingsPct = (savingsPerRunSecs / Math.max(1, beforeDurationSecs)) * 100;
  const savingsPerMonthSecs = savingsPerRunSecs * runsPerMonth;
  const minutesSavedPerMonth = savingsPerMonthSecs / 60;

  return {
    id: randomUUID(),
    schema_version: 1,
    tracked_at: new Date().toISOString(),
    source,
    provider,
    baseline_duration_secs: beforeDurationSecs,
    optimized_duration_secs: afterDurationSecs,
    savings_per_run_secs: savingsPerRunSecs,
    savings_pct: Math.max(0, savingsPct),
    runs_per_month: runsPerMonth,
    minutes_saved_per_month: Math.max(0, minutesSavedPerMonth),
    hours_saved_per_month: Math.max(0, minutesSavedPerMonth / 60),
  };
}

export async function trackOptimizationImpact(
  input: TrackOptimizationImpactInput,
): Promise<{ entry: OptimizationImpactEntry; stats: OptimizationImpactStats }> {
  const entry = buildOptimizationImpactEntry(input);
  const entries = await readImpactRegistry();
  entries.push(entry);
  await writeImpactRegistry(entries);

  const stats = await queryOptimizationImpactStats({
    source: entry.source,
    provider: entry.provider,
  });

  if (!stats) {
    throw new Error("Failed to compute optimization impact stats after submission.");
  }

  return { entry, stats };
}

export async function trackOptimizationImpactFromReport(
  report: AnalysisReport,
  runsPerMonth: number,
  source = "dashboard",
): Promise<{ entry: OptimizationImpactEntry; stats: OptimizationImpactStats }> {
  return trackOptimizationImpact({
    source,
    provider: report.provider,
    beforeDurationSecs: report.total_estimated_duration_secs,
    afterDurationSecs: report.optimized_duration_secs,
    runsPerMonth,
  });
}

export async function queryOptimizationImpactStats(
  query: OptimizationImpactQuery = {},
): Promise<OptimizationImpactStats | null> {
  const sourceFilter = query.source?.trim();
  const providerFilter = query.provider?.trim();
  const entries = await readImpactRegistry();

  if (entries.length === 0) {
    return null;
  }

  const matching = entries.filter((entry) => {
    if (sourceFilter && entry.source !== sourceFilter) {
      return false;
    }
    if (providerFilter && entry.provider !== providerFilter) {
      return false;
    }
    return true;
  });

  if (matching.length === 0) {
    return null;
  }

  const minutesValues = matching.map((entry) => entry.minutes_saved_per_month);
  const totalMinutes = minutesValues.reduce((acc, value) => acc + value, 0);
  const averageMinutes = totalMinutes / Math.max(1, minutesValues.length);

  return {
    sample_count: matching.length,
    source: sourceFilter || "all",
    provider: providerFilter || "all",
    avg_minutes_saved_per_month: averageMinutes,
    median_minutes_saved_per_month: median(minutesValues),
    p75_minutes_saved_per_month: percentile(minutesValues, 75),
    total_minutes_saved_per_month: totalMinutes,
    total_hours_saved_per_month: totalMinutes / 60,
  };
}

function alertRulesPath(repoRoot: string): string {
  return path.join(repoRoot, ALERT_RULES_RELATIVE_PATH);
}

function normalizeOptionalText(value: string | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized && normalized.length > 0 ? normalized : undefined;
}

function isAlertMetric(value: unknown): value is AlertMetric {
  return (
    value === "avg_duration_sec" ||
    value === "failure_rate_pct" ||
    value === "monthly_opportunity_cost_usd"
  );
}

function isAlertOperator(value: unknown): value is AlertOperator {
  return value === "gt" || value === "gte" || value === "lt" || value === "lte";
}

function validateAlertRuleInput(input: AlertRuleInput): AlertRuleInput {
  if (!input.name || input.name.trim().length === 0) {
    throw new Error("Alert rule name is required.");
  }
  if (!isAlertMetric(input.metric)) {
    throw new Error("Alert rule metric is invalid.");
  }
  if (!isAlertOperator(input.operator)) {
    throw new Error("Alert rule operator is invalid.");
  }
  if (!Number.isFinite(input.threshold)) {
    throw new Error("Alert rule threshold must be a finite number.");
  }

  return {
    ...input,
    name: input.name.trim(),
    enabled: input.enabled ?? true,
    threshold: Number(input.threshold),
    repo: normalizeOptionalText(input.repo),
    workflow: normalizeOptionalText(input.workflow),
    provider: normalizeOptionalText(input.provider),
  };
}

async function readAlertRules(): Promise<AlertRule[]> {
  const repoRoot = await getRepoRoot();
  const filePath = alertRulesPath(repoRoot);

  if (!(await pathExists(filePath))) {
    return [];
  }

  const raw = await readFile(filePath, "utf8");
  if (!raw.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw) as AlertRule[];
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.filter(
      (rule) =>
        typeof rule?.id === "string" &&
        typeof rule?.name === "string" &&
        typeof rule?.enabled === "boolean" &&
        isAlertMetric(rule?.metric) &&
        isAlertOperator(rule?.operator) &&
        typeof rule?.threshold === "number",
    );
  } catch {
    return [];
  }
}

async function writeAlertRules(rules: AlertRule[]): Promise<void> {
  const repoRoot = await getRepoRoot();
  const filePath = alertRulesPath(repoRoot);
  const parentDir = path.dirname(filePath);
  await mkdir(parentDir, { recursive: true });
  await writeFile(filePath, JSON.stringify(rules, null, 2), "utf8");
}

export async function listAlertRules(): Promise<AlertRule[]> {
  const rules = await readAlertRules();
  return rules.sort((a, b) => b.updated_at.localeCompare(a.updated_at));
}

export async function upsertAlertRule(input: AlertRuleInput): Promise<AlertRule> {
  const normalized = validateAlertRuleInput(input);
  const rules = await readAlertRules();
  const now = new Date().toISOString();

  if (normalized.id) {
    const index = rules.findIndex((rule) => rule.id === normalized.id);
    if (index >= 0) {
      const existing = rules[index];
      const updated: AlertRule = {
        ...existing,
        ...normalized,
        id: existing.id,
        created_at: existing.created_at,
        updated_at: now,
      };
      rules[index] = updated;
      await writeAlertRules(rules);
      return updated;
    }
  }

  const created: AlertRule = {
    id: randomUUID(),
    name: normalized.name,
    enabled: normalized.enabled ?? true,
    metric: normalized.metric,
    operator: normalized.operator,
    threshold: normalized.threshold,
    repo: normalized.repo,
    workflow: normalized.workflow,
    provider: normalized.provider,
    created_at: now,
    updated_at: now,
  };
  rules.push(created);
  await writeAlertRules(rules);
  return created;
}

export async function deleteAlertRule(id: string): Promise<boolean> {
  const normalized = id.trim();
  if (!normalized) {
    throw new Error("Alert rule id is required.");
  }

  const rules = await readAlertRules();
  const retained = rules.filter((rule) => rule.id !== normalized);
  if (retained.length === rules.length) {
    return false;
  }

  await writeAlertRules(retained);
  return true;
}

function compareNumeric(operator: AlertOperator, left: number, right: number): boolean {
  switch (operator) {
    case "gt":
      return left > right;
    case "gte":
      return left >= right;
    case "lt":
      return left < right;
    case "lte":
      return left <= right;
    default:
      return false;
  }
}

function inferProviderFromSnapshot(snapshot: HistorySnapshot): string {
  if (snapshot.provider && snapshot.provider.trim().length > 0) {
    return snapshot.provider.trim();
  }
  const workflowLower = snapshot.workflow.toLowerCase();
  if (workflowLower.includes(".gitlab-ci")) {
    return "gitlab-ci";
  }
  return "github-actions";
}

function resolveAlertDefaults(
  runsPerMonth: number | undefined,
  developerHourlyRate: number | undefined,
): { runsPerMonth: number; developerHourlyRate: number } {
  const runsFromEnv = Number.parseInt(
    process.env.PIPELINEX_ALERT_RUNS_PER_MONTH?.trim() || "",
    10,
  );
  const rateFromEnv = Number.parseFloat(
    process.env.PIPELINEX_ALERT_DEVELOPER_HOURLY_RATE?.trim() || "",
  );

  return {
    runsPerMonth:
      typeof runsPerMonth === "number" && Number.isFinite(runsPerMonth) && runsPerMonth > 0
        ? Math.floor(runsPerMonth)
        : Number.isFinite(runsFromEnv) && runsFromEnv > 0
          ? runsFromEnv
          : 500,
    developerHourlyRate:
      typeof developerHourlyRate === "number" &&
      Number.isFinite(developerHourlyRate) &&
      developerHourlyRate > 0
        ? developerHourlyRate
        : Number.isFinite(rateFromEnv) && rateFromEnv > 0
          ? rateFromEnv
          : 150,
  };
}

function metricValueForSnapshot(
  rule: AlertRule,
  snapshot: HistorySnapshot,
  defaults: { runsPerMonth: number; developerHourlyRate: number },
): number {
  switch (rule.metric) {
    case "avg_duration_sec":
      return snapshot.stats.avg_duration_sec;
    case "failure_rate_pct":
      return Math.max(0, (1 - snapshot.stats.success_rate) * 100);
    case "monthly_opportunity_cost_usd": {
      const monthlyHoursLost =
        (snapshot.stats.avg_duration_sec * defaults.runsPerMonth) / 3600;
      return monthlyHoursLost * defaults.developerHourlyRate;
    }
    default:
      return 0;
  }
}

function triggerSeverity(
  operator: AlertOperator,
  actual: number,
  threshold: number,
): "medium" | "high" | "critical" {
  const safeThreshold = Math.max(Math.abs(threshold), 1e-6);
  const ratio =
    operator === "lt" || operator === "lte"
      ? safeThreshold / Math.max(Math.abs(actual), 1e-6)
      : Math.abs(actual) / safeThreshold;
  if (ratio >= 1.5) {
    return "critical";
  }
  if (ratio >= 1.2) {
    return "high";
  }
  return "medium";
}

function snapshotMatchesRule(rule: AlertRule, snapshot: HistorySnapshot): boolean {
  const provider = inferProviderFromSnapshot(snapshot);
  if (rule.repo && rule.repo !== snapshot.repo) {
    return false;
  }
  if (rule.workflow && rule.workflow !== snapshot.workflow) {
    return false;
  }
  if (rule.provider && rule.provider !== provider) {
    return false;
  }
  return true;
}

interface EvaluateAlertRulesOptions {
  runsPerMonth?: number;
  developerHourlyRate?: number;
}

export async function evaluateAlertRules(
  options: EvaluateAlertRulesOptions = {},
): Promise<AlertEvaluationSummary> {
  const defaults = resolveAlertDefaults(options.runsPerMonth, options.developerHourlyRate);
  const rules = await listAlertRules();
  const snapshots = await listHistorySnapshots();
  const enabledRules = rules.filter((rule) => rule.enabled);
  const triggers: AlertTrigger[] = [];

  for (const rule of enabledRules) {
    const snapshot = snapshots.find((candidate) => snapshotMatchesRule(rule, candidate));
    if (!snapshot) {
      continue;
    }

    const actualValue = metricValueForSnapshot(rule, snapshot, defaults);
    const matched = compareNumeric(rule.operator, actualValue, rule.threshold);
    if (!matched) {
      continue;
    }

    const provider = inferProviderFromSnapshot(snapshot);
    const evaluatedAt = new Date().toISOString();
    triggers.push({
      rule_id: rule.id,
      rule_name: rule.name,
      metric: rule.metric,
      operator: rule.operator,
      threshold: rule.threshold,
      actual_value: actualValue,
      repo: snapshot.repo,
      workflow: snapshot.workflow,
      provider,
      severity: triggerSeverity(rule.operator, actualValue, rule.threshold),
      message: `${rule.name} triggered for ${snapshot.repo} (${rule.metric} ${rule.operator} ${rule.threshold}, actual ${actualValue.toFixed(2)})`,
      evaluated_at: evaluatedAt,
    });
  }

  triggers.sort((a, b) => {
    const severityRank = (value: AlertTrigger["severity"]): number =>
      value === "critical" ? 3 : value === "high" ? 2 : 1;
    return severityRank(b.severity) - severityRank(a.severity);
  });

  return {
    evaluated_at: new Date().toISOString(),
    default_runs_per_month: defaults.runsPerMonth,
    default_developer_hourly_rate: defaults.developerHourlyRate,
    total_rules: rules.length,
    enabled_rules: enabledRules.length,
    snapshots_considered: snapshots.length,
    triggered_count: triggers.length,
    triggers,
  };
}

interface FlakyJobOverride {
  key: string;
  status: FlakyJobStatus;
  owner?: string;
  notes?: string;
  updated_at: string;
}

function flakyManagementPath(repoRoot: string): string {
  return path.join(repoRoot, FLAKY_MANAGEMENT_RELATIVE_PATH);
}

function flakyJobKey(repo: string, workflow: string, jobName: string): string {
  return `${repo}::${workflow}::${jobName}`;
}

function normalizeFlakyStatus(value: unknown): FlakyJobStatus {
  if (value === "open" || value === "quarantined" || value === "resolved") {
    return value;
  }
  throw new Error("status must be one of: open, quarantined, resolved.");
}

async function readFlakyManagementOverrides(): Promise<FlakyJobOverride[]> {
  const repoRoot = await getRepoRoot();
  const filePath = flakyManagementPath(repoRoot);
  if (!(await pathExists(filePath))) {
    return [];
  }

  const raw = await readFile(filePath, "utf8");
  if (!raw.trim()) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw) as FlakyJobOverride[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(
      (entry) =>
        typeof entry?.key === "string" &&
        (entry?.status === "open" ||
          entry?.status === "quarantined" ||
          entry?.status === "resolved") &&
        typeof entry?.updated_at === "string",
    );
  } catch {
    return [];
  }
}

async function writeFlakyManagementOverrides(overrides: FlakyJobOverride[]): Promise<void> {
  const repoRoot = await getRepoRoot();
  const filePath = flakyManagementPath(repoRoot);
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, JSON.stringify(overrides, null, 2), "utf8");
}

interface AggregatedFlakyJob {
  repo: string;
  workflow: string;
  provider: string;
  job_name: string;
  observed_count: number;
  first_seen_at: string;
  last_seen_at: string;
}

export async function listFlakyJobs(): Promise<FlakyManagementSummary> {
  const snapshots = await listHistorySnapshots();
  const overrides = await readFlakyManagementOverrides();
  const overrideMap = new Map<string, FlakyJobOverride>();
  for (const override of overrides) {
    overrideMap.set(override.key, override);
  }

  const aggregate = new Map<string, AggregatedFlakyJob>();
  for (const snapshot of snapshots) {
    const provider = inferProviderFromSnapshot(snapshot);
    for (const rawJobName of snapshot.stats.flaky_jobs) {
      const jobName = rawJobName.trim();
      if (!jobName) {
        continue;
      }

      const key = flakyJobKey(snapshot.repo, snapshot.workflow, jobName);
      const existing = aggregate.get(key);
      if (!existing) {
        aggregate.set(key, {
          repo: snapshot.repo,
          workflow: snapshot.workflow,
          provider,
          job_name: jobName,
          observed_count: 1,
          first_seen_at: snapshot.refreshed_at,
          last_seen_at: snapshot.refreshed_at,
        });
        continue;
      }

      existing.observed_count += 1;
      if (snapshot.refreshed_at < existing.first_seen_at) {
        existing.first_seen_at = snapshot.refreshed_at;
      }
      if (snapshot.refreshed_at > existing.last_seen_at) {
        existing.last_seen_at = snapshot.refreshed_at;
      }
    }
  }

  const now = new Date().toISOString();
  const jobs: FlakyJobEntry[] = Array.from(aggregate.entries()).map(([key, entry]) => {
    const override = overrideMap.get(key);
    return {
      id: key,
      repo: entry.repo,
      workflow: entry.workflow,
      provider: entry.provider,
      job_name: entry.job_name,
      status: override?.status ?? "open",
      observed_count: entry.observed_count,
      first_seen_at: entry.first_seen_at,
      last_seen_at: entry.last_seen_at,
      owner: override?.owner,
      notes: override?.notes,
      updated_at: override?.updated_at ?? now,
    };
  });

  for (const override of overrides) {
    if (aggregate.has(override.key)) {
      continue;
    }

    const [repo = "unknown", workflow = "unknown", jobName = "unknown"] =
      override.key.split("::");
    jobs.push({
      id: override.key,
      repo,
      workflow,
      provider: "unknown",
      job_name: jobName,
      status: override.status,
      observed_count: 0,
      first_seen_at: override.updated_at,
      last_seen_at: override.updated_at,
      owner: override.owner,
      notes: override.notes,
      updated_at: override.updated_at,
    });
  }

  jobs.sort((left, right) => {
    if (right.observed_count !== left.observed_count) {
      return right.observed_count - left.observed_count;
    }
    return right.last_seen_at.localeCompare(left.last_seen_at);
  });

  return {
    updated_at: now,
    total: jobs.length,
    open: jobs.filter((job) => job.status === "open").length,
    quarantined: jobs.filter((job) => job.status === "quarantined").length,
    resolved: jobs.filter((job) => job.status === "resolved").length,
    jobs,
  };
}

export async function upsertFlakyJobStatus(
  input: FlakyJobUpdateInput,
): Promise<FlakyManagementSummary> {
  const repo = input.repo?.trim();
  const workflow = input.workflow?.trim();
  const jobName = input.job_name?.trim();
  if (!repo || !workflow || !jobName) {
    throw new Error("repo, workflow, and job_name are required.");
  }

  const status = normalizeFlakyStatus(input.status);
  const key = flakyJobKey(repo, workflow, jobName);
  const now = new Date().toISOString();
  const overrides = await readFlakyManagementOverrides();
  const index = overrides.findIndex((entry) => entry.key === key);
  const override: FlakyJobOverride = {
    key,
    status,
    owner: normalizeOptionalText(input.owner),
    notes: normalizeOptionalText(input.notes),
    updated_at: now,
  };

  if (index >= 0) {
    overrides[index] = override;
  } else {
    overrides.push(override);
  }

  await writeFlakyManagementOverrides(overrides);
  return listFlakyJobs();
}

interface WeeklyDigestOptions {
  windowDays?: number;
  runsPerMonth?: number;
  developerHourlyRate?: number;
}

function resolveWeeklyDigestDefaults(
  options: WeeklyDigestOptions = {},
): { windowDays: number; runsPerMonth: number; developerHourlyRate: number } {
  const windowDaysFromEnv = Number.parseInt(
    process.env.PIPELINEX_DIGEST_WINDOW_DAYS?.trim() || "",
    10,
  );
  const alertDefaults = resolveAlertDefaults(
    options.runsPerMonth,
    options.developerHourlyRate,
  );

  const normalizedWindow =
    typeof options.windowDays === "number" &&
    Number.isFinite(options.windowDays) &&
    options.windowDays > 0
      ? Math.floor(options.windowDays)
      : Number.isFinite(windowDaysFromEnv) && windowDaysFromEnv > 0
        ? windowDaysFromEnv
        : 7;

  return {
    windowDays: normalizedWindow,
    runsPerMonth: alertDefaults.runsPerMonth,
    developerHourlyRate: alertDefaults.developerHourlyRate,
  };
}

function csvValues(raw: string | undefined): string[] {
  if (!raw) {
    return [];
  }
  return raw
    .split(",")
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

function digestEmailOutboxPath(repoRoot: string): string {
  return path.join(repoRoot, DIGEST_EMAIL_OUTBOX_RELATIVE_PATH);
}

function roundTo(value: number, digits: number): number {
  const multiplier = 10 ** digits;
  return Math.round(value * multiplier) / multiplier;
}

function estimateMonthlyOpportunityCost(
  avgDurationSec: number,
  runsPerMonth: number,
  developerHourlyRate: number,
): number {
  const monthlyHoursLost = (avgDurationSec * runsPerMonth) / 3600;
  return monthlyHoursLost * developerHourlyRate;
}

function digestActionItems(summary: WeeklyDigestSummary): string[] {
  if (summary.snapshot_count === 0) {
    return [
      "No snapshots were captured in the selected window. Configure GitHub/GitLab webhooks or run history refresh jobs.",
    ];
  }

  const actions: string[] = [];
  if (summary.failure_rate_pct >= 10) {
    actions.push(
      "Failure rate is elevated. Prioritize flaky test triage and investigate repeated failing jobs.",
    );
  }
  if (summary.avg_duration_sec >= 20 * 60) {
    actions.push(
      "Average pipeline duration exceeds 20 minutes. Focus on critical path parallelization and cache coverage.",
    );
  }
  if (summary.top_flaky_jobs.length > 0) {
    actions.push(
      `Top flaky job this week: ${summary.top_flaky_jobs[0].job}. Consider quarantine/ownership assignment.`,
    );
  }
  if (summary.estimated_monthly_opportunity_cost_usd >= 1000) {
    actions.push(
      "Estimated opportunity cost is high. Schedule optimization sprint and alert thresholds for regressions.",
    );
  }

  if (actions.length === 0) {
    actions.push(
      "Pipeline health is stable this week. Continue monitoring trend and benchmark drift for regressions.",
    );
  }

  return actions.slice(0, 5);
}

function digestMarkdown(summary: WeeklyDigestSummary): string {
  const lines: string[] = [];
  lines.push("PipelineX Weekly Digest");
  lines.push(
    `Window: last ${summary.window_days} day(s) | Snapshots: ${summary.snapshot_count} | Total runs: ${summary.total_runs}`,
  );
  lines.push(
    `Avg duration: ${roundTo(summary.avg_duration_sec / 60, 2)} min | Failure rate: ${roundTo(summary.failure_rate_pct, 2)}%`,
  );
  lines.push(
    `Estimated monthly opportunity cost: $${roundTo(
      summary.estimated_monthly_opportunity_cost_usd,
      2,
    )}`,
  );

  if (summary.top_slowest_pipelines.length > 0) {
    lines.push("");
    lines.push("Top slow pipelines:");
    for (const pipeline of summary.top_slowest_pipelines.slice(0, 3)) {
      lines.push(
        `- ${pipeline.repo} :: ${pipeline.workflow} (${roundTo(
          pipeline.avg_duration_sec / 60,
          2,
        )} min avg, ${roundTo(pipeline.failure_rate_pct, 2)}% fail)`,
      );
    }
  }

  if (summary.top_flaky_jobs.length > 0) {
    lines.push("");
    lines.push("Top flaky jobs:");
    for (const flaky of summary.top_flaky_jobs.slice(0, 5)) {
      lines.push(`- ${flaky.job}: ${flaky.count} workflow(s) flagged`);
    }
  }

  if (summary.action_items.length > 0) {
    lines.push("");
    lines.push("Action items:");
    for (const action of summary.action_items) {
      lines.push(`- ${action}`);
    }
  }

  return lines.join("\n");
}

export async function generateWeeklyDigest(
  options: WeeklyDigestOptions = {},
): Promise<WeeklyDigestSummary> {
  const defaults = resolveWeeklyDigestDefaults(options);
  const snapshots = await listHistorySnapshots();
  const cutoffMs = Date.now() - defaults.windowDays * 24 * 60 * 60 * 1000;
  const scoped = snapshots.filter((snapshot) => {
    const refreshedMs = new Date(snapshot.refreshed_at).getTime();
    return Number.isFinite(refreshedMs) && refreshedMs >= cutoffMs;
  });

  const totalRuns = scoped.reduce(
    (acc, snapshot) => acc + Math.max(snapshot.stats.total_runs, 1),
    0,
  );
  const weightedDuration = scoped.reduce((acc, snapshot) => {
    const runs = Math.max(snapshot.stats.total_runs, 1);
    return acc + snapshot.stats.avg_duration_sec * runs;
  }, 0);
  const weightedSuccessRate = scoped.reduce((acc, snapshot) => {
    const runs = Math.max(snapshot.stats.total_runs, 1);
    return acc + snapshot.stats.success_rate * runs;
  }, 0);

  const avgDurationSec = totalRuns > 0 ? weightedDuration / totalRuns : 0;
  const successRate = totalRuns > 0 ? weightedSuccessRate / totalRuns : 0;
  const failureRatePct = Math.max(0, (1 - successRate) * 100);
  const monthlyOpportunityCost = estimateMonthlyOpportunityCost(
    avgDurationSec,
    defaults.runsPerMonth,
    defaults.developerHourlyRate,
  );

  const flakyCounts = new Map<string, number>();
  for (const snapshot of scoped) {
    for (const job of snapshot.stats.flaky_jobs) {
      const key = job.trim();
      if (!key) {
        continue;
      }
      flakyCounts.set(key, (flakyCounts.get(key) ?? 0) + 1);
    }
  }

  const topFlakyJobs = Array.from(flakyCounts.entries())
    .map(([job, count]) => ({ job, count }))
    .sort((left, right) => right.count - left.count || left.job.localeCompare(right.job))
    .slice(0, 8);

  const topSlowestPipelines: WeeklyDigestPipelineSummary[] = scoped
    .map((snapshot) => {
      const provider = inferProviderFromSnapshot(snapshot);
      const failureRate = Math.max(0, (1 - snapshot.stats.success_rate) * 100);
      return {
        repo: snapshot.repo,
        workflow: snapshot.workflow,
        provider,
        refreshed_at: snapshot.refreshed_at,
        runs: snapshot.stats.total_runs,
        avg_duration_sec: snapshot.stats.avg_duration_sec,
        failure_rate_pct: failureRate,
        estimated_monthly_opportunity_cost_usd: estimateMonthlyOpportunityCost(
          snapshot.stats.avg_duration_sec,
          defaults.runsPerMonth,
          defaults.developerHourlyRate,
        ),
        flaky_jobs: snapshot.stats.flaky_jobs,
      };
    })
    .sort((left, right) => right.avg_duration_sec - left.avg_duration_sec)
    .slice(0, 5);

  const summary: WeeklyDigestSummary = {
    generated_at: new Date().toISOString(),
    window_days: defaults.windowDays,
    snapshot_count: scoped.length,
    total_runs: totalRuns,
    avg_duration_sec: roundTo(avgDurationSec, 2),
    failure_rate_pct: roundTo(failureRatePct, 2),
    estimated_monthly_opportunity_cost_usd: roundTo(monthlyOpportunityCost, 2),
    top_flaky_jobs: topFlakyJobs,
    top_slowest_pipelines: topSlowestPipelines,
    action_items: [],
  };

  summary.action_items = digestActionItems(summary);
  return summary;
}

async function sendSlackDigest(webhookUrl: string, summary: WeeklyDigestSummary): Promise<void> {
  const response = await fetch(webhookUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      text: "```" + digestMarkdown(summary) + "```",
    }),
  });

  if (!response.ok) {
    throw new Error(`Slack webhook request failed with status ${response.status}.`);
  }
}

async function sendTeamsDigest(webhookUrl: string, summary: WeeklyDigestSummary): Promise<void> {
  const response = await fetch(webhookUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      "@type": "MessageCard",
      "@context": "https://schema.org/extensions",
      summary: "PipelineX Weekly Digest",
      themeColor: "00BCD4",
      title: "PipelineX Weekly Digest",
      text: digestMarkdown(summary).replace(/\n/g, "<br/>"),
    }),
  });

  if (!response.ok) {
    throw new Error(`Teams webhook request failed with status ${response.status}.`);
  }
}

async function queueDigestEmails(
  recipients: string[],
  summary: WeeklyDigestSummary,
  explicitOutboxPath?: string,
): Promise<{ queued: number; outboxPath: string }> {
  if (recipients.length === 0) {
    return { queued: 0, outboxPath: "" };
  }

  const repoRoot = await getRepoRoot();
  const outboxPath = explicitOutboxPath?.trim()
    ? path.resolve(repoRoot, explicitOutboxPath.trim())
    : digestEmailOutboxPath(repoRoot);

  await mkdir(path.dirname(outboxPath), { recursive: true });

  const now = new Date().toISOString();
  let queued = 0;
  for (const recipient of recipients) {
    const entry = {
      queued_at: now,
      to: recipient,
      subject: `PipelineX Weekly Digest - ${summary.generated_at.slice(0, 10)}`,
      body_text: digestMarkdown(summary),
      summary,
    };
    await appendFile(outboxPath, `${JSON.stringify(entry)}\n`, "utf8");
    queued += 1;
  }

  return { queued, outboxPath };
}

export async function deliverWeeklyDigest(
  summary: WeeklyDigestSummary,
  options: WeeklyDigestDeliveryOptions = {},
): Promise<WeeklyDigestDeliveryResult> {
  const dryRun = options.dryRun ?? false;
  const slackWebhookUrl =
    options.slackWebhookUrl?.trim() || process.env.PIPELINEX_DIGEST_SLACK_WEBHOOK_URL?.trim();
  const teamsWebhookUrl =
    options.teamsWebhookUrl?.trim() || process.env.PIPELINEX_DIGEST_TEAMS_WEBHOOK_URL?.trim();
  const emailRecipients =
    options.emailRecipients && options.emailRecipients.length > 0
      ? options.emailRecipients
      : csvValues(process.env.PIPELINEX_DIGEST_EMAIL_TO);

  const result: WeeklyDigestDeliveryResult = {
    dry_run: dryRun,
    slack_sent: false,
    teams_sent: false,
    email_queued: 0,
    errors: [],
  };

  if (!dryRun && slackWebhookUrl) {
    try {
      await sendSlackDigest(slackWebhookUrl, summary);
      result.slack_sent = true;
    } catch (error) {
      result.errors.push(
        error instanceof Error ? error.message : "Slack digest delivery failed.",
      );
    }
  }

  if (!dryRun && teamsWebhookUrl) {
    try {
      await sendTeamsDigest(teamsWebhookUrl, summary);
      result.teams_sent = true;
    } catch (error) {
      result.errors.push(
        error instanceof Error ? error.message : "Teams digest delivery failed.",
      );
    }
  }

  if (emailRecipients.length > 0) {
    try {
      if (dryRun) {
        result.email_queued = emailRecipients.length;
      } else {
        const queued = await queueDigestEmails(
          emailRecipients,
          summary,
          options.emailOutboxPath,
        );
        result.email_queued = queued.queued;
        result.email_outbox_path = queued.outboxPath;
      }
    } catch (error) {
      result.errors.push(
        error instanceof Error ? error.message : "Email digest queueing failed.",
      );
    }
  }

  return result;
}

// ============================================================================
// Team Management
// ============================================================================

/**
 * List all teams from the teams registry
 */
export async function listTeams(): Promise<Team[]> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  const exists = await pathExists(teamsPath);
  if (!exists) {
    return [];
  }

  const content = await readFile(teamsPath, "utf-8");
  const data = JSON.parse(content) as { teams: Team[] };
  return data.teams || [];
}

/**
 * Get a specific team by ID
 */
export async function getTeam(teamId: string): Promise<Team | null> {
  const teams = await listTeams();
  return teams.find((t) => t.id === teamId) || null;
}

/**
 * Create a new team
 */
export async function createTeam(input: TeamCreateInput): Promise<Team> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  // Load existing teams
  let teams: Team[] = [];
  const exists = await pathExists(teamsPath);
  if (exists) {
    const content = await readFile(teamsPath, "utf-8");
    const data = JSON.parse(content) as { teams: Team[] };
    teams = data.teams || [];
  }

  // Create new team
  const now = new Date().toISOString();
  const newTeam: Team = {
    id: `team-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`,
    name: input.name,
    description: input.description,
    created_at: now,
    updated_at: now,
    members: [],
    settings: input.settings || {},
  };

  teams.push(newTeam);

  // Ensure directory exists
  await mkdir(path.dirname(teamsPath), { recursive: true });

  // Save updated registry
  await writeFile(teamsPath, JSON.stringify({ teams }, null, 2), "utf-8");

  return newTeam;
}

/**
 * Update an existing team
 */
export async function updateTeam(
  teamId: string,
  input: TeamUpdateInput,
): Promise<Team> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  const teams = await listTeams();
  const teamIndex = teams.findIndex((t) => t.id === teamId);

  if (teamIndex === -1) {
    throw new Error(`Team not found: ${teamId}`);
  }

  // Update team
  const team = teams[teamIndex];
  if (input.name !== undefined) team.name = input.name;
  if (input.description !== undefined) team.description = input.description;
  if (input.settings !== undefined) {
    team.settings = { ...team.settings, ...input.settings };
  }
  team.updated_at = new Date().toISOString();

  teams[teamIndex] = team;

  // Save updated registry
  await writeFile(teamsPath, JSON.stringify({ teams }, null, 2), "utf-8");

  return team;
}

/**
 * Delete a team
 */
export async function deleteTeam(teamId: string): Promise<boolean> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  const teams = await listTeams();
  const filteredTeams = teams.filter((t) => t.id !== teamId);

  if (filteredTeams.length === teams.length) {
    return false; // Team not found
  }

  // Save updated registry
  await writeFile(
    teamsPath,
    JSON.stringify({ teams: filteredTeams }, null, 2),
    "utf-8",
  );

  return true;
}

/**
 * Add a member to a team
 */
export async function addTeamMember(
  teamId: string,
  input: AddTeamMemberInput,
): Promise<Team> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  const teams = await listTeams();
  const teamIndex = teams.findIndex((t) => t.id === teamId);

  if (teamIndex === -1) {
    throw new Error(`Team not found: ${teamId}`);
  }

  const team = teams[teamIndex];

  // Check if member already exists
  const existingMemberIndex = team.members.findIndex(
    (m) => m.user_id === input.user_id,
  );

  const newMember: TeamMember = {
    user_id: input.user_id,
    email: input.email,
    name: input.name,
    role: input.role,
    joined_at: new Date().toISOString(),
  };

  if (existingMemberIndex !== -1) {
    // Update existing member
    team.members[existingMemberIndex] = newMember;
  } else {
    // Add new member
    team.members.push(newMember);
  }

  team.updated_at = new Date().toISOString();
  teams[teamIndex] = team;

  // Save updated registry
  await writeFile(teamsPath, JSON.stringify({ teams }, null, 2), "utf-8");

  return team;
}

/**
 * Remove a member from a team
 */
export async function removeTeamMember(
  teamId: string,
  userId: string,
): Promise<Team> {
  const repoRoot = await getRepoRoot();
  const teamsPath = path.join(repoRoot, TEAMS_REGISTRY_RELATIVE_PATH);

  const teams = await listTeams();
  const teamIndex = teams.findIndex((t) => t.id === teamId);

  if (teamIndex === -1) {
    throw new Error(`Team not found: ${teamId}`);
  }

  const team = teams[teamIndex];
  team.members = team.members.filter((m) => m.user_id !== userId);
  team.updated_at = new Date().toISOString();
  teams[teamIndex] = team;

  // Save updated registry
  await writeFile(teamsPath, JSON.stringify({ teams }, null, 2), "utf-8");

  return team;
}

/**
 * Calculate organization-level metrics across all teams
 */
export async function calculateOrgLevelMetrics(): Promise<OrgLevelMetrics> {
  const teams = await listTeams();
  const historySnapshots = await listHistorySnapshots();

  const teamsSummary = [];
  let totalFindings = 0;
  let totalHealthScore = 0;
  let healthScoreCount = 0;
  const allPipelinePaths = new Set<string>();

  for (const team of teams) {
    const teamPipelines = team.settings.pipeline_paths || [];

    // Get metrics for this team's pipelines
    const teamMetrics = {
      team_id: team.id,
      team_name: team.name,
      pipeline_count: teamPipelines.length,
      avg_duration_secs: 0,
      total_findings: 0,
      health_score: 0,
      monthly_cost: 0,
    };

    // Calculate metrics from history snapshots for this team's pipelines
    const teamSnapshots = historySnapshots.filter((snapshot) =>
      teamPipelines.some((pipeline) =>
        snapshot.workflow_identifier.includes(pipeline),
      ),
    );

    if (teamSnapshots.length > 0) {
      const totalDuration = teamSnapshots.reduce(
        (sum, s) => sum + s.avg_duration_sec,
        0,
      );
      teamMetrics.avg_duration_secs = totalDuration / teamSnapshots.length;

      const runsPerMonth = team.settings.default_runs_per_month || 500;
      const developerRate = team.settings.default_developer_rate || 150;

      // Estimate monthly cost (simplified calculation)
      const computeCostPerRun = (teamMetrics.avg_duration_secs / 60) * 0.008; // $0.008 per minute
      teamMetrics.monthly_cost = computeCostPerRun * runsPerMonth;
    }

    // Add to totals
    teamPipelines.forEach((p) => allPipelinePaths.add(p));
    teamsSummary.push(teamMetrics);
  }

  // Calculate aggregates
  const totalMonthlyCost = teamsSummary.reduce((sum, t) => sum + t.monthly_cost, 0);
  const avgHealthScore = healthScoreCount > 0 ? totalHealthScore / healthScoreCount : 0;

  return {
    total_teams: teams.length,
    total_pipelines: allPipelinePaths.size,
    total_findings: totalFindings,
    avg_health_score: avgHealthScore,
    total_monthly_cost: totalMonthlyCost,
    total_time_saved_per_month: 0, // Would need optimization data
    teams_summary: teamsSummary,
  };
}
