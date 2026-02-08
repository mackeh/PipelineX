import { NextResponse } from "next/server";
import { analyzePipelineFile } from "@/lib/pipelinex";

export const runtime = "nodejs";

interface AnalyzeRequestBody {
  pipelinePath?: string;
}

export async function POST(request: Request) {
  let body: AnalyzeRequestBody = {};

  try {
    body = (await request.json()) as AnalyzeRequestBody;
  } catch {
    return NextResponse.json(
      { error: "Invalid JSON body. Expected: { pipelinePath: string }" },
      { status: 400 },
    );
  }

  if (!body.pipelinePath || body.pipelinePath.trim().length === 0) {
    return NextResponse.json(
      { error: "pipelinePath is required." },
      { status: 400 },
    );
  }

  try {
    const report = await analyzePipelineFile(body.pipelinePath);
    return NextResponse.json({ report });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Pipeline analysis failed unexpectedly.",
      },
      { status: 500 },
    );
  }
}
