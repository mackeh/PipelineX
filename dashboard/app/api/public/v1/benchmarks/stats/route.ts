import { NextResponse } from "next/server";
import { queryBenchmarkStats } from "@/lib/pipelinex";
import {
  applyRateLimitHeaders,
  authenticatePublicApiRequest,
} from "@/lib/public-api";

export const runtime = "nodejs";

function parsePositiveInt(value: string | null, fieldName: string): number {
  if (!value) {
    throw new Error(`${fieldName} is required.`);
  }

  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${fieldName} must be a positive integer.`);
  }

  return parsed;
}

export async function GET(request: Request) {
  const auth = authenticatePublicApiRequest(request, "benchmarks:read");
  if (!auth.ok) {
    return auth.response;
  }

  const { searchParams } = new URL(request.url);
  const provider = searchParams.get("provider")?.trim() || "";

  if (!provider) {
    const response = NextResponse.json(
      { error: "provider query parameter is required." },
      { status: 400 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }

  let jobCount = 0;
  let stepCount = 0;
  try {
    jobCount = parsePositiveInt(searchParams.get("jobCount"), "jobCount");
    stepCount = parsePositiveInt(searchParams.get("stepCount"), "stepCount");
  } catch (error) {
    const response = NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Invalid benchmark query.",
      },
      { status: 400 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }

  try {
    const stats = await queryBenchmarkStats({ provider, jobCount, stepCount });
    if (!stats) {
      const response = NextResponse.json(
        { error: "No benchmark data available for the requested cohort." },
        { status: 404 },
      );
      return applyRateLimitHeaders(response, auth.rateLimit);
    }

    const response = NextResponse.json({ stats });
    return applyRateLimitHeaders(response, auth.rateLimit);
  } catch (error) {
    const response = NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to query benchmark stats.",
      },
      { status: 500 },
    );
    return applyRateLimitHeaders(response, auth.rateLimit);
  }
}
