import { NextResponse } from "next/server";
import { listFlakyJobs, upsertFlakyJobStatus } from "@/lib/pipelinex";

export const runtime = "nodejs";

interface FlakyUpdateBody {
  repo?: string;
  workflow?: string;
  job_name?: string;
  status?: "open" | "quarantined" | "resolved";
  owner?: string;
  notes?: string;
}

export async function GET() {
  try {
    const summary = await listFlakyJobs();
    return NextResponse.json({ summary });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to load flaky management summary.",
      },
      { status: 500 },
    );
  }
}

export async function POST(request: Request) {
  let body: FlakyUpdateBody = {};
  try {
    body = (await request.json()) as FlakyUpdateBody;
  } catch {
    return NextResponse.json(
      {
        error:
          "Invalid JSON body. Expected: { repo, workflow, job_name, status, owner?, notes? }.",
      },
      { status: 400 },
    );
  }

  if (!body.repo || !body.workflow || !body.job_name || !body.status) {
    return NextResponse.json(
      {
        error: "repo, workflow, job_name, and status are required.",
      },
      { status: 400 },
    );
  }

  try {
    const summary = await upsertFlakyJobStatus({
      repo: body.repo,
      workflow: body.workflow,
      job_name: body.job_name,
      status: body.status,
      owner: body.owner,
      notes: body.notes,
    });
    return NextResponse.json({ summary });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to update flaky job status.",
      },
      { status: 500 },
    );
  }
}
