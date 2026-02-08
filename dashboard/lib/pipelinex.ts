import { constants } from "node:fs";
import { access, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";

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
  runs: number;
  refreshed_at: string;
  source: "manual" | "webhook";
  stats: PipelineStatistics;
  delivery_id?: string;
  workflow_run_id?: number;
}

const PIPELINE_EXTENSIONS = [".yml", ".yaml", ".groovy", ".jenkinsfile"];
const SEARCH_ROOTS = [".github/workflows", "tests/fixtures"];
const HISTORY_CACHE_RELATIVE_DIR = ".pipelinex/history-cache";

function pathExists(filePath: string): Promise<boolean> {
  return access(filePath, constants.F_OK)
    .then(() => true)
    .catch(() => false);
}

export async function getRepoRoot(): Promise<string> {
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

async function findPipelinexCommand(repoRoot: string): Promise<string[]> {
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

function validateRepoFullName(repo: string): void {
  if (!repo || repo.trim().length === 0) {
    throw new Error("repo is required in owner/repo format.");
  }

  const parts = repo.split("/");
  if (parts.length !== 2 || !parts[0] || !parts[1]) {
    throw new Error("repo must be in owner/repo format.");
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
  validateRepoFullName(repo);
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

export async function refreshHistorySnapshot(
  options: RefreshHistoryOptions,
): Promise<HistorySnapshot> {
  validateRepoFullName(options.repo);
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
