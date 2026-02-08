import { NextResponse } from "next/server";
import { submitBenchmarkReport, type AnalysisReport } from "@/lib/pipelinex";

export const runtime = "nodejs";

interface SubmitBenchmarkBody {
  report?: AnalysisReport;
  source?: string;
}

export async function POST(request: Request) {
  let body: SubmitBenchmarkBody = {};

  try {
    body = (await request.json()) as SubmitBenchmarkBody;
  } catch {
    return NextResponse.json(
      { error: "Invalid JSON body. Expected: { report: AnalysisReport }" },
      { status: 400 },
    );
  }

  if (!body.report) {
    return NextResponse.json({ error: "report is required." }, { status: 400 });
  }

  try {
    const result = await submitBenchmarkReport(body.report, body.source ?? "dashboard");
    return NextResponse.json(result);
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to submit benchmark report.",
      },
      { status: 500 },
    );
  }
}
