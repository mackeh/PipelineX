import { NextResponse } from "next/server";
import {
  listHistorySnapshots,
  readHistorySnapshot,
  refreshHistorySnapshot,
} from "@/lib/pipelinex";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

interface RefreshBody {
  repo?: string;
  workflow?: string;
  runs?: number;
}

function parseRuns(value: unknown): number | undefined {
  if (typeof value !== "number") {
    return undefined;
  }
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error("runs must be a positive number when provided.");
  }
  return Math.floor(value);
}

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "history:read");
  if (!auth.ok) {
    return auth.response;
  }

  const { searchParams } = new URL(request.url);
  const repo = searchParams.get("repo");
  const workflow = searchParams.get("workflow");

  try {
    if (!repo && !workflow) {
      const snapshots = await listHistorySnapshots();
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json({ snapshots }),
        "History snapshots returned.",
      );
    }

    if (!repo || !workflow) {
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json(
          { error: "Provide both repo and workflow query params, or neither." },
          { status: 400 },
        ),
        "Invalid history query parameters.",
      );
    }

    const snapshot = await readHistorySnapshot(repo, workflow);
    if (!snapshot) {
      return finalizePublicApiResponse(
        request,
        auth,
        NextResponse.json(
          { error: "No cached history snapshot found for repo/workflow." },
          { status: 404 },
        ),
        "No history snapshot found.",
      );
    }

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ snapshot }),
      "History snapshot returned.",
    );
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error:
            error instanceof Error ? error.message : "Failed to fetch history snapshots.",
        },
        { status: 500 },
      ),
      "History lookup failed.",
    );
  }
}

export async function POST(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "history:write");
  if (!auth.ok) {
    return auth.response;
  }

  let body: RefreshBody = {};

  try {
    body = (await request.json()) as RefreshBody;
  } catch {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error: "Invalid JSON body. Expected: { repo: string, workflow: string, runs?: number }",
        },
        { status: 400 },
      ),
      "Invalid history refresh payload.",
    );
  }

  if (!body.repo || !body.workflow) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ error: "repo and workflow are required." }, { status: 400 }),
      "Missing repo/workflow for history refresh.",
    );
  }

  let runs: number | undefined;
  try {
    runs = parseRuns(body.runs);
  } catch (error) {
    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json(
        {
          error: error instanceof Error ? error.message : "Invalid runs value.",
        },
        { status: 400 },
      ),
      "Invalid runs value for history refresh.",
    );
  }

  try {
    const snapshot = await refreshHistorySnapshot({
      repo: body.repo,
      workflow: body.workflow,
      runs,
      token: process.env.GITHUB_TOKEN,
      source: "manual",
    });

    return finalizePublicApiResponse(
      request,
      auth,
      NextResponse.json({ snapshot }),
      "History refreshed through public API.",
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
              : "Failed to refresh workflow history.",
        },
        { status: 500 },
      ),
      "History refresh failed.",
    );
  }
}
