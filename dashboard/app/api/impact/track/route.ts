import { NextResponse } from "next/server";
import {
  trackOptimizationImpact,
  trackOptimizationImpactFromReport,
  type AnalysisReport,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

interface TrackImpactBody {
  source?: string;
  provider?: string;
  runsPerMonth?: number;
  beforeDurationSecs?: number;
  afterDurationSecs?: number;
  report?: AnalysisReport;
}

function parsePositiveNumber(value: number | undefined, fieldName: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    throw new Error(`${fieldName} must be a positive number.`);
  }
  return value;
}

function resolveRunsPerMonth(value: number | undefined): number {
  if (typeof value === "number") {
    return Math.floor(parsePositiveNumber(value, "runsPerMonth"));
  }

  const envDefault = process.env.PIPELINEX_IMPACT_DEFAULT_RUNS_PER_MONTH?.trim();
  if (envDefault) {
    const parsed = Number.parseInt(envDefault, 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      return parsed;
    }
  }

  return 100;
}

export async function POST(request: Request) {
  let body: TrackImpactBody = {};

  try {
    body = (await request.json()) as TrackImpactBody;
  } catch {
    return NextResponse.json(
      {
        error:
          "Invalid JSON body. Expected report-based or explicit duration payload.",
      },
      { status: 400 },
    );
  }

  try {
    const runsPerMonth = resolveRunsPerMonth(body.runsPerMonth);

    if (body.report) {
      const source = body.source ?? "dashboard";
      const result = await trackOptimizationImpactFromReport(
        body.report,
        runsPerMonth,
        source,
      );
      return NextResponse.json(result, { status: 201 });
    }

    const beforeDurationSecs = parsePositiveNumber(
      body.beforeDurationSecs,
      "beforeDurationSecs",
    );
    const afterDurationSecs = parsePositiveNumber(
      body.afterDurationSecs,
      "afterDurationSecs",
    );

    const result = await trackOptimizationImpact({
      source: body.source ?? "dashboard",
      provider: body.provider ?? "unknown",
      beforeDurationSecs,
      afterDurationSecs,
      runsPerMonth,
    });

    return NextResponse.json(result, { status: 201 });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to track optimization impact.",
      },
      { status: 400 },
    );
  }
}
