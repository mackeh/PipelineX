import { NextResponse } from "next/server";
import {
  submitBenchmarkReport,
  trackOptimizationImpactFromReport,
  type AnalysisReport,
} from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

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
  const auth = await authenticatePublicApiRequest(request, "benchmarks:write");
  if (!auth.ok) {
    return auth.response;
  }

  let body: SubmitBenchmarkBody = {};
  try {
    body = (await request.json()) as SubmitBenchmarkBody;
  } catch {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      { error: "Invalid JSON body. Expected: { report: AnalysisReport }" },
      { status: 400 },
      ),
      "Invalid benchmark submission payload.",
    );
  }

  if (!body.report) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      { error: "report is required." },
      { status: 400 },
      ),
      "Missing report in benchmark submission.",
    );
  }

  try {
    const source = body.source
      ? `public-api:${auth.principal.id}:${body.source}`
      : `public-api:${auth.principal.id}`;
    const result = await submitBenchmarkReport(body.report, source);
    const runsPerMonth = resolveRunsPerMonth(body.runsPerMonth);

    if (!runsPerMonth) {
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json(result, { status: 201 }),
        "Benchmark submitted.",
      );
    }

    const impact = await trackOptimizationImpactFromReport(
      body.report,
      runsPerMonth,
      source,
    );

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ ...result, impact }, { status: 201 }),
      "Benchmark submitted.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to submit benchmark report.",
      },
      { status: 500 },
      ),
      "Benchmark submission failed.",
    );
  }
}
