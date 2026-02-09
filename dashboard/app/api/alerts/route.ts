import { NextResponse } from "next/server";
import {
  deleteAlertRule,
  listAlertRules,
  upsertAlertRule,
  type AlertMetric,
  type AlertOperator,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

interface AlertRuleBody {
  id?: string;
  name?: string;
  enabled?: boolean;
  metric?: AlertMetric;
  operator?: AlertOperator;
  threshold?: number;
  repo?: string;
  workflow?: string;
  provider?: string;
}

export async function GET() {
  try {
    const rules = await listAlertRules();
    return NextResponse.json({ rules });
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Failed to list alert rules.",
      },
      { status: 500 },
    );
  }
}

export async function POST(request: Request) {
  let body: AlertRuleBody = {};
  try {
    body = (await request.json()) as AlertRuleBody;
  } catch {
    return NextResponse.json(
      { error: "Invalid JSON body for alert rule." },
      { status: 400 },
    );
  }

  if (!body.name || !body.metric || !body.operator || typeof body.threshold !== "number") {
    return NextResponse.json(
      {
        error:
          "name, metric, operator, and numeric threshold are required to create/update an alert rule.",
      },
      { status: 400 },
    );
  }

  try {
    const rule = await upsertAlertRule({
      id: body.id,
      name: body.name,
      enabled: body.enabled,
      metric: body.metric,
      operator: body.operator,
      threshold: body.threshold,
      repo: body.repo,
      workflow: body.workflow,
      provider: body.provider,
    });
    return NextResponse.json({ rule }, { status: body.id ? 200 : 201 });
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Failed to upsert alert rule.",
      },
      { status: 400 },
    );
  }
}

export async function DELETE(request: Request) {
  const { searchParams } = new URL(request.url);
  const id = searchParams.get("id")?.trim() || "";
  if (!id) {
    return NextResponse.json(
      { error: "Alert rule id query parameter is required." },
      { status: 400 },
    );
  }

  try {
    const deleted = await deleteAlertRule(id);
    if (!deleted) {
      return NextResponse.json(
        { error: "Alert rule not found." },
        { status: 404 },
      );
    }
    return NextResponse.json({ deleted: true });
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Failed to delete alert rule.",
      },
      { status: 500 },
    );
  }
}
