import { NextResponse } from "next/server";
import {
  trackOptimizationImpact,
  trackOptimizationImpactFromReport,
  type AnalysisReport,
} from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

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
  const auth = await authenticatePublicApiRequest(request, "impact:write");
  if (!auth.ok) {
    return auth.response;
  }

  let body: TrackImpactBody = {};
  try {
    body = (await request.json()) as TrackImpactBody;
  } catch {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error:
            "Invalid JSON body. Expected report-based or explicit duration payload.",
        },
        { status: 400 },
      ),
      "Invalid optimization impact payload.",
    );
  }

  try {
    const runsPerMonth = resolveRunsPerMonth(body.runsPerMonth);
    const source = body.source
      ? `public-api:${auth.principal.id}:${body.source}`
      : `public-api:${auth.principal.id}`;

    const result = body.report
      ? await trackOptimizationImpactFromReport(body.report, runsPerMonth, source)
      : await trackOptimizationImpact({
          source,
          provider: body.provider ?? "unknown",
          beforeDurationSecs: parsePositiveNumber(
            body.beforeDurationSecs,
            "beforeDurationSecs",
          ),
          afterDurationSecs: parsePositiveNumber(
            body.afterDurationSecs,
            "afterDurationSecs",
          ),
          runsPerMonth,
        });

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(result, { status: 201 }),
      "Optimization impact tracked.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error:
            error instanceof Error
              ? error.message
              : "Failed to track optimization impact.",
        },
        { status: 400 },
      ),
      "Optimization impact tracking failed.",
    );
  }
}
