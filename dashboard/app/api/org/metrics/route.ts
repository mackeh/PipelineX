import { NextResponse } from "next/server";
import { calculateOrgLevelMetrics, type OrgLevelMetrics } from "@/lib/pipelinex";

type OrgMetricsResponse = {
  metrics?: OrgLevelMetrics;
  error?: string;
};

/**
 * GET /api/org/metrics - Get organization-level metrics
 */
export async function GET(): Promise<NextResponse<OrgMetricsResponse>> {
  try {
    const metrics = await calculateOrgLevelMetrics();
    return NextResponse.json({ metrics });
  } catch (error) {
    console.error("Failed to calculate org metrics:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to calculate org metrics",
      },
      { status: 500 }
    );
  }
}
