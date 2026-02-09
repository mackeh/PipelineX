import { NextRequest, NextResponse } from "next/server";
import {
  addTeamMember,
  type Team,
  type AddTeamMemberInput,
} from "@/lib/pipelinex";

type TeamResponse = {
  team?: Team;
  error?: string;
};

type RouteContext = {
  params: Promise<{ id: string }>;
};

/**
 * POST /api/teams/:id/members - Add member to team
 */
export async function POST(
  req: NextRequest,
  context: RouteContext
): Promise<NextResponse<TeamResponse>> {
  try {
    const { id } = await context.params;
    const input: AddTeamMemberInput = await req.json();

    if (!input.user_id?.trim() || !input.email?.trim()) {
      return NextResponse.json(
        { error: "user_id and email are required" },
        { status: 400 }
      );
    }

    if (!["admin", "member", "viewer"].includes(input.role)) {
      return NextResponse.json(
        { error: "Invalid role. Must be admin, member, or viewer" },
        { status: 400 }
      );
    }

    const team = await addTeamMember(id, input);
    return NextResponse.json({ team });
  } catch (error) {
    console.error("Failed to add team member:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to add team member",
      },
      { status: 500 }
    );
  }
}
