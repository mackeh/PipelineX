import { NextResponse } from "next/server";
import {
  applyRateLimitHeaders,
  authenticatePublicApiRequest,
} from "@/lib/public-api";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const auth = authenticatePublicApiRequest(request, "benchmarks:read");
  if (!auth.ok) {
    return auth.response;
  }

  const response = NextResponse.json({
    id: auth.principal.id,
    scopes: auth.principal.scopes,
  });
  return applyRateLimitHeaders(response, auth.rateLimit);
}
