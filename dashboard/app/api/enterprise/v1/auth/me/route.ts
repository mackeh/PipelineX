import { NextResponse } from "next/server";
import { authenticateEnterpriseSessionRequest } from "@/lib/enterprise-auth";

export const runtime = "nodejs";

export async function GET(request: Request) {
  const auth = authenticateEnterpriseSessionRequest(request);
  if (!auth.ok) {
    return NextResponse.json({ error: auth.message }, { status: auth.status });
  }

  return NextResponse.json({ principal: auth.principal });
}
