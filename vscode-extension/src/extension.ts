import * as path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import * as vscode from "vscode";

const execFileAsync = promisify(execFile);

type Severity = "critical" | "high" | "medium" | "low" | "info";

interface Finding {
  severity: string;
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

interface AnalysisReport {
  findings: Finding[];
}

interface Hint {
  line: number;
  finding: Finding;
  message: string;
  summary: string;
}

class HintLensProvider implements vscode.CodeLensProvider {
  private readonly onDidChangeEmitter = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this.onDidChangeEmitter.event;

  constructor(private readonly hintStore: Map<string, Hint[]>) {}

  refresh(): void {
    this.onDidChangeEmitter.fire();
  }

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const config = getConfig();
    if (!config.showCodeLens) {
      return [];
    }

    const hints = this.hintStore.get(document.uri.toString()) ?? [];
    return hints.map((hint) => {
      const line = Math.max(0, Math.min(hint.line, Math.max(0, document.lineCount - 1)));
      const range = new vscode.Range(line, 0, line, 0);
      return new vscode.CodeLens(range, {
        title: `PipelineX: ${hint.summary}`,
        command: "pipelinex.showHintDetails",
        arguments: [hint],
      });
    });
  }
}

interface ExtensionConfig {
  commandPath: string;
  autoAnalyzeOnSave: boolean;
  autoAnalyzeOnOpen: boolean;
  showCodeLens: boolean;
  maxHints: number;
  severityThreshold: Severity;
  commandTimeoutMs: number;
}

const severityRank: Record<Severity, number> = {
  info: 0,
  low: 1,
  medium: 2,
  high: 3,
  critical: 4,
};

const knownPipelineFileNames = new Set([
  ".gitlab-ci.yml",
  ".gitlab-ci.yaml",
  "jenkinsfile",
  "bitbucket-pipelines.yml",
  "azure-pipelines.yml",
]);

function getConfig(): ExtensionConfig {
  const config = vscode.workspace.getConfiguration("pipelinex");
  const maxHints = config.get<number>("maxHints", 25);
  const timeout = config.get<number>("commandTimeoutMs", 120000);
  return {
    commandPath: config.get<string>("commandPath", "pipelinex").trim() || "pipelinex",
    autoAnalyzeOnSave: config.get<boolean>("autoAnalyzeOnSave", true),
    autoAnalyzeOnOpen: config.get<boolean>("autoAnalyzeOnOpen", true),
    showCodeLens: config.get<boolean>("showCodeLens", true),
    maxHints: Math.max(1, Math.min(200, maxHints)),
    severityThreshold: parseSeverity(config.get<string>("severityThreshold", "medium")),
    commandTimeoutMs: Math.max(1000, Math.min(600000, timeout)),
  };
}

function parseSeverity(value: string): Severity {
  const normalized = value.toLowerCase();
  if (normalized === "critical") return "critical";
  if (normalized === "high") return "high";
  if (normalized === "medium") return "medium";
  if (normalized === "low") return "low";
  return "info";
}

function isPipelineDocument(document: vscode.TextDocument): boolean {
  if (document.uri.scheme !== "file") {
    return false;
  }

  const normalizedPath = document.uri.fsPath.replace(/\\/g, "/").toLowerCase();
  const baseName = path.basename(normalizedPath);

  if (normalizedPath.includes("/.github/workflows/") && (baseName.endsWith(".yml") || baseName.endsWith(".yaml"))) {
    return true;
  }

  if (normalizedPath.endsWith("/.circleci/config.yml") || normalizedPath.endsWith("/.buildkite/pipeline.yml")) {
    return true;
  }

  if (knownPipelineFileNames.has(baseName)) {
    return true;
  }

  return false;
}

function workspaceRootForDocument(document: vscode.TextDocument): string {
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
  if (workspaceFolder) {
    return workspaceFolder.uri.fsPath;
  }
  return path.dirname(document.uri.fsPath);
}

function parseReport(stdout: string): AnalysisReport {
  const trimmed = stdout.trim();

  try {
    return JSON.parse(trimmed) as AnalysisReport;
  } catch {
    const firstBrace = trimmed.indexOf("{");
    const lastBrace = trimmed.lastIndexOf("}");
    if (firstBrace >= 0 && lastBrace > firstBrace) {
      const maybeJson = trimmed.slice(firstBrace, lastBrace + 1);
      return JSON.parse(maybeJson) as AnalysisReport;
    }
    throw new Error("Failed to parse PipelineX JSON output.");
  }
}

function normalizeFindingSeverity(severity: string): Severity {
  return parseSeverity(severity);
}

function findJobLine(document: vscode.TextDocument, jobName: string): number | null {
  const escaped = jobName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`^\\s*${escaped}\\s*:\\s*(#.*)?$`);

  for (let index = 0; index < document.lineCount; index += 1) {
    if (regex.test(document.lineAt(index).text)) {
      return index;
    }
  }

  return null;
}

function hintSummaryFromFinding(finding: Finding): string {
  const savings =
    finding.estimated_savings_secs !== null && Number.isFinite(finding.estimated_savings_secs)
      ? ` (~${Math.round(finding.estimated_savings_secs)}s saved)`
      : "";
  const severity = normalizeFindingSeverity(finding.severity).toUpperCase();
  return `[${severity}] ${finding.title}${savings}`;
}

function diagnosticsSeverityFromFinding(finding: Finding): vscode.DiagnosticSeverity {
  const severity = normalizeFindingSeverity(finding.severity);
  if (severity === "critical") return vscode.DiagnosticSeverity.Error;
  if (severity === "high") return vscode.DiagnosticSeverity.Warning;
  if (severity === "medium") return vscode.DiagnosticSeverity.Warning;
  if (severity === "low") return vscode.DiagnosticSeverity.Information;
  return vscode.DiagnosticSeverity.Hint;
}

function chooseHintLine(document: vscode.TextDocument, finding: Finding): number {
  const jobs = Array.isArray(finding.affected_jobs) ? finding.affected_jobs : [];
  for (const job of jobs) {
    const line = findJobLine(document, job);
    if (line !== null) {
      return line;
    }
  }
  return 0;
}

function buildHintMessage(finding: Finding): string {
  const prefix = `[${normalizeFindingSeverity(finding.severity).toUpperCase()}]`;
  const recommendation = finding.recommendation ? ` ${finding.recommendation}` : "";
  const savings =
    finding.estimated_savings_secs !== null && Number.isFinite(finding.estimated_savings_secs)
      ? ` Estimated savings: ${Math.round(finding.estimated_savings_secs)}s.`
      : "";
  return `${prefix} ${finding.title}.${recommendation}${savings}`.trim();
}

async function runAnalyzeCommand(
  document: vscode.TextDocument,
  output: vscode.OutputChannel,
): Promise<AnalysisReport> {
  const config = getConfig();
  const cwd = workspaceRootForDocument(document);
  const args = ["analyze", document.uri.fsPath, "--format", "json"];

  output.appendLine(`$ ${config.commandPath} ${args.join(" ")}`);

  const result = await execFileAsync(config.commandPath, args, {
    cwd,
    timeout: config.commandTimeoutMs,
    maxBuffer: 16 * 1024 * 1024,
  });

  if (result.stderr && result.stderr.trim().length > 0) {
    output.appendLine(result.stderr.trim());
  }

  return parseReport(result.stdout);
}

export function activate(context: vscode.ExtensionContext): void {
  const diagnosticCollection = vscode.languages.createDiagnosticCollection("pipelinex");
  const output = vscode.window.createOutputChannel("PipelineX");
  const hintStore = new Map<string, Hint[]>();
  const inFlight = new Set<string>();
  const lensProvider = new HintLensProvider(hintStore);

  context.subscriptions.push(diagnosticCollection, output);

  const clearHintsForUri = (uri: vscode.Uri): void => {
    diagnosticCollection.delete(uri);
    hintStore.delete(uri.toString());
    lensProvider.refresh();
  };

  const analyzeDocument = async (
    document: vscode.TextDocument,
    trigger: "manual" | "save" | "open",
  ): Promise<void> => {
    if (!isPipelineDocument(document)) {
      return;
    }

    const key = document.uri.toString();
    if (inFlight.has(key)) {
      return;
    }

    inFlight.add(key);

    try {
      const report = await runAnalyzeCommand(document, output);
      const findings = Array.isArray(report.findings) ? report.findings : [];
      const config = getConfig();
      const thresholdRank = severityRank[config.severityThreshold];

      const filtered = findings
        .filter((finding) => {
          const sev = normalizeFindingSeverity(finding.severity);
          return severityRank[sev] >= thresholdRank;
        })
        .sort((left, right) => {
          const sevDiff = severityRank[normalizeFindingSeverity(right.severity)] - severityRank[normalizeFindingSeverity(left.severity)];
          if (sevDiff !== 0) {
            return sevDiff;
          }
          const rightSavings = right.estimated_savings_secs ?? 0;
          const leftSavings = left.estimated_savings_secs ?? 0;
          return rightSavings - leftSavings;
        })
        .slice(0, config.maxHints);

      const hints: Hint[] = filtered.map((finding) => {
        const line = chooseHintLine(document, finding);
        return {
          line,
          finding,
          message: buildHintMessage(finding),
          summary: hintSummaryFromFinding(finding),
        };
      });

      const diagnostics = hints.map((hint) => {
        const line = Math.max(0, Math.min(hint.line, Math.max(0, document.lineCount - 1)));
        const lineText = document.lineAt(line).text;
        const range = new vscode.Range(line, 0, line, Math.max(1, lineText.length));
        const diagnostic = new vscode.Diagnostic(
          range,
          hint.message,
          diagnosticsSeverityFromFinding(hint.finding),
        );
        diagnostic.source = "PipelineX";
        diagnostic.code = hint.finding.category;
        return diagnostic;
      });

      diagnosticCollection.set(document.uri, diagnostics);
      hintStore.set(key, hints);
      lensProvider.refresh();

      const status = `PipelineX: ${hints.length} hint${hints.length === 1 ? "" : "s"} (${trigger})`;
      void vscode.window.setStatusBarMessage(status, 2500);
    } catch (error) {
      clearHintsForUri(document.uri);

      const message =
        error instanceof Error
          ? error.message
          : "PipelineX analysis failed unexpectedly.";

      output.appendLine(`[error] ${message}`);

      if (trigger === "manual") {
        void vscode.window.showErrorMessage(
          `PipelineX analysis failed: ${message}`,
          "Show Output",
        ).then((selection) => {
          if (selection === "Show Output") {
            output.show(true);
          }
        });
      } else {
        void vscode.window.setStatusBarMessage("PipelineX: analysis failed (see output)", 3500);
      }
    } finally {
      inFlight.delete(key);
    }
  };

  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(
      [{ language: "yaml", scheme: "file" }, { language: "plaintext", scheme: "file" }],
      lensProvider,
    ),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("pipelinex.runAnalysis", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        void vscode.window.showInformationMessage("Open a pipeline workflow file first.");
        return;
      }

      if (!isPipelineDocument(editor.document)) {
        void vscode.window.showInformationMessage(
          "Current file is not a supported pipeline file.",
        );
        return;
      }

      await analyzeDocument(editor.document, "manual");
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("pipelinex.showHintDetails", async (hint: Hint) => {
      if (!hint || !hint.finding) {
        return;
      }

      const details: string[] = [
        `Title: ${hint.finding.title}`,
        `Severity: ${hint.finding.severity}`,
        `Category: ${hint.finding.category}`,
        `Recommendation: ${hint.finding.recommendation || "n/a"}`,
      ];

      if (hint.finding.fix_command) {
        details.push(`Fix Command: ${hint.finding.fix_command}`);
      }

      if (hint.finding.estimated_savings_secs !== null) {
        details.push(`Estimated Savings: ${Math.round(hint.finding.estimated_savings_secs)}s`);
      }

      const actions = ["Copy Recommendation"];
      if (hint.finding.fix_command) {
        actions.push("Copy Fix Command");
      }

      const choice = await vscode.window.showInformationMessage(
        details.join("\n"),
        ...actions,
      );

      if (choice === "Copy Recommendation") {
        await vscode.env.clipboard.writeText(hint.finding.recommendation || "");
      }

      if (choice === "Copy Fix Command" && hint.finding.fix_command) {
        await vscode.env.clipboard.writeText(hint.finding.fix_command);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("pipelinex.clearHints", () => {
      diagnosticCollection.clear();
      hintStore.clear();
      lensProvider.refresh();
      void vscode.window.setStatusBarMessage("PipelineX hints cleared", 2000);
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (!getConfig().autoAnalyzeOnSave) {
        return;
      }
      void analyzeDocument(document, "save");
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => {
      if (!getConfig().autoAnalyzeOnOpen) {
        return;
      }
      void analyzeDocument(document, "open");
    }),
  );

  if (getConfig().autoAnalyzeOnOpen && vscode.window.activeTextEditor) {
    void analyzeDocument(vscode.window.activeTextEditor.document, "open");
  }
}

export function deactivate(): void {
  // no-op
}
