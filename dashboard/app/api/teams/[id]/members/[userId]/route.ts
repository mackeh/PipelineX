import { NextRequest, NextResponse } from "next/server";
import { removeTeamMember, type Team } from "@/lib/pipelinex";

type TeamResponse = {
  team?: Team;
  error?: string;
};

type RouteContext = {
  params: Promise<{ id: string; userId: string }>;
};

/**
 * DELETE /api/teams/:id/members/:userId - Remove member from team
 */
export async function DELETE(
  _req: NextRequest,
  context: RouteContext
): Promise<NextResponse<TeamResponse>> {
  try {
    const { id, userId } = await context.params;
    const team = await removeTeamMember(id, userId);
    return NextResponse.json({ team });
  } catch (error) {
    console.error("Failed to remove team member:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to remove team member",
      },
      { status: 500 }
    );
  }
}
