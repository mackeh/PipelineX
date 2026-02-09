import { NextRequest, NextResponse } from "next/server";
import {
  getTeam,
  updateTeam,
  deleteTeam,
  type Team,
  type TeamUpdateInput,
} from "@/lib/pipelinex";

type TeamResponse = {
  team?: Team;
  error?: string;
};

type DeleteResponse = {
  success?: boolean;
  error?: string;
};

type RouteContext = {
  params: Promise<{ id: string }>;
};

/**
 * GET /api/teams/:id - Get team details
 */
export async function GET(
  _req: NextRequest,
  context: RouteContext
): Promise<NextResponse<TeamResponse>> {
  try {
    const { id } = await context.params;
    const team = await getTeam(id);

    if (!team) {
      return NextResponse.json(
        { error: "Team not found" },
        { status: 404 }
      );
    }

    return NextResponse.json({ team });
  } catch (error) {
    console.error("Failed to get team:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error ? error.message : "Failed to get team",
      },
      { status: 500 }
    );
  }
}

/**
 * PUT /api/teams/:id - Update team
 */
export async function PUT(
  req: NextRequest,
  context: RouteContext
): Promise<NextResponse<TeamResponse>> {
  try {
    const { id } = await context.params;
    const input: TeamUpdateInput = await req.json();

    const team = await updateTeam(id, input);
    return NextResponse.json({ team });
  } catch (error) {
    console.error("Failed to update team:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to update team",
      },
      { status: 500 }
    );
  }
}

/**
 * DELETE /api/teams/:id - Delete team
 */
export async function DELETE(
  _req: NextRequest,
  context: RouteContext
): Promise<NextResponse<DeleteResponse>> {
  try {
    const { id } = await context.params;
    const success = await deleteTeam(id);

    if (!success) {
      return NextResponse.json(
        { error: "Team not found" },
        { status: 404 }
      );
    }

    return NextResponse.json({ success: true });
  } catch (error) {
    console.error("Failed to delete team:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to delete team",
      },
      { status: 500 }
    );
  }
}
