import { NextResponse } from "next/server";
import { queryBenchmarkStats } from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
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
  const auth = await authenticatePublicApiRequest(request, "benchmarks:read");
  if (!auth.ok) {
    return auth.response;
  }

  const { searchParams } = new URL(request.url);
  const provider = searchParams.get("provider")?.trim() || "";

  if (!provider) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      { error: "provider query parameter is required." },
      { status: 400 },
      ),
      "Missing provider query parameter.",
    );
  }

  let jobCount = 0;
  let stepCount = 0;
  try {
    jobCount = parsePositiveInt(searchParams.get("jobCount"), "jobCount");
    stepCount = parsePositiveInt(searchParams.get("stepCount"), "stepCount");
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Invalid benchmark query.",
      },
      { status: 400 },
      ),
      "Invalid benchmark stats query.",
    );
  }

  try {
    const stats = await queryBenchmarkStats({ provider, jobCount, stepCount });
    if (!stats) {
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json(
        { error: "No benchmark data available for the requested cohort." },
        { status: 404 },
        ),
        "No benchmark stats for cohort.",
      );
    }

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ stats }),
      "Benchmark stats returned.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to query benchmark stats.",
      },
      { status: 500 },
      ),
      "Benchmark stats query failed.",
    );
  }
}
