"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Gauge,
  GitMerge,
  Play,
  Plus,
  RefreshCw,
  Trophy,
  Users,
  Workflow,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type {
  AlertEvaluationSummary,
  AlertRule,
  AnalysisReport,
  BenchmarkEntry,
  BenchmarkStats,
  FlakyJobEntry,
  FlakyManagementSummary,
  Finding,
  HistorySnapshot,
  OrgLevelMetrics,
  Team,
  WeeklyDigestDeliveryResult,
  WeeklyDigestSummary,
} from "@/lib/pipelinex";
import { DagExplorer } from "@/components/DagExplorer";

type WorkflowsResponse = {
  files?: string[];
  error?: string;
};

type AnalyzeResponse = {
  report?: AnalysisReport;
  error?: string;
};

type HistoryListResponse = {
  snapshots?: HistorySnapshot[];
  error?: string;
};

type BenchmarkSubmitResponse = {
  entry?: BenchmarkEntry;
  stats?: BenchmarkStats;
  error?: string;
};

type BenchmarkStatsResponse = {
  stats?: BenchmarkStats;
  error?: string;
};

type AlertsResponse = {
  rules?: AlertRule[];
  error?: string;
};

type AlertEvaluateResponse = {
  summary?: AlertEvaluationSummary;
  error?: string;
};

type WeeklyDigestResponse = {
  summary?: WeeklyDigestSummary;
  delivery?: WeeklyDigestDeliveryResult;
  error?: string;
};

type FlakyResponse = {
  summary?: FlakyManagementSummary;
  error?: string;
};

function formatDuration(seconds: number): string {
  const safeSeconds = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  const remainder = safeSeconds % 60;
  return `${minutes}m ${remainder}s`;
}

function percentage(value: number): string {
  return `${value.toFixed(1)}%`;
}

function formatUsd(value: number): string {
  return `$${value.toFixed(2)}`;
}

function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case "critical":
      return "bg-red-500/15 text-red-300 border-red-500/40";
    case "high":
      return "bg-orange-500/15 text-orange-300 border-orange-500/40";
    case "medium":
      return "bg-yellow-500/15 text-yellow-200 border-yellow-500/40";
    case "low":
      return "bg-sky-500/15 text-sky-200 border-sky-500/40";
    default:
      return "bg-slate-500/20 text-slate-200 border-slate-500/40";
  }
}

export default function DashboardPage() {
  const [workflows, setWorkflows] = useState<string[]>([]);
  const [selectedPath, setSelectedPath] = useState("");
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [loadingWorkflows, setLoadingWorkflows] = useState(true);
  const [runningAnalysis, setRunningAnalysis] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [historySnapshots, setHistorySnapshots] = useState<HistorySnapshot[]>([]);
  const [loadingAlerts, setLoadingAlerts] = useState(false);
  const [alertRules, setAlertRules] = useState<AlertRule[]>([]);
  const [alertSummary, setAlertSummary] = useState<AlertEvaluationSummary | null>(null);
  const [loadingFlaky, setLoadingFlaky] = useState(false);
  const [flakySummary, setFlakySummary] = useState<FlakyManagementSummary | null>(null);
  const [flakyError, setFlakyError] = useState<string | null>(null);
  const [flakyUpdatingId, setFlakyUpdatingId] = useState<string | null>(null);
  const [digestLoading, setDigestLoading] = useState(false);
  const [digestSummary, setDigestSummary] = useState<WeeklyDigestSummary | null>(null);
  const [digestDelivery, setDigestDelivery] = useState<WeeklyDigestDeliveryResult | null>(null);
  const [digestError, setDigestError] = useState<string | null>(null);
  const [benchmarkStats, setBenchmarkStats] = useState<BenchmarkStats | null>(null);
  const [benchmarkSubmitting, setBenchmarkSubmitting] = useState(false);
  const [benchmarkLoading, setBenchmarkLoading] = useState(false);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  const [applyingOptimization, setApplyingOptimization] = useState(false);
  const [applySuccess, setApplySuccess] = useState<{ prUrl?: string; prNumber?: number; branch?: string } | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [teams, setTeams] = useState<Team[]>([]);
  const [loadingTeams, setLoadingTeams] = useState(false);
  const [showCreateTeam, setShowCreateTeam] = useState(false);
  const [newTeamName, setNewTeamName] = useState("");
  const [orgMetrics, setOrgMetrics] = useState<OrgLevelMetrics | null>(null);
  const [loadingOrgMetrics, setLoadingOrgMetrics] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);

  const refreshBenchmarkStats = useCallback(
    async (nextReport: AnalysisReport) => {
      setBenchmarkLoading(true);
      setBenchmarkError(null);
      try {
        const params = new URLSearchParams({
          provider: nextReport.provider,
          jobCount: String(nextReport.job_count),
          stepCount: String(nextReport.step_count),
        });
        const response = await fetch(`/api/benchmarks/stats?${params.toString()}`);
        const payload = (await response.json()) as BenchmarkStatsResponse;

        if (!response.ok || !payload.stats) {
          throw new Error(payload.error || "No benchmark stats available.");
        }

        setBenchmarkStats(payload.stats);
      } catch (statsError) {
        setBenchmarkStats(null);
        setBenchmarkError(
          statsError instanceof Error
            ? statsError.message
            : "Failed to fetch benchmark stats.",
        );
      } finally {
        setBenchmarkLoading(false);
      }
    },
    [],
  );

  const submitBenchmark = useCallback(
    async (nextReport: AnalysisReport) => {
      setBenchmarkSubmitting(true);
      setBenchmarkError(null);
      try {
        const response = await fetch("/api/benchmarks/submit", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ report: nextReport, source: "dashboard-live" }),
        });
        const payload = (await response.json()) as BenchmarkSubmitResponse;

        if (!response.ok || !payload.stats) {
          throw new Error(payload.error || "Benchmark submission failed.");
        }

        setBenchmarkStats(payload.stats);
      } catch (submitError) {
        setBenchmarkError(
          submitError instanceof Error
            ? submitError.message
            : "Failed to submit benchmark entry.",
        );
        await refreshBenchmarkStats(nextReport);
      } finally {
        setBenchmarkSubmitting(false);
      }
    },
    [refreshBenchmarkStats],
  );

  const runAnalysis = useCallback(async (pipelinePath: string) => {
    if (!pipelinePath) {
      return;
    }

    setRunningAnalysis(true);
    setError(null);

    try {
      const response = await fetch("/api/analyze", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pipelinePath }),
      });
      const payload = (await response.json()) as AnalyzeResponse;

      if (!response.ok || !payload.report) {
        throw new Error(payload.error || "Analysis failed.");
      }

      setReport(payload.report);
      setLastUpdated(new Date().toLocaleString());
      await submitBenchmark(payload.report);
    } catch (analysisError) {
      setError(
        analysisError instanceof Error ? analysisError.message : "Analysis failed unexpectedly.",
      );
    } finally {
      setRunningAnalysis(false);
    }
  }, [submitBenchmark]);

  const applyOptimization = useCallback(async (pipelinePath: string) => {
    if (!pipelinePath) {
      return;
    }

    setApplyingOptimization(true);
    setApplyError(null);
    setApplySuccess(null);

    try {
      const response = await fetch("/api/apply", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          pipelinePath,
          baseBranch: "main",
          noPr: false,
        }),
      });

      type ApplyResponse = {
        success?: boolean;
        prUrl?: string;
        prNumber?: number;
        branch?: string;
        message?: string;
        error?: string;
      };

      const payload = (await response.json()) as ApplyResponse;

      if (!response.ok || !payload.success) {
        throw new Error(payload.error || "Failed to apply optimization.");
      }

      setApplySuccess({
        prUrl: payload.prUrl,
        prNumber: payload.prNumber,
        branch: payload.branch,
      });
    } catch (applyErr) {
      setApplyError(
        applyErr instanceof Error
          ? applyErr.message
          : "Failed to apply optimization unexpectedly.",
      );
    } finally {
      setApplyingOptimization(false);
    }
  }, []);

  const loadTeams = useCallback(async () => {
    setLoadingTeams(true);
    try {
      const response = await fetch("/api/teams");
      type TeamsResponse = { teams?: Team[]; error?: string };
      const payload = (await response.json()) as TeamsResponse;
      if (!response.ok || !payload.teams) {
        throw new Error(payload.error || "Failed to load teams.");
      }
      setTeams(payload.teams);
    } catch (teamsError) {
      console.error("Failed to load teams:", teamsError);
    } finally {
      setLoadingTeams(false);
    }
  }, []);

  const createTeam = useCallback(async () => {
    if (!newTeamName.trim()) {
      return;
    }

    try {
      const response = await fetch("/api/teams", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: newTeamName }),
      });

      type TeamResponse = { team?: Team; error?: string };
      const payload = (await response.json()) as TeamResponse;

      if (!response.ok || !payload.team) {
        throw new Error(payload.error || "Failed to create team.");
      }

      setNewTeamName("");
      setShowCreateTeam(false);
      await loadTeams();
    } catch (teamError) {
      setError(
        teamError instanceof Error ? teamError.message : "Failed to create team.",
      );
    }
  }, [newTeamName, loadTeams]);

  const loadOrgMetrics = useCallback(async () => {
    setLoadingOrgMetrics(true);
    try {
      const response = await fetch("/api/org/metrics");
      type OrgMetricsResponse = { metrics?: OrgLevelMetrics; error?: string };
      const payload = (await response.json()) as OrgMetricsResponse;
      if (!response.ok || !payload.metrics) {
        throw new Error(payload.error || "Failed to load org metrics.");
      }
      setOrgMetrics(payload.metrics);
    } catch (metricsError) {
      console.error("Failed to load org metrics:", metricsError);
    } finally {
      setLoadingOrgMetrics(false);
    }
  }, []);

  const loadHistorySnapshots = useCallback(async () => {
    setLoadingHistory(true);
    try {
      const response = await fetch("/api/history");
      const payload = (await response.json()) as HistoryListResponse;
      if (!response.ok || !payload.snapshots) {
        throw new Error(payload.error || "Failed to load history snapshots.");
      }
      setHistorySnapshots(payload.snapshots.slice(0, 20));
    } catch (historyError) {
      setError(
        historyError instanceof Error
          ? historyError.message
          : "Failed to load history snapshot list.",
      );
    } finally {
      setLoadingHistory(false);
    }
  }, []);

  const loadAlerts = useCallback(async () => {
    setLoadingAlerts(true);
    try {
      const [rulesResponse, summaryResponse] = await Promise.all([
        fetch("/api/alerts"),
        fetch("/api/alerts/evaluate"),
      ]);

      const rulesPayload = (await rulesResponse.json()) as AlertsResponse;
      if (!rulesResponse.ok || !rulesPayload.rules) {
        throw new Error(rulesPayload.error || "Failed to load alert rules.");
      }

      const summaryPayload = (await summaryResponse.json()) as AlertEvaluateResponse;
      if (!summaryResponse.ok || !summaryPayload.summary) {
        throw new Error(summaryPayload.error || "Failed to evaluate alert rules.");
      }

      setAlertRules(rulesPayload.rules);
      setAlertSummary(summaryPayload.summary);
    } catch (alertsError) {
      setError(
        alertsError instanceof Error
          ? alertsError.message
          : "Failed to load alerting state.",
      );
    } finally {
      setLoadingAlerts(false);
    }
  }, []);

  const loadFlaky = useCallback(async () => {
    setLoadingFlaky(true);
    setFlakyError(null);
    try {
      const response = await fetch("/api/flaky");
      const payload = (await response.json()) as FlakyResponse;
      if (!response.ok || !payload.summary) {
        throw new Error(payload.error || "Failed to load flaky job summary.");
      }
      setFlakySummary(payload.summary);
    } catch (flakyLoadError) {
      setFlakyError(
        flakyLoadError instanceof Error
          ? flakyLoadError.message
          : "Failed to load flaky job summary.",
      );
    } finally {
      setLoadingFlaky(false);
    }
  }, []);

  const updateFlakyStatus = useCallback(
    async (job: FlakyJobEntry, status: FlakyJobEntry["status"]) => {
      setFlakyUpdatingId(job.id);
      setFlakyError(null);
      try {
        const response = await fetch("/api/flaky", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            repo: job.repo,
            workflow: job.workflow,
            job_name: job.job_name,
            status,
          }),
        });
        const payload = (await response.json()) as FlakyResponse;
        if (!response.ok || !payload.summary) {
          throw new Error(payload.error || "Failed to update flaky job status.");
        }
        setFlakySummary(payload.summary);
      } catch (flakyUpdateError) {
        setFlakyError(
          flakyUpdateError instanceof Error
            ? flakyUpdateError.message
            : "Failed to update flaky job status.",
        );
      } finally {
        setFlakyUpdatingId(null);
      }
    },
    [],
  );

  const loadDigest = useCallback(async (deliver: boolean) => {
    setDigestLoading(true);
    setDigestError(null);
    try {
      const response = await fetch("/api/digest/weekly", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          deliver,
          channels: { dryRun: !deliver },
        }),
      });
      const payload = (await response.json()) as WeeklyDigestResponse;
      if (!response.ok || !payload.summary) {
        throw new Error(payload.error || "Failed to generate weekly digest.");
      }

      setDigestSummary(payload.summary);
      setDigestDelivery(payload.delivery ?? null);
    } catch (digestLoadError) {
      setDigestError(
        digestLoadError instanceof Error
          ? digestLoadError.message
          : "Failed to generate weekly digest.",
      );
    } finally {
      setDigestLoading(false);
    }
  }, []);

  useEffect(() => {
    let mounted = true;

    const loadWorkflows = async () => {
      setLoadingWorkflows(true);
      setError(null);
      try {
        const response = await fetch("/api/workflows");
        const payload = (await response.json()) as WorkflowsResponse;

        if (!response.ok || !payload.files) {
          throw new Error(payload.error || "Failed to load workflows.");
        }

        if (!mounted) {
          return;
        }

        setWorkflows(payload.files);
        const firstFile = payload.files[0] || "";
        setSelectedPath(firstFile);
        if (firstFile) {
          await runAnalysis(firstFile);
        }
      } catch (loadError) {
        if (!mounted) {
          return;
        }
        setError(
          loadError instanceof Error ? loadError.message : "Failed to load dashboard workflows.",
        );
      } finally {
        if (mounted) {
          setLoadingWorkflows(false);
        }
      }
    };

    void loadWorkflows();
    void loadHistorySnapshots();
    void loadAlerts();
    void loadFlaky();
    void loadDigest(false);
    void loadTeams();
    void loadOrgMetrics();

    return () => {
      mounted = false;
    };
  }, [loadAlerts, loadDigest, loadFlaky, loadHistorySnapshots, loadTeams, loadOrgMetrics, runAnalysis]);

  const severityCounts = useMemo(() => {
    const counts = {
      critical: 0,
      high: 0,
      medium: 0,
      low: 0,
      info: 0,
    };

    if (!report) {
      return counts;
    }

    for (const finding of report.findings) {
      const key = finding.severity.toLowerCase() as keyof typeof counts;
      if (key in counts) {
        counts[key] += 1;
      }
    }

    return counts;
  }, [report]);

  const durationData = useMemo(
    () =>
      report
        ? [
            { name: "Current", duration: Number(report.total_estimated_duration_secs.toFixed(1)) },
            { name: "Optimized", duration: Number(report.optimized_duration_secs.toFixed(1)) },
          ]
        : [],
    [report],
  );

  const severityData = useMemo(
    () => [
      { name: "Critical", count: severityCounts.critical },
      { name: "High", count: severityCounts.high },
      { name: "Medium", count: severityCounts.medium },
      { name: "Low", count: severityCounts.low },
      { name: "Info", count: severityCounts.info },
    ],
    [severityCounts],
  );

  const topFindings = useMemo(() => report?.findings.slice(0, 8) ?? [], [report]);
  const bottleneckCategoryRows = useMemo(() => {
    if (!report) {
      return [] as Array<{ label: string; count: number }>;
    }

    const buckets = new Map<string, number>();
    for (const finding of report.findings) {
      const key = finding.category || "Unknown";
      buckets.set(key, (buckets.get(key) ?? 0) + 1);
    }

    return Array.from(buckets.entries())
      .map(([label, count]) => ({ label, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 8);
  }, [report]);

  const bottleneckJobRows = useMemo(() => {
    if (!report) {
      return [] as Array<{ label: string; count: number }>;
    }

    const buckets = new Map<string, number>();
    for (const finding of report.findings) {
      for (const job of finding.affected_jobs) {
        const key = job || "(unknown)";
        buckets.set(key, (buckets.get(key) ?? 0) + 1);
      }
    }

    return Array.from(buckets.entries())
      .map(([label, count]) => ({ label, count }))
      .sort((a, b) => b.count - a.count)
      .slice(0, 8);
  }, [report]);

  const trendSeries = useMemo(() => {
    if (historySnapshots.length === 0) {
      return [] as Array<{
        label: string;
        durationSec: number;
        failureRatePct: number;
        costUsdPerRun: number;
      }>;
    }

    const developerHourlyRate = 150;
    return [...historySnapshots]
      .sort(
        (left, right) =>
          new Date(left.refreshed_at).getTime() - new Date(right.refreshed_at).getTime(),
      )
      .slice(-12)
      .map((snapshot) => {
        const refreshed = new Date(snapshot.refreshed_at);
        const label = `${refreshed.toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        })} ${refreshed.toLocaleTimeString(undefined, {
          hour: "2-digit",
          minute: "2-digit",
        })}`;
        const durationSec = snapshot.stats.avg_duration_sec;
        const failureRatePct = Math.max(0, (1 - snapshot.stats.success_rate) * 100);
        const costUsdPerRun = (durationSec / 3600) * developerHourlyRate;

        return {
          label,
          durationSec: Number(durationSec.toFixed(2)),
          failureRatePct: Number(failureRatePct.toFixed(2)),
          costUsdPerRun: Number(costUsdPerRun.toFixed(2)),
        };
      });
  }, [historySnapshots]);

  const costCenter = useMemo(() => {
    if (!report) {
      return null;
    }

    const runsPerMonth = alertSummary?.default_runs_per_month ?? 500;
    const developerHourlyRate = alertSummary?.default_developer_hourly_rate ?? 150;
    const categoryBuckets = new Map<string, { findings: number; savingsSec: number }>();
    let totalSavingsSec = 0;
    let criticalSavingsSec = 0;

    for (const finding of report.findings) {
      const category = finding.category || "Unknown";
      const savingsSec = Math.max(0, finding.estimated_savings_secs ?? 0);
      const current = categoryBuckets.get(category) ?? { findings: 0, savingsSec: 0 };
      current.findings += 1;
      current.savingsSec += savingsSec;
      categoryBuckets.set(category, current);
      totalSavingsSec += savingsSec;
      if (finding.severity.toLowerCase() === "critical") {
        criticalSavingsSec += savingsSec;
      }
    }

    const rows = Array.from(categoryBuckets.entries())
      .map(([category, value]) => {
        const monthlyWasteUsd =
          ((value.savingsSec * runsPerMonth) / 3600) * developerHourlyRate;
        return {
          category,
          findings: value.findings,
          savingsSec: Number(value.savingsSec.toFixed(2)),
          monthlyWasteUsd: Number(monthlyWasteUsd.toFixed(2)),
        };
      })
      .sort((left, right) => right.monthlyWasteUsd - left.monthlyWasteUsd);

    const monthlyWasteUsd = ((totalSavingsSec * runsPerMonth) / 3600) * developerHourlyRate;
    const criticalWasteUsd =
      ((criticalSavingsSec * runsPerMonth) / 3600) * developerHourlyRate;

    return {
      runsPerMonth,
      developerHourlyRate,
      monthlyWasteUsd: Number(monthlyWasteUsd.toFixed(2)),
      criticalWasteUsd: Number(criticalWasteUsd.toFixed(2)),
      categories: rows.slice(0, 8),
    };
  }, [alertSummary, report]);

  const savingsSeconds = report
    ? Math.max(0, report.total_estimated_duration_secs - report.optimized_duration_secs)
    : 0;
  const savingsPercent = report
    ? (savingsSeconds / Math.max(report.total_estimated_duration_secs, 1)) * 100
    : 0;
  const durationDeltaVsMedian =
    report && benchmarkStats
      ? report.total_estimated_duration_secs - benchmarkStats.duration_median_secs
      : null;
  const improvementDeltaVsMedian =
    benchmarkStats ? savingsPercent - benchmarkStats.improvement_median_pct : null;

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <div className="mx-auto flex w-full max-w-[1440px] flex-col lg:flex-row">
        <aside className="w-full border-b border-zinc-800 bg-zinc-900/70 lg:min-h-screen lg:w-72 lg:border-b-0 lg:border-r">
          <div className="flex items-center gap-3 border-b border-zinc-800 px-6 py-5">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-cyan-500/20 text-cyan-300">
              <Workflow className="h-5 w-5" />
            </div>
            <div>
              <p className="text-xs uppercase tracking-[0.2em] text-zinc-400">PipelineX</p>
              <h1 className="text-xl font-semibold text-zinc-50">Platform Dashboard</h1>
            </div>
          </div>

          <nav className="space-y-2 px-4 py-4">
            <NavItem icon={Activity} label="Overview" active />
            <NavItem icon={Gauge} label="Pipeline Health" />
            <NavItem icon={AlertTriangle} label="Bottlenecks" badge={String(severityCounts.critical)} />
            <NavItem icon={Wrench} label="Optimization Queue" />
          </nav>

          <div className="border-t border-zinc-800 px-6 py-4">
            <p className="text-xs uppercase tracking-[0.2em] text-zinc-500">Roadmap</p>
            <p className="mt-2 text-sm text-zinc-200">Phase 3 active: dashboard + automation APIs.</p>
          </div>
        </aside>

        <main className="flex-1 px-4 py-4 sm:px-6 lg:px-8 lg:py-6">
          <section className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-4 sm:p-5">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
              <div>
                <h2 className="text-lg font-semibold text-zinc-50">Analyze a pipeline file</h2>
                <p className="text-sm text-zinc-400">
                  Run live analysis directly from the dashboard and inspect findings instantly.
                </p>
              </div>

              <div className="flex w-full flex-col gap-3 sm:flex-row lg:w-auto">
                <select
                  value={selectedPath}
                  onChange={(event) => setSelectedPath(event.target.value)}
                  className="min-w-[280px] rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 focus:border-cyan-400 focus:outline-none"
                  disabled={loadingWorkflows || runningAnalysis}
                >
                  {workflows.length === 0 && <option value="">No pipeline files found</option>}
                  {workflows.map((workflowPath) => (
                    <option key={workflowPath} value={workflowPath}>
                      {workflowPath}
                    </option>
                  ))}
                </select>

                <button
                  type="button"
                  onClick={() => void runAnalysis(selectedPath)}
                  disabled={!selectedPath || runningAnalysis}
                  className="inline-flex items-center justify-center gap-2 rounded-lg bg-cyan-500 px-4 py-2 text-sm font-semibold text-zinc-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {runningAnalysis ? <RefreshCw className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
                  {runningAnalysis ? "Analyzing..." : "Run Analysis"}
                </button>

                {report && report.findings.length > 0 && (
                  <button
                    type="button"
                    onClick={() => void applyOptimization(selectedPath)}
                    disabled={!selectedPath || applyingOptimization}
                    className="inline-flex items-center justify-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-semibold text-white transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    {applyingOptimization ? <RefreshCw className="h-4 w-4 animate-spin" /> : <GitMerge className="h-4 w-4" />}
                    {applyingOptimization ? "Creating PR..." : "Apply & Create PR"}
                  </button>
                )}
              </div>
            </div>

            {lastUpdated && (
              <p className="mt-3 text-xs uppercase tracking-[0.16em] text-zinc-500">
                Last updated: {lastUpdated}
              </p>
            )}
          </section>

          {error && (
            <section className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-100">
              {error}
            </section>
          )}

          {applyError && (
            <section className="mt-4 rounded-xl border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-100">
              <strong>Apply Error:</strong> {applyError}
            </section>
          )}

          {applySuccess && (
            <section className="mt-4 rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-sm text-emerald-100">
              <div className="flex items-start gap-2">
                <CheckCircle2 className="h-5 w-5 flex-shrink-0 mt-0.5" />
                <div>
                  <strong>Pull Request Created Successfully!</strong>
                  {applySuccess.prUrl && (
                    <p className="mt-1">
                      <a
                        href={applySuccess.prUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-emerald-200 underline hover:text-emerald-100"
                      >
                        View PR #{applySuccess.prNumber || ""}
                      </a>
                    </p>
                  )}
                  {applySuccess.branch && (
                    <p className="mt-1 text-xs text-emerald-200">
                      Branch: {applySuccess.branch}
                    </p>
                  )}
                </div>
              </div>
            </section>
          )}

          {report && (
            <>
              <section className="mt-5 grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
                <StatCard
                  title="Current Duration"
                  value={formatDuration(report.total_estimated_duration_secs)}
                  subtitle={`Provider: ${report.provider}`}
                  icon={Clock3}
                />
                <StatCard
                  title="Optimized Projection"
                  value={formatDuration(report.optimized_duration_secs)}
                  subtitle={`${percentage(savingsPercent)} faster`}
                  icon={Gauge}
                />
                <StatCard
                  title="Potential Savings"
                  value={formatDuration(savingsSeconds)}
                  subtitle={`${report.findings.length} findings`}
                  icon={CheckCircle2}
                />
                <StatCard
                  title="Health Score"
                  value={report.health_score ? report.health_score.total_score.toFixed(1) : "n/a"}
                  subtitle={report.health_score?.grade ?? "No grade"}
                  icon={Activity}
                />
              </section>

              <section className="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-2">
                <Panel title="Duration Snapshot">
                  <ResponsiveContainer width="100%" height={260}>
                    <AreaChart data={durationData} margin={{ top: 12, right: 8, left: -20, bottom: 0 }}>
                      <defs>
                        <linearGradient id="durationFill" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="0%" stopColor="#22d3ee" stopOpacity={0.4} />
                          <stop offset="100%" stopColor="#22d3ee" stopOpacity={0.06} />
                        </linearGradient>
                      </defs>
                      <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                      <XAxis dataKey="name" stroke="#a1a1aa" />
                      <YAxis stroke="#a1a1aa" />
                      <Tooltip
                        contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                        labelStyle={{ color: "#e4e4e7" }}
                      />
                      <Area
                        type="monotone"
                        dataKey="duration"
                        stroke="#22d3ee"
                        strokeWidth={2}
                        fill="url(#durationFill)"
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                </Panel>

                <Panel title="Finding Severity">
                  <ResponsiveContainer width="100%" height={260}>
                    <BarChart data={severityData} margin={{ top: 12, right: 8, left: -20, bottom: 0 }}>
                      <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                      <XAxis dataKey="name" stroke="#a1a1aa" />
                      <YAxis allowDecimals={false} stroke="#a1a1aa" />
                      <Tooltip
                        contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                        labelStyle={{ color: "#e4e4e7" }}
                      />
                      <Bar dataKey="count" fill="#f97316" radius={[6, 6, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Pipeline Explorer: Interactive DAG (D3)">
                  <DagExplorer report={report} />
                </Panel>
              </section>

              <section className="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-3">
                <Panel title="Trend: Duration">
                  {trendSeries.length > 0 ? (
                    <ResponsiveContainer width="100%" height={220}>
                      <AreaChart data={trendSeries} margin={{ top: 10, right: 8, left: -18, bottom: 0 }}>
                        <defs>
                          <linearGradient id="trendDurationFill" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stopColor="#22d3ee" stopOpacity={0.35} />
                            <stop offset="100%" stopColor="#22d3ee" stopOpacity={0.05} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                        <XAxis dataKey="label" stroke="#a1a1aa" hide />
                        <YAxis stroke="#a1a1aa" />
                        <Tooltip
                          contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                          labelStyle={{ color: "#e4e4e7" }}
                          formatter={(value: number | string | undefined) =>
                            formatDuration(Number(value ?? 0))
                          }
                        />
                        <Area
                          type="monotone"
                          dataKey="durationSec"
                          stroke="#22d3ee"
                          strokeWidth={2}
                          fill="url(#trendDurationFill)"
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      Trend data appears after history snapshots are collected by webhooks or manual refresh.
                    </p>
                  )}
                </Panel>

                <Panel title="Trend: Failure Rate">
                  {trendSeries.length > 0 ? (
                    <ResponsiveContainer width="100%" height={220}>
                      <AreaChart data={trendSeries} margin={{ top: 10, right: 8, left: -18, bottom: 0 }}>
                        <defs>
                          <linearGradient id="trendFailureFill" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stopColor="#f97316" stopOpacity={0.35} />
                            <stop offset="100%" stopColor="#f97316" stopOpacity={0.05} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                        <XAxis dataKey="label" stroke="#a1a1aa" hide />
                        <YAxis stroke="#a1a1aa" />
                        <Tooltip
                          contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                          labelStyle={{ color: "#e4e4e7" }}
                          formatter={(value: number | string | undefined) =>
                            percentage(Number(value ?? 0))
                          }
                        />
                        <Area
                          type="monotone"
                          dataKey="failureRatePct"
                          stroke="#f97316"
                          strokeWidth={2}
                          fill="url(#trendFailureFill)"
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      Failure-rate trend will render once snapshot history is available.
                    </p>
                  )}
                </Panel>

                <Panel title="Trend: Cost / Run">
                  {trendSeries.length > 0 ? (
                    <ResponsiveContainer width="100%" height={220}>
                      <AreaChart data={trendSeries} margin={{ top: 10, right: 8, left: -18, bottom: 0 }}>
                        <defs>
                          <linearGradient id="trendCostFill" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stopColor="#a78bfa" stopOpacity={0.35} />
                            <stop offset="100%" stopColor="#a78bfa" stopOpacity={0.05} />
                          </linearGradient>
                        </defs>
                        <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                        <XAxis dataKey="label" stroke="#a1a1aa" hide />
                        <YAxis stroke="#a1a1aa" />
                        <Tooltip
                          contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                          labelStyle={{ color: "#e4e4e7" }}
                          formatter={(value: number | string | undefined) =>
                            formatUsd(Number(value ?? 0))
                          }
                        />
                        <Area
                          type="monotone"
                          dataKey="costUsdPerRun"
                          stroke="#a78bfa"
                          strokeWidth={2}
                          fill="url(#trendCostFill)"
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      Cost trend is computed from average duration and default labor-rate assumptions.
                    </p>
                  )}
                </Panel>
              </section>

              <section className="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-2">
                <Panel title="Cost Center: Waste Breakdown">
                  {costCenter ? (
                    <>
                      <div className="mb-3 grid grid-cols-1 gap-3 sm:grid-cols-3">
                        <BenchmarkTile
                          label="Monthly Waste"
                          value={formatUsd(costCenter.monthlyWasteUsd)}
                          sublabel={`${costCenter.runsPerMonth} runs/mo at $${costCenter.developerHourlyRate}/hr`}
                          icon={Gauge}
                        />
                        <BenchmarkTile
                          label="Critical Waste"
                          value={formatUsd(costCenter.criticalWasteUsd)}
                          sublabel="critical-severity opportunities"
                          icon={AlertTriangle}
                        />
                        <BenchmarkTile
                          label="Tracked Categories"
                          value={String(costCenter.categories.length)}
                          sublabel="waste sources in current analysis"
                          icon={Workflow}
                        />
                      </div>
                      {costCenter.categories.length > 0 ? (
                        <ResponsiveContainer width="100%" height={260}>
                          <BarChart data={costCenter.categories} margin={{ top: 8, right: 8, left: -8, bottom: 0 }}>
                            <CartesianGrid stroke="#3f3f46" strokeDasharray="3 3" />
                            <XAxis dataKey="category" stroke="#a1a1aa" hide />
                            <YAxis stroke="#a1a1aa" />
                            <Tooltip
                              contentStyle={{ backgroundColor: "#09090b", border: "1px solid #3f3f46" }}
                              labelStyle={{ color: "#e4e4e7" }}
                              formatter={(value: number | string | undefined) =>
                                formatUsd(Number(value ?? 0))
                              }
                            />
                            <Bar dataKey="monthlyWasteUsd" fill="#f97316" radius={[6, 6, 0, 0]} />
                          </BarChart>
                        </ResponsiveContainer>
                      ) : (
                        <p className="text-sm text-zinc-400">
                          No savings estimates are available in findings for this report.
                        </p>
                      )}
                    </>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      Run an analysis to populate cost center metrics.
                    </p>
                  )}
                </Panel>

                <Panel title="Cost Center: Top Waste Sources">
                  {costCenter && costCenter.categories.length > 0 ? (
                    <ul className="space-y-2">
                      {costCenter.categories.map((row) => (
                        <li
                          key={row.category}
                          className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3"
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <p className="text-sm font-semibold text-zinc-100">{row.category}</p>
                            <span className="text-sm font-semibold text-amber-200">
                              {formatUsd(row.monthlyWasteUsd)}/mo
                            </span>
                          </div>
                          <p className="mt-1 text-xs text-zinc-300">
                            {row.findings} finding(s) • Estimated savings {formatDuration(row.savingsSec)}
                          </p>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      Cost center breakdown will appear once findings include estimated savings.
                    </p>
                  )}
                </Panel>
              </section>

              <section className="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-3">
                <Panel title="Critical Path" className="xl:col-span-1">
                  <p className="text-sm text-zinc-300">
                    {report.critical_path.join(" -> ")}
                  </p>
                  <p className="mt-3 text-xs uppercase tracking-[0.16em] text-zinc-500">
                    Path duration: {formatDuration(report.critical_path_duration_secs)}
                  </p>
                </Panel>

                <Panel title="Top Findings" className="xl:col-span-2">
                  <div className="space-y-3">
                    {topFindings.map((finding) => (
                      <FindingRow key={`${finding.title}-${finding.severity}`} finding={finding} />
                    ))}
                    {topFindings.length === 0 && (
                      <p className="text-sm text-zinc-400">No findings for this pipeline.</p>
                    )}
                  </div>
                </Panel>
              </section>

              <section className="mt-5 grid grid-cols-1 gap-4 xl:grid-cols-2">
                <Panel title="Bottleneck Drilldown: Categories">
                  {bottleneckCategoryRows.length > 0 ? (
                    <ul className="space-y-2">
                      {bottleneckCategoryRows.map((row) => (
                        <li
                          key={row.label}
                          className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm"
                        >
                          <span className="text-zinc-200">{row.label}</span>
                          <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                            {row.count}
                          </span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="text-sm text-zinc-400">No category hotspots available.</p>
                  )}
                </Panel>

                <Panel title="Bottleneck Drilldown: Job Hotspots">
                  {bottleneckJobRows.length > 0 ? (
                    <ul className="space-y-2">
                      {bottleneckJobRows.map((row) => (
                        <li
                          key={row.label}
                          className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm"
                        >
                          <span className="text-zinc-200">{row.label}</span>
                          <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-xs text-zinc-300">
                            {row.count}
                          </span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="text-sm text-zinc-400">No job hotspot data available.</p>
                  )}
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Recommended Actions">
                  <ul className="space-y-2">
                    {(report.health_score?.recommendations ?? []).map((recommendation) => (
                      <li key={recommendation} className="text-sm text-zinc-300">
                        {recommendation}
                      </li>
                    ))}
                    {(report.health_score?.recommendations.length ?? 0) === 0 && (
                      <li className="text-sm text-zinc-400">
                        No recommendations available yet for this report.
                      </li>
                    )}
                  </ul>
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Community Benchmarks (Anonymized)">
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <p className="text-sm text-zinc-400">
                      Compare this run against similar pipelines without storing repo identifiers.
                    </p>
                    <button
                      type="button"
                      onClick={() => {
                        if (report) {
                          void refreshBenchmarkStats(report);
                        }
                      }}
                      disabled={!report || benchmarkLoading}
                      className="inline-flex items-center gap-2 rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-1.5 text-xs font-semibold text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <RefreshCw
                        className={`h-3.5 w-3.5 ${benchmarkLoading ? "animate-spin" : ""}`}
                      />
                      Refresh Cohort
                    </button>
                  </div>

                  {benchmarkError && (
                    <p className="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-100">
                      {benchmarkError}
                    </p>
                  )}

                  {benchmarkStats ? (
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
                      <BenchmarkTile
                        label="Cohort"
                        value={benchmarkStats.cohort}
                        sublabel={`${benchmarkStats.sample_count} samples`}
                        icon={Trophy}
                      />
                      <BenchmarkTile
                        label="Median Duration"
                        value={formatDuration(benchmarkStats.duration_median_secs)}
                        sublabel={`P75 ${formatDuration(benchmarkStats.duration_p75_secs)}`}
                        icon={Clock3}
                      />
                      <BenchmarkTile
                        label="Median Improvement"
                        value={percentage(benchmarkStats.improvement_median_pct)}
                        sublabel={`You: ${percentage(savingsPercent)}`}
                        icon={Gauge}
                      />
                      <BenchmarkTile
                        label="Median Findings"
                        value={benchmarkStats.finding_median.toFixed(1)}
                        sublabel={
                          benchmarkStats.health_score_median !== null
                            ? `Health ${benchmarkStats.health_score_median.toFixed(1)}`
                            : "Health n/a"
                        }
                        icon={AlertTriangle}
                      />
                    </div>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      {benchmarkSubmitting
                        ? "Submitting anonymized benchmark sample..."
                        : "No benchmark stats yet for this cohort."}
                    </p>
                  )}

                  {benchmarkStats && durationDeltaVsMedian !== null && improvementDeltaVsMedian !== null && (
                    <div className="mt-3 space-y-1 text-xs text-zinc-300">
                      <p>
                        Duration vs median:{" "}
                        <span className={durationDeltaVsMedian <= 0 ? "text-emerald-300" : "text-amber-300"}>
                          {durationDeltaVsMedian <= 0 ? "faster" : "slower"} by{" "}
                          {formatDuration(Math.abs(durationDeltaVsMedian))}
                        </span>
                      </p>
                      <p>
                        Improvement vs median:{" "}
                        <span
                          className={
                            improvementDeltaVsMedian >= 0
                              ? "text-emerald-300"
                              : "text-amber-300"
                          }
                        >
                          {improvementDeltaVsMedian >= 0 ? "+" : ""}
                          {percentage(improvementDeltaVsMedian)}
                        </span>
                      </p>
                    </div>
                  )}
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Webhook History Cache">
                  <div className="mb-3 flex items-center justify-between">
                    <p className="text-sm text-zinc-400">
                      Recent snapshots refreshed by GitHub or GitLab webhook events.
                    </p>
                    <button
                      type="button"
                      onClick={() => void loadHistorySnapshots()}
                      className="inline-flex items-center gap-2 rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-1.5 text-xs font-semibold text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200"
                    >
                      <RefreshCw className={`h-3.5 w-3.5 ${loadingHistory ? "animate-spin" : ""}`} />
                      Refresh
                    </button>
                  </div>
                  <div className="space-y-3">
                    {historySnapshots.map((snapshot) => (
                      <article
                        key={`${snapshot.repo}-${snapshot.workflow}-${snapshot.refreshed_at}`}
                        className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3"
                      >
                        <p className="text-sm font-semibold text-zinc-100">{snapshot.repo}</p>
                        <p className="text-xs text-zinc-400">{snapshot.workflow}</p>
                        <div className="mt-2 flex flex-wrap gap-3 text-xs text-zinc-300">
                          <span>Runs: {snapshot.stats.total_runs}</span>
                          <span>Success: {percentage(snapshot.stats.success_rate * 100)}</span>
                          <span>Avg: {formatDuration(snapshot.stats.avg_duration_sec)}</span>
                          <span>Updated: {new Date(snapshot.refreshed_at).toLocaleString()}</span>
                        </div>
                      </article>
                    ))}
                    {!loadingHistory && historySnapshots.length === 0 && (
                      <p className="text-sm text-zinc-400">
                        No cached history yet. Send a GitHub `workflow_run` or GitLab `pipeline` webhook to populate this panel.
                      </p>
                    )}
                  </div>
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Alert System (Threshold-Based)">
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <p className="text-sm text-zinc-400">
                      Alert rules evaluate duration, failure rate, and opportunity-cost thresholds.
                    </p>
                    <button
                      type="button"
                      onClick={() => void loadAlerts()}
                      className="inline-flex items-center gap-2 rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-1.5 text-xs font-semibold text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200"
                    >
                      <RefreshCw className={`h-3.5 w-3.5 ${loadingAlerts ? "animate-spin" : ""}`} />
                      Evaluate
                    </button>
                  </div>

                  {alertSummary && (
                    <div className="mb-3 grid grid-cols-1 gap-3 sm:grid-cols-4">
                      <BenchmarkTile
                        label="Rules"
                        value={String(alertSummary.total_rules)}
                        sublabel={`${alertSummary.enabled_rules} enabled`}
                        icon={Wrench}
                      />
                      <BenchmarkTile
                        label="Snapshots"
                        value={String(alertSummary.snapshots_considered)}
                        sublabel="history cache inputs"
                        icon={Workflow}
                      />
                      <BenchmarkTile
                        label="Triggered"
                        value={String(alertSummary.triggered_count)}
                        sublabel="active threshold breaches"
                        icon={AlertTriangle}
                      />
                      <BenchmarkTile
                        label="Defaults"
                        value={`${alertSummary.default_runs_per_month}/mo`}
                        sublabel={`$${alertSummary.default_developer_hourly_rate}/hr`}
                        icon={Clock3}
                      />
                    </div>
                  )}

                  {alertSummary && alertSummary.triggers.length > 0 ? (
                    <div className="space-y-2">
                      {alertSummary.triggers.slice(0, 6).map((trigger) => (
                        <article
                          key={`${trigger.rule_id}-${trigger.repo}-${trigger.workflow}`}
                          className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3"
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <p className="text-sm font-semibold text-zinc-100">{trigger.rule_name}</p>
                            <span
                              className={`rounded-full px-2 py-0.5 text-xs font-semibold ${
                                trigger.severity === "critical"
                                  ? "bg-red-500/20 text-red-200"
                                  : trigger.severity === "high"
                                    ? "bg-amber-500/20 text-amber-200"
                                    : "bg-sky-500/20 text-sky-200"
                              }`}
                            >
                              {trigger.severity}
                            </span>
                          </div>
                          <p className="mt-1 text-xs text-zinc-300">{trigger.message}</p>
                          <p className="mt-1 text-xs text-zinc-400">
                            {trigger.repo} • {trigger.workflow} • {trigger.provider}
                          </p>
                        </article>
                      ))}
                    </div>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      {alertRules.length === 0
                        ? "No alert rules configured yet. Use POST /api/alerts to add threshold rules."
                        : "No threshold breaches detected in current snapshot cache."}
                    </p>
                  )}
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Flaky Test Management (Quarantine / Track / Resolve)">
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <p className="text-sm text-zinc-400">
                      Manage unstable jobs detected from webhook history snapshots.
                    </p>
                    <button
                      type="button"
                      onClick={() => void loadFlaky()}
                      className="inline-flex items-center gap-2 rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-1.5 text-xs font-semibold text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200"
                    >
                      <RefreshCw className={`h-3.5 w-3.5 ${loadingFlaky ? "animate-spin" : ""}`} />
                      Refresh
                    </button>
                  </div>

                  {flakyError && (
                    <p className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-100">
                      {flakyError}
                    </p>
                  )}

                  {flakySummary ? (
                    <>
                      <div className="mb-3 grid grid-cols-1 gap-3 sm:grid-cols-4">
                        <BenchmarkTile
                          label="Total"
                          value={String(flakySummary.total)}
                          sublabel="tracked flaky jobs"
                          icon={AlertTriangle}
                        />
                        <BenchmarkTile
                          label="Open"
                          value={String(flakySummary.open)}
                          sublabel="active investigation"
                          icon={Activity}
                        />
                        <BenchmarkTile
                          label="Quarantined"
                          value={String(flakySummary.quarantined)}
                          sublabel="isolated from critical path"
                          icon={Wrench}
                        />
                        <BenchmarkTile
                          label="Resolved"
                          value={String(flakySummary.resolved)}
                          sublabel="stable after remediation"
                          icon={CheckCircle2}
                        />
                      </div>

                      <div className="space-y-2">
                        {flakySummary.jobs.slice(0, 8).map((job) => (
                          <article
                            key={job.id}
                            className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3"
                          >
                            <div className="flex flex-wrap items-center justify-between gap-2">
                              <div>
                                <p className="text-sm font-semibold text-zinc-100">{job.job_name}</p>
                                <p className="text-xs text-zinc-400">
                                  {job.repo} • {job.workflow}
                                </p>
                              </div>
                              <span
                                className={`rounded-full px-2 py-0.5 text-xs font-semibold ${
                                  job.status === "open"
                                    ? "bg-red-500/20 text-red-200"
                                    : job.status === "quarantined"
                                      ? "bg-amber-500/20 text-amber-200"
                                      : "bg-emerald-500/20 text-emerald-200"
                                }`}
                              >
                                {job.status}
                              </span>
                            </div>
                            <p className="mt-1 text-xs text-zinc-300">
                              Observed in {job.observed_count} snapshot(s) • Last seen{" "}
                              {new Date(job.last_seen_at).toLocaleString()}
                            </p>
                            <div className="mt-2 flex flex-wrap gap-2">
                              <button
                                type="button"
                                onClick={() => void updateFlakyStatus(job, "quarantined")}
                                disabled={flakyUpdatingId === job.id}
                                className="rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 transition hover:border-amber-400 hover:text-amber-200 disabled:opacity-50"
                              >
                                Quarantine
                              </button>
                              <button
                                type="button"
                                onClick={() => void updateFlakyStatus(job, "resolved")}
                                disabled={flakyUpdatingId === job.id}
                                className="rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 transition hover:border-emerald-400 hover:text-emerald-200 disabled:opacity-50"
                              >
                                Resolve
                              </button>
                              <button
                                type="button"
                                onClick={() => void updateFlakyStatus(job, "open")}
                                disabled={flakyUpdatingId === job.id}
                                className="rounded-md border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200 disabled:opacity-50"
                              >
                                Reopen
                              </button>
                            </div>
                          </article>
                        ))}
                        {flakySummary.jobs.length === 0 && (
                          <p className="text-sm text-zinc-400">
                            No flaky jobs detected in current history snapshots.
                          </p>
                        )}
                      </div>
                    </>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      No flaky summary available yet.
                    </p>
                  )}
                </Panel>
              </section>

              <section className="mt-5">
                <Panel title="Weekly Digest Reports (Slack / Teams / Email)">
                  <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                    <p className="text-sm text-zinc-400">
                      Generate a weekly summary from cached snapshots and optionally deliver using configured channels.
                    </p>
                    <div className="flex gap-2">
                      <button
                        type="button"
                        onClick={() => void loadDigest(false)}
                        disabled={digestLoading}
                        className="inline-flex items-center gap-2 rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-1.5 text-xs font-semibold text-zinc-200 transition hover:border-cyan-400 hover:text-cyan-200 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <RefreshCw className={`h-3.5 w-3.5 ${digestLoading ? "animate-spin" : ""}`} />
                        Refresh
                      </button>
                      <button
                        type="button"
                        onClick={() => void loadDigest(true)}
                        disabled={digestLoading}
                        className="inline-flex items-center gap-2 rounded-lg bg-cyan-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        Deliver
                      </button>
                    </div>
                  </div>

                  {digestError && (
                    <p className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-100">
                      {digestError}
                    </p>
                  )}

                  {digestSummary ? (
                    <>
                      <div className="grid grid-cols-1 gap-3 sm:grid-cols-4">
                        <BenchmarkTile
                          label="Snapshots"
                          value={String(digestSummary.snapshot_count)}
                          sublabel={`Window ${digestSummary.window_days} day(s)`}
                          icon={Workflow}
                        />
                        <BenchmarkTile
                          label="Total Runs"
                          value={String(digestSummary.total_runs)}
                          sublabel={`Avg ${formatDuration(digestSummary.avg_duration_sec)}`}
                          icon={Clock3}
                        />
                        <BenchmarkTile
                          label="Failure Rate"
                          value={percentage(digestSummary.failure_rate_pct)}
                          sublabel="weighted across snapshots"
                          icon={AlertTriangle}
                        />
                        <BenchmarkTile
                          label="Monthly Cost"
                          value={formatUsd(digestSummary.estimated_monthly_opportunity_cost_usd)}
                          sublabel="estimated opportunity cost"
                          icon={Gauge}
                        />
                      </div>

                      <div className="mt-3 grid grid-cols-1 gap-4 xl:grid-cols-2">
                        <div>
                          <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">Action Items</p>
                          <ul className="mt-2 space-y-2">
                            {digestSummary.action_items.map((item) => (
                              <li key={item} className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-300">
                                {item}
                              </li>
                            ))}
                          </ul>
                        </div>
                        <div>
                          <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">Top Slow Pipelines</p>
                          <ul className="mt-2 space-y-2">
                            {digestSummary.top_slowest_pipelines.slice(0, 4).map((pipeline) => (
                              <li
                                key={`${pipeline.repo}-${pipeline.workflow}-${pipeline.refreshed_at}`}
                                className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-300"
                              >
                                <p className="font-semibold text-zinc-100">{pipeline.repo}</p>
                                <p className="text-xs text-zinc-400">{pipeline.workflow}</p>
                                <p className="mt-1 text-xs text-zinc-300">
                                  {formatDuration(pipeline.avg_duration_sec)} avg • {percentage(pipeline.failure_rate_pct)} failure
                                </p>
                              </li>
                            ))}
                            {digestSummary.top_slowest_pipelines.length === 0 && (
                              <li className="rounded-lg border border-zinc-800 bg-zinc-950/60 px-3 py-2 text-sm text-zinc-400">
                                No snapshot data in the selected digest window.
                              </li>
                            )}
                          </ul>
                        </div>
                      </div>

                      {digestDelivery && (
                        <p className="mt-3 text-xs text-zinc-300">
                          Delivery status: Slack {digestDelivery.slack_sent ? "sent" : "not sent"} • Teams{" "}
                          {digestDelivery.teams_sent ? "sent" : "not sent"} • Email queued {digestDelivery.email_queued}
                          {digestDelivery.email_outbox_path ? ` • Outbox ${digestDelivery.email_outbox_path}` : ""}
                        </p>
                      )}
                    </>
                  ) : (
                    <p className="text-sm text-zinc-400">
                      No weekly digest has been generated yet.
                    </p>
                  )}
                </Panel>
              </section>
            </>
          )}

          {/* Team Management Section */}
          <section className="mt-5">
            <Panel title="Team Management">
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Users className="h-5 w-5 text-cyan-400" />
                    <h3 className="text-sm font-semibold text-zinc-100">Teams ({teams.length})</h3>
                  </div>
                  {!showCreateTeam && (
                    <button
                      type="button"
                      onClick={() => setShowCreateTeam(true)}
                      className="inline-flex items-center gap-1 rounded-lg bg-cyan-500/20 px-3 py-1.5 text-xs font-medium text-cyan-200 transition hover:bg-cyan-500/30"
                    >
                      <Plus className="h-3 w-3" />
                      New Team
                    </button>
                  )}
                </div>

                {showCreateTeam && (
                  <div className="rounded-lg border border-zinc-700 bg-zinc-900 p-3">
                    <div className="space-y-3">
                      <div>
                        <label className="mb-1 block text-xs text-zinc-400">Team Name</label>
                        <input
                          type="text"
                          value={newTeamName}
                          onChange={(e) => setNewTeamName(e.target.value)}
                          placeholder="Engineering, QA, DevOps..."
                          className="w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 focus:border-cyan-400 focus:outline-none"
                        />
                      </div>
                      <div className="flex gap-2">
                        <button
                          type="button"
                          onClick={() => void createTeam()}
                          disabled={!newTeamName.trim()}
                          className="rounded-lg bg-cyan-500 px-3 py-1.5 text-xs font-semibold text-zinc-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                          Create
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setShowCreateTeam(false);
                            setNewTeamName("");
                          }}
                          className="rounded-lg bg-zinc-700 px-3 py-1.5 text-xs font-semibold text-zinc-100 transition hover:bg-zinc-600"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  </div>
                )}

                {loadingTeams ? (
                  <div className="flex items-center gap-2 py-6 text-sm text-zinc-400">
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    Loading teams...
                  </div>
                ) : teams.length === 0 ? (
                  <p className="py-6 text-center text-sm text-zinc-400">
                    No teams yet. Create your first team to get started.
                  </p>
                ) : (
                  <div className="space-y-2">
                    {teams.map((team) => (
                      <div
                        key={team.id}
                        className="rounded-lg border border-zinc-700 bg-zinc-900 p-3"
                      >
                        <div className="flex items-start justify-between">
                          <div>
                            <h4 className="font-semibold text-zinc-100">{team.name}</h4>
                            {team.description && (
                              <p className="mt-1 text-xs text-zinc-400">{team.description}</p>
                            )}
                            <div className="mt-2 flex items-center gap-3 text-xs text-zinc-500">
                              <span>{team.members.length} members</span>
                              <span>•</span>
                              <span>{team.settings.pipeline_paths?.length || 0} pipelines</span>
                              <span>•</span>
                              <span>Created {new Date(team.created_at).toLocaleDateString()}</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </Panel>
          </section>

          {/* Organization Metrics Section */}
          {orgMetrics && (
            <section className="mt-5">
              <Panel title="Organization Overview">
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                  <div className="rounded-lg bg-zinc-800/50 p-3">
                    <p className="text-xs uppercase tracking-wide text-zinc-400">Total Teams</p>
                    <p className="mt-1 text-2xl font-bold text-zinc-100">{orgMetrics.total_teams}</p>
                  </div>
                  <div className="rounded-lg bg-zinc-800/50 p-3">
                    <p className="text-xs uppercase tracking-wide text-zinc-400">Total Pipelines</p>
                    <p className="mt-1 text-2xl font-bold text-zinc-100">{orgMetrics.total_pipelines}</p>
                  </div>
                  <div className="rounded-lg bg-zinc-800/50 p-3">
                    <p className="text-xs uppercase tracking-wide text-zinc-400">Avg Health Score</p>
                    <p className="mt-1 text-2xl font-bold text-cyan-400">
                      {orgMetrics.avg_health_score.toFixed(1)}
                    </p>
                  </div>
                  <div className="rounded-lg bg-zinc-800/50 p-3">
                    <p className="text-xs uppercase tracking-wide text-zinc-400">Monthly Cost</p>
                    <p className="mt-1 text-2xl font-bold text-red-400">
                      ${orgMetrics.total_monthly_cost.toFixed(0)}
                    </p>
                  </div>
                </div>

                {orgMetrics.teams_summary.length > 0 && (
                  <div className="mt-4">
                    <h4 className="mb-2 text-xs uppercase tracking-wide text-zinc-400">Teams Breakdown</h4>
                    <div className="space-y-2">
                      {orgMetrics.teams_summary.map((teamSummary) => (
                        <div
                          key={teamSummary.team_id}
                          className="flex items-center justify-between rounded-lg bg-zinc-800/50 p-2 text-sm"
                        >
                          <span className="font-medium text-zinc-100">{teamSummary.team_name}</span>
                          <div className="flex items-center gap-4 text-xs text-zinc-400">
                            <span>{teamSummary.pipeline_count} pipelines</span>
                            <span>{formatDuration(teamSummary.avg_duration_secs)}</span>
                            <span className="text-red-400">${teamSummary.monthly_cost.toFixed(0)}/mo</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </Panel>
            </section>
          )}
        </main>
      </div>
    </div>
  );
}

function NavItem({
  icon: Icon,
  label,
  active = false,
  badge,
}: {
  icon: LucideIcon;
  label: string;
  active?: boolean;
  badge?: string;
}) {
  return (
    <div
      className={`flex items-center justify-between rounded-lg px-3 py-2 text-sm transition ${
        active
          ? "bg-cyan-500/20 text-cyan-200"
          : "text-zinc-400 hover:bg-zinc-800/80 hover:text-zinc-100"
      }`}
    >
      <span className="inline-flex items-center gap-2">
        <Icon className="h-4 w-4" />
        {label}
      </span>
      {badge && (
        <span className="rounded-full border border-red-400/50 bg-red-500/15 px-2 py-0.5 text-xs text-red-200">
          {badge}
        </span>
      )}
    </div>
  );
}

function Panel({
  title,
  children,
  className = "",
}: {
  title: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`rounded-2xl border border-zinc-800 bg-zinc-900/60 p-4 ${className}`}>
      <h3 className="mb-3 text-sm uppercase tracking-[0.16em] text-zinc-400">{title}</h3>
      {children}
    </section>
  );
}

function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
}: {
  title: string;
  value: string;
  subtitle: string;
  icon: LucideIcon;
}) {
  return (
    <article className="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-4">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">{title}</p>
          <p className="mt-2 text-2xl font-semibold text-zinc-50">{value}</p>
          <p className="mt-1 text-sm text-zinc-400">{subtitle}</p>
        </div>
        <span className="rounded-lg bg-zinc-800 p-2 text-cyan-300">
          <Icon className="h-5 w-5" />
        </span>
      </div>
    </article>
  );
}

function FindingRow({ finding }: { finding: Finding }) {
  return (
    <article className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className={`rounded-full border px-2 py-0.5 text-xs ${severityColor(finding.severity)}`}>
          {finding.severity}
        </span>
        <p className="text-sm font-semibold text-zinc-100">{finding.title}</p>
      </div>
      <p className="mt-2 text-sm text-zinc-300">{finding.description}</p>
      <p className="mt-2 text-xs text-zinc-400">
        Recommended: {finding.recommendation}
      </p>
    </article>
  );
}

function BenchmarkTile({
  label,
  value,
  sublabel,
  icon: Icon,
}: {
  label: string;
  value: string;
  sublabel: string;
  icon: LucideIcon;
}) {
  return (
    <article className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-3">
      <div className="mb-2 inline-flex rounded-lg bg-zinc-800 p-2 text-cyan-300">
        <Icon className="h-4 w-4" />
      </div>
      <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">{label}</p>
      <p className="mt-1 text-lg font-semibold text-zinc-50">{value}</p>
      <p className="mt-1 text-xs text-zinc-400">{sublabel}</p>
    </article>
  );
}
