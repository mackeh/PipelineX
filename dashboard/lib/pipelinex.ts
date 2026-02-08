import { constants } from "node:fs";
import { access, readdir, stat } from "node:fs/promises";
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

const PIPELINE_EXTENSIONS = [".yml", ".yaml", ".groovy", ".jenkinsfile"];
const SEARCH_ROOTS = [".github/workflows", "tests/fixtures"];

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

export async function analyzePipelineFile(inputPath: string): Promise<AnalysisReport> {
  const repoRoot = await getRepoRoot();
  const absolutePath = await resolveRepoPath(inputPath);
  const commandPrefix = await findPipelinexCommand(repoRoot);
  const fullCommand = [
    ...commandPrefix,
    "analyze",
    absolutePath,
    "--format",
    "json",
  ];
  const { stdout } = await runCommand(fullCommand, repoRoot);

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
