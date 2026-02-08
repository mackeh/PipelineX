import { NextResponse } from "next/server";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
  queryPublicApiAuditLogs,
  type PublicApiAuditQuery,
} from "@/lib/public-api";

export const runtime = "nodejs";

function parsePositiveInt(value: string | null, fieldName: string): number | undefined {
  if (!value) {
    return undefined;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${fieldName} must be a positive integer.`);
  }
  return parsed;
}

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "audit:read");
  if (!auth.ok) {
    return auth.response;
  }

  const { searchParams } = new URL(request.url);

  let query: PublicApiAuditQuery = {};
  try {
    query = {
      keyId: searchParams.get("keyId")?.trim() || undefined,
      scope: searchParams.get("scope")?.trim() || undefined,
      method: searchParams.get("method")?.trim() || undefined,
      pathContains: searchParams.get("pathContains")?.trim() || undefined,
      since: searchParams.get("since")?.trim() || undefined,
      until: searchParams.get("until")?.trim() || undefined,
      status: parsePositiveInt(searchParams.get("status"), "status"),
      limit: parsePositiveInt(searchParams.get("limit"), "limit"),
    };
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error: error instanceof Error ? error.message : "Invalid audit query.",
        },
        { status: 400 },
      ),
      "Invalid audit logs query.",
    );
  }

  try {
    const records = await queryPublicApiAuditLogs(query);
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ records, count: records.length }),
      "Audit logs returned.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error: error instanceof Error ? error.message : "Failed to query audit logs.",
        },
        { status: 500 },
      ),
      "Audit logs query failed.",
    );
  }
}
