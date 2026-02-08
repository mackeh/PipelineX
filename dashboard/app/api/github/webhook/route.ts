import { createHmac, timingSafeEqual } from "node:crypto";
import { NextResponse } from "next/server";
import { refreshHistorySnapshot } from "@/lib/pipelinex";

export const runtime = "nodejs";

interface WorkflowRunPayload {
  action?: string;
  repository?: {
    full_name?: string;
  };
  workflow?: {
    path?: string;
    name?: string;
  };
  workflow_run?: {
    id?: number;
    path?: string;
    name?: string;
    status?: string;
    conclusion?: string | null;
  };
}

function isValidGithubSignature(
  body: string,
  signatureHeader: string | null,
  secret: string,
): boolean {
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) {
    return false;
  }

  const computed = `sha256=${createHmac("sha256", secret).update(body).digest("hex")}`;
  const expectedBuffer = Buffer.from(computed);
  const receivedBuffer = Buffer.from(signatureHeader);

  if (expectedBuffer.length !== receivedBuffer.length) {
    return false;
  }

  return timingSafeEqual(expectedBuffer, receivedBuffer);
}

function parsePositiveInt(value: string | undefined, fallback: number): number {
  if (!value) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

export async function POST(request: Request) {
  const event = request.headers.get("x-github-event") || "unknown";
  const deliveryId = request.headers.get("x-github-delivery") || undefined;
  const signatureHeader = request.headers.get("x-hub-signature-256");
  const rawBody = await request.text();
  const webhookSecret = process.env.GITHUB_WEBHOOK_SECRET;

  if (
    webhookSecret &&
    !isValidGithubSignature(rawBody, signatureHeader, webhookSecret)
  ) {
    return NextResponse.json({ error: "Invalid webhook signature." }, { status: 401 });
  }

  let payload: WorkflowRunPayload = {};
  try {
    payload = JSON.parse(rawBody) as WorkflowRunPayload;
  } catch {
    return NextResponse.json({ error: "Invalid JSON payload." }, { status: 400 });
  }

  if (event === "ping") {
    return NextResponse.json({
      ok: true,
      event,
      message: "Ping received.",
    });
  }

  if (event !== "workflow_run") {
    return NextResponse.json({
      ok: true,
      event,
      ignored: true,
      reason: "Only workflow_run events trigger history refresh.",
    });
  }

  const action = payload.action || "unknown";
  if (action !== "completed") {
    return NextResponse.json({
      ok: true,
      event,
      action,
      ignored: true,
      reason: "Refresh runs on completed workflow_run actions.",
    });
  }

  const repo = payload.repository?.full_name;
  const workflow =
    payload.workflow?.path ||
    payload.workflow_run?.path ||
    payload.workflow?.name ||
    payload.workflow_run?.name;

  if (!repo || !workflow) {
    return NextResponse.json({
      ok: true,
      event,
      action,
      ignored: true,
      reason: "Missing repository full_name or workflow path/name in payload.",
    });
  }

  const runs = parsePositiveInt(process.env.PIPELINEX_HISTORY_RUNS, 100);

  try {
    const snapshot = await refreshHistorySnapshot({
      repo,
      workflow,
      runs,
      token: process.env.GITHUB_TOKEN,
      source: "webhook",
      deliveryId,
      workflowRunId: payload.workflow_run?.id,
    });

    return NextResponse.json({
      ok: true,
      event,
      action,
      refreshed: true,
      snapshot,
    });
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        event,
        action,
        refreshed: false,
        error:
          error instanceof Error
            ? error.message
            : "Failed to refresh workflow history from webhook.",
      },
      { status: 500 },
    );
  }
}
