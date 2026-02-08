import { NextResponse } from "next/server";
import {
  authenticatePublicApiRequest,
  finalizePublicApiResponse,
} from "@/lib/public-api";

export const runtime = "nodejs";

const OPENAPI_SPEC = {
  openapi: "3.1.0",
  info: {
    title: "PipelineX Public API",
    version: "v1",
    description: "Public REST API for custom integrations and automation.",
  },
  servers: [{ url: "/api/public/v1" }],
  security: [{ bearerAuth: [] }],
  components: {
    securitySchemes: {
      bearerAuth: {
        type: "http",
        scheme: "bearer",
        bearerFormat: "API key or enterprise session token",
      },
    },
  },
  paths: {
    "/auth/me": { get: { summary: "Return principal metadata" } },
    "/workflows": { get: { summary: "List discovered workflow files" } },
    "/analyze": { post: { summary: "Analyze a pipeline file" } },
    "/history": {
      get: { summary: "List or fetch cached history snapshots" },
      post: { summary: "Refresh workflow history snapshot" },
    },
    "/impact/stats": { get: { summary: "Query optimization impact savings metrics" } },
    "/impact/track": { post: { summary: "Track optimization impact event" } },
    "/benchmarks/stats": { get: { summary: "Get benchmark cohort stats" } },
    "/benchmarks/submit": { post: { summary: "Submit benchmark report" } },
    "/audit/logs": { get: { summary: "Query public API audit logs" } },
  },
};

export async function GET(request: Request) {
  const auth = await authenticatePublicApiRequest(request, "workflows:read");
  if (!auth.ok) {
    return auth.response;
  }

  return finalizePublicApiResponse(
    request,
    auth,
    NextResponse.json(OPENAPI_SPEC),
    "OpenAPI descriptor returned.",
  );
}
