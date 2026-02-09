import { timingSafeEqual } from "node:crypto";
import { NextResponse } from "next/server";
import { storeHistorySnapshot, type PipelineStatistics } from "@/lib/pipelinex";

export const runtime = "nodejs";

interface GitLabPipelinePayload {
  object_kind?: string;
  event_type?: string;
  project?: {
    path_with_namespace?: string;
  };
  object_attributes?: {
    id?: number;
    status?: string;
    duration?: number | null;
    ref?: string;
    name?: string;
  };
}

function secureTokenEqual(expected: string, received: string | null): boolean {
  if (!received) {
    return false;
  }

  const expectedBuffer = Buffer.from(expected);
  const receivedBuffer = Buffer.from(received);

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

function isCompletedStatus(status: string): boolean {
  return ["success", "failed", "canceled", "skipped", "manual"].includes(status);
}

function normalizeDuration(value: number | null | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return 0;
  }
  return value;
}

function buildPipelineStats(
  workflow: string,
  status: string,
  durationSecs: number,
): PipelineStatistics {
  const successRate = status === "success" ? 1 : 0;
  return {
    workflow_name: workflow,
    total_runs: 1,
    success_rate: successRate,
    avg_duration_sec: durationSecs,
    p50_duration_sec: durationSecs,
    p90_duration_sec: durationSecs,
    p99_duration_sec: durationSecs,
    job_timings: [],
    flaky_jobs: [],
  };
}

export async function POST(request: Request) {
  const event = request.headers.get("x-gitlab-event") || "unknown";
  const deliveryId =
    request.headers.get("x-gitlab-event-uuid") || request.headers.get("x-request-id") || undefined;
  const tokenHeader = request.headers.get("x-gitlab-token");
  const rawBody = await request.text();
  const webhookToken =
    process.env.GITLAB_WEBHOOK_TOKEN?.trim() ||
    process.env.GITLAB_WEBHOOK_SECRET_TOKEN?.trim() ||
    "";

  if (webhookToken.length > 0 && !secureTokenEqual(webhookToken, tokenHeader)) {
    return NextResponse.json({ error: "Invalid webhook token." }, { status: 401 });
  }

  let payload: GitLabPipelinePayload = {};
  try {
    payload = JSON.parse(rawBody) as GitLabPipelinePayload;
  } catch {
    return NextResponse.json({ error: "Invalid JSON payload." }, { status: 400 });
  }

  if (event === "Ping Hook" || payload.object_kind === "ping") {
    return NextResponse.json({
      ok: true,
      event,
      message: "Ping received.",
    });
  }

  if (payload.object_kind !== "pipeline") {
    return NextResponse.json({
      ok: true,
      event,
      ignored: true,
      reason: "Only pipeline hook events trigger snapshot refresh.",
    });
  }

  const status = payload.object_attributes?.status?.toLowerCase() || "unknown";
  if (!isCompletedStatus(status)) {
    return NextResponse.json({
      ok: true,
      event,
      status,
      ignored: true,
      reason: "Refresh runs only after pipeline completion states.",
    });
  }

  const repo = payload.project?.path_with_namespace;
  const workflow = process.env.PIPELINEX_GITLAB_WORKFLOW_PATH?.trim() || ".gitlab-ci.yml";

  if (!repo) {
    return NextResponse.json({
      ok: true,
      event,
      status,
      ignored: true,
      reason: "Missing project.path_with_namespace in payload.",
    });
  }

  const durationSecs = normalizeDuration(payload.object_attributes?.duration);
  const stats = buildPipelineStats(workflow, status, durationSecs);
  const runs = parsePositiveInt(process.env.PIPELINEX_HISTORY_RUNS, 1);

  try {
    const snapshot = await storeHistorySnapshot({
      repo,
      workflow,
      runs,
      source: "webhook",
      stats,
      deliveryId,
      workflowRunId: payload.object_attributes?.id,
    });

    return NextResponse.json({
      ok: true,
      event,
      status,
      ref: payload.object_attributes?.ref || undefined,
      pipeline: payload.object_attributes?.name || undefined,
      refreshed: true,
      snapshot,
    });
  } catch (error) {
    return NextResponse.json(
      {
        ok: false,
        event,
        status,
        refreshed: false,
        error:
          error instanceof Error
            ? error.message
            : "Failed to refresh workflow history from GitLab webhook.",
      },
      { status: 500 },
    );
  }
}
