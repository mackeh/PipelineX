import { NextResponse } from "next/server";
import { analyzePipelineFile } from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

interface AnalyzeRequestBody {
  pipelinePath?: string;
}

export async function POST(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "analysis:run");
  if (!auth.ok) {
    return auth.response;
  }

  let body: AnalyzeRequestBody = {};

  try {
    body = (await request.json()) as AnalyzeRequestBody;
  } catch {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        { error: "Invalid JSON body. Expected: { pipelinePath: string }" },
        { status: 400 },
      ),
      "Invalid public analyze payload.",
    );
  }

  if (!body.pipelinePath || body.pipelinePath.trim().length === 0) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ error: "pipelinePath is required." }, { status: 400 }),
      "Missing pipelinePath for public analyze request.",
    );
  }

  try {
    const report = await analyzePipelineFile(body.pipelinePath);
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ report }),
      "Public analyze completed.",
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
              : "Pipeline analysis failed unexpectedly.",
        },
        { status: 500 },
      ),
      "Public analyze failed.",
    );
  }
}
