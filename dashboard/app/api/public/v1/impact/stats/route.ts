import { NextResponse } from "next/server";
import { queryOptimizationImpactStats } from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "impact:read");
  if (!auth.ok) {
    return auth.response;
  }

  const { searchParams } = new URL(request.url);
  const source = searchParams.get("source")?.trim() || undefined;
  const provider = searchParams.get("provider")?.trim() || undefined;

  try {
    const stats = await queryOptimizationImpactStats({ source, provider });
    if (!stats) {
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json(
          { error: "No optimization impact data available for the requested filter." },
          { status: 404 },
        ),
        "No optimization impact stats for filter.",
      );
    }

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ stats }),
      "Optimization impact stats returned.",
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
              : "Failed to query optimization impact stats.",
        },
        { status: 500 },
      ),
      "Optimization impact stats query failed.",
    );
  }
}
