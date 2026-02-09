import { NextResponse } from "next/server";
import {
  deliverWeeklyDigest,
  generateWeeklyDigest,
  type WeeklyDigestDeliveryOptions,
} from "@/lib/pipelinex";

export const runtime = "nodejs";

interface DigestQueryOptions {
  windowDays?: number;
  runsPerMonth?: number;
  developerHourlyRate?: number;
}

interface DigestRequestBody extends DigestQueryOptions {
  deliver?: boolean;
  channels?: WeeklyDigestDeliveryOptions;
}

function parsePositiveNumber(
  raw: string | number | undefined,
  field: string,
): number | undefined {
  if (raw === undefined) {
    return undefined;
  }
  const value = typeof raw === "number" ? raw : Number.parseFloat(raw);
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${field} must be a positive number.`);
  }
  return value;
}

function parseEmailRecipients(raw: unknown): string[] | undefined {
  if (raw === undefined) {
    return undefined;
  }
  if (!Array.isArray(raw)) {
    throw new Error("channels.emailRecipients must be an array of strings.");
  }
  const recipients = raw
    .filter((value): value is string => typeof value === "string")
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
  return recipients.length > 0 ? recipients : [];
}

export async function GET(request: Request) {
  const { searchParams } = new URL(request.url);

  let windowDays: number | undefined;
  let runsPerMonth: number | undefined;
  let developerHourlyRate: number | undefined;

  try {
    windowDays = parsePositiveNumber(searchParams.get("windowDays") || undefined, "windowDays");
    runsPerMonth = parsePositiveNumber(
      searchParams.get("runsPerMonth") || undefined,
      "runsPerMonth",
    );
    developerHourlyRate = parsePositiveNumber(
      searchParams.get("developerHourlyRate") || undefined,
      "developerHourlyRate",
    );
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Invalid query params.",
      },
      { status: 400 },
    );
  }

  try {
    const summary = await generateWeeklyDigest({
      windowDays,
      runsPerMonth,
      developerHourlyRate,
    });
    return NextResponse.json({ summary });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to generate weekly digest summary.",
      },
      { status: 500 },
    );
  }
}

export async function POST(request: Request) {
  let body: DigestRequestBody = {};

  try {
    body = (await request.json()) as DigestRequestBody;
  } catch {
    return NextResponse.json(
      {
        error:
          "Invalid JSON body. Expected optional digest options and { deliver?: boolean, channels?: { ... } }.",
      },
      { status: 400 },
    );
  }

  let windowDays: number | undefined;
  let runsPerMonth: number | undefined;
  let developerHourlyRate: number | undefined;
  let channels: WeeklyDigestDeliveryOptions | undefined;

  try {
    windowDays = parsePositiveNumber(body.windowDays, "windowDays");
    runsPerMonth = parsePositiveNumber(body.runsPerMonth, "runsPerMonth");
    developerHourlyRate = parsePositiveNumber(
      body.developerHourlyRate,
      "developerHourlyRate",
    );

    if (body.channels) {
      channels = {
        slackWebhookUrl:
          typeof body.channels.slackWebhookUrl === "string"
            ? body.channels.slackWebhookUrl
            : undefined,
        teamsWebhookUrl:
          typeof body.channels.teamsWebhookUrl === "string"
            ? body.channels.teamsWebhookUrl
            : undefined,
        emailRecipients: parseEmailRecipients(body.channels.emailRecipients),
        emailOutboxPath:
          typeof body.channels.emailOutboxPath === "string"
            ? body.channels.emailOutboxPath
            : undefined,
        dryRun: body.channels.dryRun === true,
      };
    }
  } catch (error) {
    return NextResponse.json(
      {
        error: error instanceof Error ? error.message : "Invalid digest payload.",
      },
      { status: 400 },
    );
  }

  try {
    const summary = await generateWeeklyDigest({
      windowDays,
      runsPerMonth,
      developerHourlyRate,
    });

    if (!body.deliver) {
      return NextResponse.json({ summary });
    }

    const delivery = await deliverWeeklyDigest(summary, channels);
    return NextResponse.json({ summary, delivery });
  } catch (error) {
    return NextResponse.json(
      {
        error:
          error instanceof Error
            ? error.message
            : "Failed to generate or deliver weekly digest.",
      },
      { status: 500 },
    );
  }
}
