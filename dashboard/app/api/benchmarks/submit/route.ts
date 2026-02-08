import { NextResponse } from "next/server";
import {
  submitBenchmarkReport,
  trackOptimizationImpactFromReport,
  type AnalysisReport,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

interface SubmitBenchmarkBody {
  report?: AnalysisReport;
  source?: string;
  runsPerMonth?: number;
}

function resolveRunsPerMonth(value: number | undefined): number | null {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.floor(value);
  }

  const envDefault = process.env.PIPELINEX_IMPACT_DEFAULT_RUNS_PER_MONTH?.trim();
  if (!envDefault) {
    return null;
  }

  const parsed = Number.parseInt(envDefault, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

export async function POST(request: Request) {
  let body: SubmitBenchmarkBody = {};

  try {
    body = (await request.json()) as SubmitBenchmarkBody;
  } catch {
    return NextResponse.json(
      { error: "Invalid JSON body. Expected: { report: AnalysisReport }" },
      { status: 400 },
    );
  }

  if (!body.report) {
    return NextResponse.json({ error: "report is required." }, { status: 400 });
  }

  try {
    const result = await submitBenchmarkReport(body.report, body.source ?? "dashboard");
    const runsPerMonth = resolveRunsPerMonth(body.runsPerMonth);

    if (!runsPerMonth) {
      return NextResponse.json(result);
    }

    const impact = await trackOptimizationImpactFromReport(
      body.report,
      runsPerMonth,
      body.source ?? "dashboard",
    );
    return NextResponse.json({ ...result, impact });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to submit benchmark report.",
      },
      { status: 500 },
    );
  }
}
