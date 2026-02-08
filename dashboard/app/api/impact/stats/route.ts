import { NextResponse } from "next/server";
import { queryOptimizationImpactStats } from "@/lib/pipelinex";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const source = searchParams.get("source")?.trim() || undefined;
  const provider = searchParams.get("provider")?.trim() || undefined;

  try {
    const stats = await queryOptimizationImpactStats({ source, provider });
    if (!stats) {
      return NextResponse.json(
        { error: "No optimization impact data available for the requested filter." },
        { status: 404 },
      );
    }

    return NextResponse.json({ stats });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to query optimization impact stats.",
      },
      { status: 500 },
    );
  }
}
