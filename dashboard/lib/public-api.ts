import { timingSafeEqual } from "node:crypto";
import { NextResponse } from "next/server";

export type PublicApiScope = "benchmarks:read" | "benchmarks:write";

interface ApiKeyConfig {
  id: string;
  key: string;
  scopes: PublicApiScope[];
  rateLimitPerMinute: number;
}

interface RateLimitState {
  limit: number;
  remaining: number;
  resetEpochSeconds: number;
}

type AuthSuccess = {
  ok: true;
  principal: {
    id: string;
    scopes: PublicApiScope[];
  };
  rateLimit: RateLimitState;
};

type AuthFailure = {
  ok: false;
  response: NextResponse;
};

type AuthResult = AuthSuccess | AuthFailure;

interface RateBucket {
  windowStartMs: number;
  count: number;
}

const WINDOW_MS = 60_000;
const DEFAULT_RATE_LIMIT = 60;

function parseRateLimit(rawValue: string | undefined, fallback: number): number {
  if (!rawValue) {
    return fallback;
  }

  const parsed = Number.parseInt(rawValue, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return fallback;
  }

  return parsed;
}

function normalizeScopes(rawScopes: unknown): PublicApiScope[] {
  if (!Array.isArray(rawScopes)) {
    return [];
  }

  const allowed: PublicApiScope[] = ["benchmarks:read", "benchmarks:write"];
  return rawScopes.filter((scope): scope is PublicApiScope =>
    typeof scope === "string" ? allowed.includes(scope as PublicApiScope) : false,
  );
}

function parseConfiguredApiKeys(): ApiKeyConfig[] {
  const defaultLimit = parseRateLimit(
    process.env.PIPELINEX_API_RATE_LIMIT_PER_MINUTE,
    DEFAULT_RATE_LIMIT,
  );

  const directKey = process.env.PIPELINEX_API_KEY?.trim();
  const directScopes = normalizeScopes(
    (process.env.PIPELINEX_API_KEY_SCOPES || "benchmarks:read,benchmarks:write")
      .split(",")
      .map((value) => value.trim()),
  );

  const configured: ApiKeyConfig[] = [];

  if (directKey) {
    configured.push({
      id: "default",
      key: directKey,
      scopes: directScopes.length > 0 ? directScopes : ["benchmarks:read", "benchmarks:write"],
      rateLimitPerMinute: defaultLimit,
    });
  }

  const rawJson = process.env.PIPELINEX_API_KEYS;
  if (!rawJson) {
    return configured;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(rawJson);
  } catch {
    throw new Error("PIPELINEX_API_KEYS must be valid JSON.");
  }

  if (!Array.isArray(parsed)) {
    throw new Error("PIPELINEX_API_KEYS must be a JSON array.");
  }

  for (const item of parsed) {
    if (!item || typeof item !== "object") {
      continue;
    }

    const record = item as {
      id?: unknown;
      key?: unknown;
      scopes?: unknown;
      rateLimitPerMinute?: unknown;
    };
    const key = typeof record.key === "string" ? record.key.trim() : "";
    if (!key) {
      continue;
    }

    const id =
      typeof record.id === "string" && record.id.trim().length > 0
        ? record.id.trim()
        : `key-${configured.length + 1}`;
    const scopes = normalizeScopes(record.scopes);
    const rateLimitPerMinute =
      typeof record.rateLimitPerMinute === "number"
        ? parseRateLimit(String(record.rateLimitPerMinute), defaultLimit)
        : defaultLimit;

    configured.push({
      id,
      key,
      scopes: scopes.length > 0 ? scopes : ["benchmarks:read"],
      rateLimitPerMinute,
    });
  }

  return configured;
}

function extractApiKey(request: Request): string | null {
  const headerKey = request.headers.get("x-api-key")?.trim();
  if (headerKey) {
    return headerKey;
  }

  const authHeader = request.headers.get("authorization")?.trim() || "";
  if (authHeader.toLowerCase().startsWith("bearer ")) {
    const token = authHeader.slice(7).trim();
    return token || null;
  }

  return null;
}

function safeKeyMatch(expected: string, provided: string): boolean {
  const expectedBuffer = Buffer.from(expected);
  const providedBuffer = Buffer.from(provided);

  if (expectedBuffer.length !== providedBuffer.length) {
    return false;
  }

  return timingSafeEqual(expectedBuffer, providedBuffer);
}

function getRateBucketMap(): Map<string, RateBucket> {
  const globalState = globalThis as typeof globalThis & {
    __pipelinexPublicApiRateBuckets?: Map<string, RateBucket>;
  };

  if (!globalState.__pipelinexPublicApiRateBuckets) {
    globalState.__pipelinexPublicApiRateBuckets = new Map<string, RateBucket>();
  }

  return globalState.__pipelinexPublicApiRateBuckets;
}

function consumeRateLimit(keyId: string, limit: number): RateLimitState & { blocked: boolean } {
  const now = Date.now();
  const buckets = getRateBucketMap();
  const bucketKey = `public-api:${keyId}`;
  const existing = buckets.get(bucketKey);

  if (!existing || now - existing.windowStartMs >= WINDOW_MS) {
    const fresh: RateBucket = { windowStartMs: now, count: 1 };
    buckets.set(bucketKey, fresh);
    return {
      blocked: false,
      limit,
      remaining: Math.max(0, limit - 1),
      resetEpochSeconds: Math.floor((fresh.windowStartMs + WINDOW_MS) / 1000),
    };
  }

  if (existing.count >= limit) {
    return {
      blocked: true,
      limit,
      remaining: 0,
      resetEpochSeconds: Math.floor((existing.windowStartMs + WINDOW_MS) / 1000),
    };
  }

  existing.count += 1;
  buckets.set(bucketKey, existing);
  return {
    blocked: false,
    limit,
    remaining: Math.max(0, limit - existing.count),
    resetEpochSeconds: Math.floor((existing.windowStartMs + WINDOW_MS) / 1000),
  };
}

export function applyRateLimitHeaders(
  response: NextResponse,
  rateLimit: RateLimitState,
): NextResponse {
  response.headers.set("x-ratelimit-limit", String(rateLimit.limit));
  response.headers.set("x-ratelimit-remaining", String(rateLimit.remaining));
  response.headers.set("x-ratelimit-reset", String(rateLimit.resetEpochSeconds));
  return response;
}

function authError(status: number, message: string): NextResponse {
  const response = NextResponse.json({ error: message }, { status });
  if (status === 401) {
    response.headers.set("www-authenticate", 'Bearer realm="PipelineX Public API"');
  }
  return response;
}

export function authenticatePublicApiRequest(
  request: Request,
  requiredScope: PublicApiScope,
): AuthResult {
  let configuredKeys: ApiKeyConfig[];
  try {
    configuredKeys = parseConfiguredApiKeys();
  } catch (error) {
    return {
      ok: false,
      response: authError(
        500,
        error instanceof Error
          ? error.message
          : "Public API key configuration error.",
      ),
    };
  }

  if (configuredKeys.length === 0) {
    return {
      ok: false,
      response: authError(
        503,
        "Public API is not configured. Set PIPELINEX_API_KEY or PIPELINEX_API_KEYS.",
      ),
    };
  }

  const incomingKey = extractApiKey(request);
  if (!incomingKey) {
    return {
      ok: false,
      response: authError(401, "Missing API key. Use Authorization: Bearer <token>."),
    };
  }

  const matched = configuredKeys.find((config) => safeKeyMatch(config.key, incomingKey));
  if (!matched) {
    return {
      ok: false,
      response: authError(401, "Invalid API key."),
    };
  }

  if (!matched.scopes.includes(requiredScope)) {
    return {
      ok: false,
      response: authError(
        403,
        `Insufficient scope. Required scope: ${requiredScope}.`,
      ),
    };
  }

  const rateLimit = consumeRateLimit(matched.id, matched.rateLimitPerMinute);
  if (rateLimit.blocked) {
    const response = NextResponse.json(
      { error: "Rate limit exceeded. Please retry after reset." },
      { status: 429 },
    );
    applyRateLimitHeaders(response, rateLimit);
    return { ok: false, response };
  }

  return {
    ok: true,
    principal: {
      id: matched.id,
      scopes: matched.scopes,
    },
    rateLimit,
  };
}
