import { NextResponse } from "next/server";
import { submitBenchmarkReport, type AnalysisReport } from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

interface SubmitBenchmarkBody {
  report?: AnalysisReport;
  source?: string;
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
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(result, { status: 201 }),
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
