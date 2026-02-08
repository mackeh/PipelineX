import { NextResponse } from "next/server";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "benchmarks:read");
  if (!auth.ok) {
    return auth.response;
  }

  return finalizePublicApiResponse(
    request,
    auth,
    NextResponse.json({
      id: auth.principal.id,
      scopes: auth.principal.scopes,
    }),
    "Principal metadata returned.",
  );
}
