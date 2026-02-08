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
import type { AnalysisReport, Finding } from "@/lib/pipelinex";

type WorkflowsResponse = {
  files?: string[];
  error?: string;
};

type AnalyzeResponse = {
  report?: AnalysisReport;
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
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<string | null>(null);

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
    } catch (analysisError) {
      setError(
        analysisError instanceof Error ? analysisError.message : "Analysis failed unexpectedly.",
      );
    } finally {
      setRunningAnalysis(false);
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

    return () => {
      mounted = false;
    };
  }, [runAnalysis]);

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
  const savingsSeconds = report
    ? Math.max(0, report.total_estimated_duration_secs - report.optimized_duration_secs)
    : 0;
  const savingsPercent = report
    ? (savingsSeconds / Math.max(report.total_estimated_duration_secs, 1)) * 100
    : 0;

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
