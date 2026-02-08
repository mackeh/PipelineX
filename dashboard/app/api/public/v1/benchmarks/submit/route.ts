import { NextResponse } from "next/server";
import { submitBenchmarkReport, type AnalysisReport } from "@/lib/pipelinex";
import {
  applyRateLimitHeaders,
  authenticatePublicApiRequest,
} from "@/lib/public-api";

export const runtime = "nodejs";

interface SubmitBenchmarkBody {
  report?: AnalysisReport;
  source?: string;
}

export async function POST(request: Request) {
  const auth = authenticatePublicApiRequest(request, "benchmarks:write");
  if (!auth.ok) {
    return auth.response;
  }

  let body: SubmitBenchmarkBody = {};
  try {
    body = (await request.json()) as SubmitBenchmarkBody;
  } catch {
    const response = NextResponse.json(
      { error: "Invalid JSON body. Expected: { report: AnalysisReport }" },
      { status: 400 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }

  if (!body.report) {
    const response = NextResponse.json(
      { error: "report is required." },
      { status: 400 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }

  try {
    const source = body.source
      ? `public-api:${auth.principal.id}:${body.source}`
      : `public-api:${auth.principal.id}`;
    const result = await submitBenchmarkReport(body.report, source);
    const response = NextResponse.json(result, { status: 201 });
    return applyRateLimitHeaders(response, auth.rateLimit);
  } catch (error) {
    const response = NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to submit benchmark report.",
      },
      { status: 500 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }
}
