import { NextResponse } from "next/server";
import { listPipelineFiles } from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "workflows:read");
  if (!auth.ok) {
    return auth.response;
  }

  try {
    const files = await listPipelineFiles();
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ files }),
      "Workflow list returned.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error:
            error instanceof Error ? error.message : "Failed to list pipeline files.",
        },
        { status: 500 },
      ),
      "Workflow list failed.",
    );
  }
}
