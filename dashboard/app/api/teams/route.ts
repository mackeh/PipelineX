import { NextRequest, NextResponse } from "next/server";
import {
  listTeams,
  createTeam,
  type Team,
  type TeamCreateInput,
} from "@/lib/pipelinex";

type TeamsListResponse = {
  teams?: Team[];
  error?: string;
};

type TeamCreateResponse = {
  team?: Team;
  error?: string;
};

/**
 * GET /api/teams - List all teams
 */
export async function GET(): Promise<NextResponse<TeamsListResponse>> {
  try {
    const teams = await listTeams();
    return NextResponse.json({ teams });
  } catch (error) {
    console.error("Failed to list teams:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to list teams",
      },
      { status: 500 }
    );
  }
}

/**
 * POST /api/teams - Create a new team
 */
export async function POST(
  req: NextRequest
): Promise<NextResponse<TeamCreateResponse>> {
  try {
    const input: TeamCreateInput = await req.json();

    if (!input.name?.trim()) {
      return NextResponse.json(
        { error: "Team name is required" },
        { status: 400 }
      );
    }

    const team = await createTeam(input);
    return NextResponse.json({ team }, { status: 201 });
  } catch (error) {
    console.error("Failed to create team:", error);
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to create team",
      },
      { status: 500 }
    );
  }
}
