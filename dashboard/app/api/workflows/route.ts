import { NextResponse } from "next/server";
import { listPipelineFiles } from "@/lib/pipelinex";

export const runtime = "nodejs";

export async function GET() {
  try {
    const files = await listPipelineFiles();
    return NextResponse.json({ files });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to list pipeline files.",
      },
      { status: 500 },
    );
  }
}
