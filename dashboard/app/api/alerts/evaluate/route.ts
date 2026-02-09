import { NextResponse } from "next/server";
import { evaluateAlertRules } from "@/lib/pipelinex";

export const runtime = "nodejs";

function parsePositiveNumber(value: string | null): number | undefined {
  if (!value || value.trim().length === 0) {
    return undefined;
  }
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error("Query values must be positive numbers when provided.");
  }
  return parsed;
}

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);

  let runsPerMonth: number | undefined;
  let developerHourlyRate: number | undefined;
  try {
    runsPerMonth = parsePositiveNumber(searchParams.get("runsPerMonth"));
    developerHourlyRate = parsePositiveNumber(searchParams.get("developerHourlyRate"));
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Invalid alert evaluation query.",
      },
      { status: 400 },
    );
  }

  try {
    const summary = await evaluateAlertRules({
      runsPerMonth,
      developerHourlyRate,
    });
    return NextResponse.json({ summary });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to evaluate alert rules.",
      },
      { status: 500 },
    );
  }
}
