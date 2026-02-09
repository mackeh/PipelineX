import { exec } from "child_process";
import { promisify } from "util";
import { NextRequest, NextResponse } from "next/server";
import { getRepoRoot, findPipelinexCommand, resolveRepoPath } from "@/lib/pipelinex";

const execAsync = promisify(exec);

type ApplyRequest = {
  pipelinePath: string;
  repo?: string;
  baseBranch?: string;
  noPr?: boolean;
};

type ApplyResponse = {
  success?: boolean;
  prUrl?: string;
  prNumber?: number;
  branch?: string;
  message?: string;
  error?: string;
};

export async function POST(req: NextRequest): Promise<NextResponse<ApplyResponse>> {
  try {
    const body: ApplyRequest = await req.json();
    const { pipelinePath, repo, baseBranch = "main", noPr = false } = body;

    if (!pipelinePath) {
      return NextResponse.json(
        { error: "Missing pipelinePath in request body" },
        { status: 400 }
      );
    }

    const repoRoot = await getRepoRoot();
    const absolutePath = await resolveRepoPath(pipelinePath);
    const commandPrefix = await findPipelinexCommand(repoRoot);

    // Build the command
    const args = [
      "apply",
      absolutePath,
      "--base",
      baseBranch,
    ];

    if (repo) {
      args.push("--repo", repo);
    }

    if (noPr) {
      args.push("--no-pr");
    }

    // Get GitHub token from environment
    const githubToken = process.env.GITHUB_TOKEN;
    if (!githubToken) {
      return NextResponse.json(
        { error: "GITHUB_TOKEN environment variable not set" },
        { status: 500 }
      );
    }

    const command = [...commandPrefix, ...args].map(s =>
      s.includes(" ") ? `"${s}"` : s
    ).join(" ");

    console.log(`Executing: ${command}`);

    // Execute the command
    const { stdout, stderr } = await execAsync(command, {
      cwd: repoRoot,
      env: {
        ...process.env,
        GITHUB_TOKEN: githubToken,
      },
      maxBuffer: 10 * 1024 * 1024, // 10MB
    });

    // Parse the output to extract PR URL and number
    const output = stdout + "\n" + stderr;
    const prUrlMatch = output.match(/🔗 (https:\/\/github\.com\/[^\s]+)/);
    const prNumberMatch = output.match(/PR #(\d+)/);
    const branchMatch = output.match(/Creating branch: ([^\s]+)/);

    return NextResponse.json({
      success: true,
      prUrl: prUrlMatch ? prUrlMatch[1] : undefined,
      prNumber: prNumberMatch ? parseInt(prNumberMatch[1], 10) : undefined,
      branch: branchMatch ? branchMatch[1] : undefined,
      message: noPr
        ? "Branch created and pushed successfully"
        : "Pull request created successfully",
    });

  } catch (error) {
    console.error("Apply command failed:", error);
    const errorMessage = error instanceof Error
      ? error.message
      : "Unknown error occurred";

    return NextResponse.json(
      { error: `Failed to apply optimization: ${errorMessage}` },
      { status: 500 }
    );
  }
}
