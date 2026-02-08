import { NextResponse } from "next/server";
import {
  listHistorySnapshots,
  readHistorySnapshot,
  refreshHistorySnapshot,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

interface RefreshBody {
  repo?: string;
  workflow?: string;
  runs?: number;
}

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);
  const repo = searchParams.get("repo");
  const workflow = searchParams.get("workflow");

  try {
    if (!repo && !workflow) {
      const snapshots = await listHistorySnapshots();
      return NextResponse.json({ snapshots });
    }

    if (!repo || !workflow) {
      return NextResponse.json(
        { error: "Provide both repo and workflow query params, or neither." },
        { status: 400 },
      );
    }

    const snapshot = await readHistorySnapshot(repo, workflow);
    if (!snapshot) {
      return NextResponse.json(
        { error: "No cached history snapshot found for repo/workflow." },
        { status: 404 },
      );
    }

    return NextResponse.json({ snapshot });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to fetch history snapshots.",
      },
      { status: 500 },
    );
  }
}

export async function POST(request: Request) {
  let body: RefreshBody = {};

  try {
    body = (await request.json()) as RefreshBody;
  } catch {
    return NextResponse.json(
      {
        error: "Invalid JSON body. Expected: { repo: string, workflow: string, runs?: number }",
      },
      { status: 400 },
    );
  }

  if (!body.repo || !body.workflow) {
    return NextResponse.json(
      { error: "repo and workflow are required." },
      { status: 400 },
    );
  }

  try {
    const snapshot = await refreshHistorySnapshot({
      repo: body.repo,
      workflow: body.workflow,
      runs: body.runs,
      token: process.env.GITHUB_TOKEN,
      source: "manual",
    });

    return NextResponse.json({ snapshot });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to refresh workflow history.",
      },
      { status: 500 },
    );
  }
}
