"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Gauge,
  Play,
  RefreshCw,
  Trophy,
  Workflow,
  Wrench,
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
  Finding,
  HistorySnapshot,
} from "@/lib/pipelinex";

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

function formatDuration(seconds: number): string {
  const safeSeconds = Math.max(0, Math.round(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  const remainder = safeSeconds % 60;
  return `${minutes}m ${remainder}s`;
}

function percentage(value: number): string {
  return `${value.toFixed(1)}%`;
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
  const [benchmarkStats, setBenchmarkStats] = useState<BenchmarkStats | null>(null);
  const [benchmarkSubmitting, setBenchmarkSubmitting] = useState(false);
  const [benchmarkLoading, setBenchmarkLoading] = useState(false);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
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

  const loadHistorySnapshots = useCallback(async () => {
    setLoadingHistory(true);
    try {
      const response = await fetch("/api/history");
      const payload = (await response.json()) as HistoryListResponse;
      if (!response.ok || !payload.snapshots) {
        throw new Error(payload.error || "Failed to load history snapshots.");
      }
      setHistorySnapshots(payload.snapshots.slice(0, 6));
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

    return () => {
      mounted = false;
    };
  }, [loadAlerts, loadHistorySnapshots, runAnalysis]);

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
            </>
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
