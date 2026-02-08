import { NextResponse } from "next/server";
import {
  exchangeEnterpriseSsoAssertion,
  type EnterpriseSsoAssertion,
} from "@/lib/enterprise-auth";

export const runtime = "nodejs";

export async function POST(request: Request) {
  let body: EnterpriseSsoAssertion;

  try {
    body = (await request.json()) as EnterpriseSsoAssertion;
  } catch {
    return NextResponse.json(
      {
        error:
          "Invalid JSON body. Expected: { subject, issuedAt, expiresAt, signature, roles?, scopes?, email?, org?, nonce? }",
      },
      { status: 400 },
    );
  }

  const exchanged = exchangeEnterpriseSsoAssertion(body);
  if (!exchanged.ok) {
    return NextResponse.json({ error: exchanged.message }, { status: exchanged.status });
  }

  return NextResponse.json(
    {
      token: exchanged.token,
      tokenType: "Bearer",
      principal: exchanged.principal,
    },
    { status: 201 },
  );
}
