import { randomUUID, timingSafeEqual } from "node:crypto";
import { constants } from "node:fs";
import { appendFile, access, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { NextResponse } from "next/server";

export type PublicApiScope = "benchmarks:read" | "benchmarks:write" | "audit:read";

export type PublicApiRole = "admin" | "analyst" | "ingest" | "viewer" | "auditor";

interface ApiKeyConfig {
  id: string;
  key: string;
  scopes: PublicApiScope[];
  roles: PublicApiRole[];
  rateLimitPerMinute: number;
  description?: string;
  notBefore?: string;
  expiresAt?: string;
  disabled?: boolean;
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
    roles: PublicApiRole[];
  };
  rateLimit: RateLimitState;
  requiredScope: PublicApiScope;
  requestId: string;
  clientIp: string;
  userAgent: string;
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

interface RateLimitStore {
  version: 1;
  updatedAt: string;
  buckets: Record<string, RateBucket>;
}

export interface PublicApiAuditRecord {
  timestamp: string;
  requestId: string;
  keyId: string;
  scope: string;
  method: string;
  path: string;
  status: number;
  clientIp: string;
  userAgent: string;
  message?: string;
  rateLimit?: {
    limit: number;
    remaining: number;
    resetEpochSeconds: number;
  };
}

export interface PublicApiAuditQuery {
  keyId?: string;
  scope?: string;
  method?: string;
  pathContains?: string;
  status?: number;
  since?: string;
  until?: string;
  limit?: number;
}

const WINDOW_MS = 60_000;
const DEFAULT_RATE_LIMIT = 60;
const DEFAULT_AUDIT_QUERY_LIMIT = 100;
const MAX_AUDIT_QUERY_LIMIT = 1000;
const RATE_LIMIT_STORE_RELATIVE_PATH = ".pipelinex/public-api-rate-limits.json";
const AUDIT_LOG_RELATIVE_PATH = ".pipelinex/public-api-audit.log";

const ROLE_SCOPES: Record<PublicApiRole, PublicApiScope[]> = {
  admin: ["benchmarks:read", "benchmarks:write", "audit:read"],
  analyst: ["benchmarks:read", "audit:read"],
  ingest: ["benchmarks:write"],
  viewer: ["benchmarks:read"],
  auditor: ["audit:read"],
};

type LockFn<T> = () => Promise<T>;
let rateLimitLock: Promise<void> = Promise.resolve();

function withRateLimitLock<T>(fn: LockFn<T>): Promise<T> {
  const run = rateLimitLock.then(fn, fn);
  rateLimitLock = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}

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

function dedupeScopes(scopes: PublicApiScope[]): PublicApiScope[] {
  return [...new Set(scopes)];
}

function dedupeRoles(roles: PublicApiRole[]): PublicApiRole[] {
  return [...new Set(roles)];
}

function normalizeScopes(rawScopes: unknown): PublicApiScope[] {
  if (!Array.isArray(rawScopes)) {
    return [];
  }

  const allowed: PublicApiScope[] = ["benchmarks:read", "benchmarks:write", "audit:read"];
  return rawScopes.filter((scope): scope is PublicApiScope =>
    typeof scope === "string" ? allowed.includes(scope as PublicApiScope) : false,
  );
}

function normalizeScopeList(raw: string | undefined): PublicApiScope[] {
  if (!raw) {
    return [];
  }
  return normalizeScopes(
    raw
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0),
  );
}

function normalizeRoles(rawRoles: unknown): PublicApiRole[] {
  if (!Array.isArray(rawRoles)) {
    return [];
  }

  const allowed: PublicApiRole[] = ["admin", "analyst", "ingest", "viewer", "auditor"];
  return rawRoles.filter((role): role is PublicApiRole =>
    typeof role === "string" ? allowed.includes(role as PublicApiRole) : false,
  );
}

function normalizeRoleList(raw: string | undefined): PublicApiRole[] {
  if (!raw) {
    return [];
  }
  return normalizeRoles(
    raw
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0),
  );
}

function scopesForRoles(roles: PublicApiRole[]): PublicApiScope[] {
  const scopes: PublicApiScope[] = [];
  for (const role of roles) {
    scopes.push(...ROLE_SCOPES[role]);
  }
  return dedupeScopes(scopes);
}

function mergeScopesWithRoles(
  scopes: PublicApiScope[],
  roles: PublicApiRole[],
  fallbackScopes: PublicApiScope[],
): PublicApiScope[] {
  const merged = dedupeScopes([...scopes, ...scopesForRoles(roles)]);
  return merged.length > 0 ? merged : fallbackScopes;
}

function parseIsoTimestamp(value: unknown): string | undefined {
  if (typeof value !== "string" || value.trim().length === 0) {
    return undefined;
  }
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) {
    return undefined;
  }
  return new Date(parsed).toISOString();
}

function keyIsActive(config: ApiKeyConfig, nowMs: number): boolean {
  if (config.disabled) {
    return false;
  }
  if (config.notBefore) {
    const notBeforeMs = Date.parse(config.notBefore);
    if (Number.isFinite(notBeforeMs) && nowMs < notBeforeMs) {
      return false;
    }
  }
  if (config.expiresAt) {
    const expiresMs = Date.parse(config.expiresAt);
    if (Number.isFinite(expiresMs) && nowMs >= expiresMs) {
      return false;
    }
  }
  return true;
}

async function pathExists(filePath: string): Promise<boolean> {
  try {
    await access(filePath, constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function getRepoRoot(): Promise<string> {
  const cwd = process.cwd();
  const cwdHasCargo = await pathExists(path.join(cwd, "Cargo.toml"));
  if (cwdHasCargo) {
    return cwd;
  }

  const parent = path.resolve(cwd, "..");
  const parentHasCargo = await pathExists(path.join(parent, "Cargo.toml"));
  if (parentHasCargo) {
    return parent;
  }

  return cwd;
}

async function resolveStorePath(envKey: string, defaultRelativePath: string): Promise<string> {
  const repoRoot = await getRepoRoot();
  const configured = process.env[envKey]?.trim();
  if (!configured) {
    return path.join(repoRoot, defaultRelativePath);
  }
  return path.isAbsolute(configured) ? configured : path.join(repoRoot, configured);
}

async function readConfiguredApiKeysFile(): Promise<ApiKeyConfig[]> {
  const configuredPath = process.env.PIPELINEX_API_KEYS_FILE?.trim();
  if (!configuredPath) {
    return [];
  }

  const repoRoot = await getRepoRoot();
  const filePath = path.isAbsolute(configuredPath)
    ? configuredPath
    : path.join(repoRoot, configuredPath);

  if (!(await pathExists(filePath))) {
    throw new Error(`PIPELINEX_API_KEYS_FILE does not exist: ${filePath}`);
  }

  const raw = await readFile(filePath, "utf8");
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error("PIPELINEX_API_KEYS_FILE must contain valid JSON.");
  }

  if (!Array.isArray(parsed)) {
    throw new Error("PIPELINEX_API_KEYS_FILE must contain a JSON array.");
  }

  return normalizeApiKeyRecords(parsed, "file");
}

function normalizeApiKeyRecords(records: unknown[], source: string): ApiKeyConfig[] {
  const normalized: ApiKeyConfig[] = [];

  for (const item of records) {
    if (!item || typeof item !== "object") {
      continue;
    }

    const record = item as {
      id?: unknown;
      key?: unknown;
      scopes?: unknown;
      roles?: unknown;
      rateLimitPerMinute?: unknown;
      description?: unknown;
      notBefore?: unknown;
      expiresAt?: unknown;
      disabled?: unknown;
    };

    const key = typeof record.key === "string" ? record.key.trim() : "";
    if (!key) {
      continue;
    }

    const id =
      typeof record.id === "string" && record.id.trim().length > 0
        ? record.id.trim()
        : `${source}-key-${normalized.length + 1}`;

    const roles = dedupeRoles(normalizeRoles(record.roles));
    const explicitScopes = normalizeScopes(record.scopes);
    const scopes = mergeScopesWithRoles(explicitScopes, roles, ["benchmarks:read"]);
    const rateLimitPerMinute =
      typeof record.rateLimitPerMinute === "number"
        ? parseRateLimit(String(record.rateLimitPerMinute), DEFAULT_RATE_LIMIT)
        : DEFAULT_RATE_LIMIT;

    normalized.push({
      id,
      key,
      scopes,
      roles,
      rateLimitPerMinute,
      description:
        typeof record.description === "string" ? record.description.trim() : undefined,
      notBefore: parseIsoTimestamp(record.notBefore),
      expiresAt: parseIsoTimestamp(record.expiresAt),
      disabled: Boolean(record.disabled),
    });
  }

  return normalized;
}

async function parseConfiguredApiKeys(): Promise<ApiKeyConfig[]> {
  const defaultLimit = parseRateLimit(
    process.env.PIPELINEX_API_RATE_LIMIT_PER_MINUTE,
    DEFAULT_RATE_LIMIT,
  );
  const configured: ApiKeyConfig[] = [];

  const directKey = process.env.PIPELINEX_API_KEY?.trim();
  const directRoles = dedupeRoles(normalizeRoleList(process.env.PIPELINEX_API_KEY_ROLES));
  const directScopes = normalizeScopeList(process.env.PIPELINEX_API_KEY_SCOPES);

  if (directKey) {
    configured.push({
      id: process.env.PIPELINEX_API_KEY_ID?.trim() || "default",
      key: directKey,
      scopes: mergeScopesWithRoles(
        directScopes,
        directRoles,
        ["benchmarks:read", "benchmarks:write"],
      ),
      roles: directRoles,
      rateLimitPerMinute: defaultLimit,
      notBefore: parseIsoTimestamp(process.env.PIPELINEX_API_KEY_NOT_BEFORE),
      expiresAt: parseIsoTimestamp(process.env.PIPELINEX_API_KEY_EXPIRES_AT),
      disabled: false,
    });
  }

  const envJson = process.env.PIPELINEX_API_KEYS;
  if (envJson) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(envJson);
    } catch {
      throw new Error("PIPELINEX_API_KEYS must be valid JSON.");
    }
    if (!Array.isArray(parsed)) {
      throw new Error("PIPELINEX_API_KEYS must be a JSON array.");
    }
    configured.push(...normalizeApiKeyRecords(parsed, "env"));
  }

  configured.push(...(await readConfiguredApiKeysFile()));

  const nowMs = Date.now();
  return configured
    .map((key) => ({
      ...key,
      rateLimitPerMinute: parseRateLimit(String(key.rateLimitPerMinute), defaultLimit),
      scopes: dedupeScopes(key.scopes),
      roles: dedupeRoles(key.roles),
    }))
    .filter((key) => keyIsActive(key, nowMs));
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

function parseClientIp(request: Request): string {
  const forwardedFor = request.headers.get("x-forwarded-for");
  if (forwardedFor) {
    return forwardedFor.split(",")[0].trim();
  }
  return request.headers.get("x-real-ip") || "unknown";
}

function parseRequestId(request: Request): string {
  return request.headers.get("x-request-id")?.trim() || randomUUID();
}

async function readRateLimitStore(filePath: string): Promise<RateLimitStore> {
  if (!(await pathExists(filePath))) {
    return { version: 1, updatedAt: new Date().toISOString(), buckets: {} };
  }

  try {
    const raw = await readFile(filePath, "utf8");
    const parsed = JSON.parse(raw) as RateLimitStore;
    if (!parsed || typeof parsed !== "object" || typeof parsed.buckets !== "object") {
      throw new Error("Invalid rate limit store shape.");
    }
    return {
      version: 1,
      updatedAt:
        typeof parsed.updatedAt === "string" ? parsed.updatedAt : new Date().toISOString(),
      buckets: parsed.buckets || {},
    };
  } catch {
    return { version: 1, updatedAt: new Date().toISOString(), buckets: {} };
  }
}

async function writeRateLimitStore(filePath: string, store: RateLimitStore): Promise<void> {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, JSON.stringify(store, null, 2), "utf8");
}

async function consumeRateLimit(
  keyId: string,
  clientIp: string,
  limit: number,
): Promise<RateLimitState & { blocked: boolean }> {
  const storePath = await resolveStorePath(
    "PIPELINEX_PUBLIC_API_RATE_LIMIT_FILE",
    RATE_LIMIT_STORE_RELATIVE_PATH,
  );

  return withRateLimitLock(async () => {
    const now = Date.now();
    const store = await readRateLimitStore(storePath);
    const bucketKey = `${keyId}:${clientIp}`;
    const existing = store.buckets[bucketKey];

    // Prune stale buckets to keep the store compact.
    const staleThreshold = now - WINDOW_MS * 2;
    for (const [key, bucket] of Object.entries(store.buckets)) {
      if (!bucket || bucket.windowStartMs < staleThreshold) {
        delete store.buckets[key];
      }
    }

    if (!existing || now - existing.windowStartMs >= WINDOW_MS) {
      const fresh: RateBucket = { windowStartMs: now, count: 1 };
      store.buckets[bucketKey] = fresh;
      store.updatedAt = new Date().toISOString();
      await writeRateLimitStore(storePath, store);
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
    store.buckets[bucketKey] = existing;
    store.updatedAt = new Date().toISOString();
    await writeRateLimitStore(storePath, store);

    return {
      blocked: false,
      limit,
      remaining: Math.max(0, limit - existing.count),
      resetEpochSeconds: Math.floor((existing.windowStartMs + WINDOW_MS) / 1000),
    };
  });
}

async function writeAuditRecord(record: PublicApiAuditRecord): Promise<void> {
  const auditPath = await resolveStorePath(
    "PIPELINEX_PUBLIC_API_AUDIT_LOG_FILE",
    AUDIT_LOG_RELATIVE_PATH,
  );
  await mkdir(path.dirname(auditPath), { recursive: true });
  await appendFile(auditPath, `${JSON.stringify(record)}\n`, "utf8");
}

async function auditPublicApiRequest(record: PublicApiAuditRecord): Promise<void> {
  try {
    await writeAuditRecord(record);
  } catch {
    // Auditing must never break API functionality.
  }
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

async function buildAuthFailure(
  request: Request,
  requestId: string,
  requiredScope: PublicApiScope,
  status: number,
  message: string,
  keyId = "anonymous",
): Promise<AuthFailure> {
  const response = authError(status, message);
  await auditPublicApiRequest({
    timestamp: new Date().toISOString(),
    requestId,
    keyId,
    scope: requiredScope,
    method: request.method,
    path: new URL(request.url).pathname,
    status,
    clientIp: parseClientIp(request),
    userAgent: request.headers.get("user-agent") || "unknown",
    message,
  });
  return { ok: false, response };
}

export async function authenticatePublicApiRequest(
  request: Request,
  requiredScope: PublicApiScope,
): Promise<AuthResult> {
  const requestId = parseRequestId(request);
  const clientIp = parseClientIp(request);
  const userAgent = request.headers.get("user-agent") || "unknown";

  let configuredKeys: ApiKeyConfig[];
  try {
    configuredKeys = await parseConfiguredApiKeys();
  } catch (error) {
    return buildAuthFailure(
      request,
      requestId,
      requiredScope,
      500,
      error instanceof Error ? error.message : "Public API key configuration error.",
    );
  }

  if (configuredKeys.length === 0) {
    return buildAuthFailure(
      request,
      requestId,
      requiredScope,
      503,
      "Public API is not configured. Set PIPELINEX_API_KEY, PIPELINEX_API_KEYS, or PIPELINEX_API_KEYS_FILE.",
    );
  }

  const incomingKey = extractApiKey(request);
  if (!incomingKey) {
    return buildAuthFailure(
      request,
      requestId,
      requiredScope,
      401,
      "Missing API key. Use Authorization: Bearer <token>.",
    );
  }

  const matched = configuredKeys.find((config) => safeKeyMatch(config.key, incomingKey));
  if (!matched) {
    return buildAuthFailure(
      request,
      requestId,
      requiredScope,
      401,
      "Invalid API key.",
    );
  }

  if (!matched.scopes.includes(requiredScope)) {
    return buildAuthFailure(
      request,
      requestId,
      requiredScope,
      403,
      `Insufficient scope. Required scope: ${requiredScope}.`,
      matched.id,
    );
  }

  const rateLimit = await consumeRateLimit(
    matched.id,
    clientIp,
    matched.rateLimitPerMinute,
  );
  if (rateLimit.blocked) {
    const response = NextResponse.json(
      { error: "Rate limit exceeded. Please retry after reset." },
      { status: 429 },
    );
    applyRateLimitHeaders(response, rateLimit);
    await auditPublicApiRequest({
      timestamp: new Date().toISOString(),
      requestId,
      keyId: matched.id,
      scope: requiredScope,
      method: request.method,
      path: new URL(request.url).pathname,
      status: 429,
      clientIp,
      userAgent,
      message: "Rate limit exceeded.",
      rateLimit,
    });
    return { ok: false, response };
  }

  return {
    ok: true,
    principal: {
      id: matched.id,
      scopes: matched.scopes,
      roles: matched.roles,
    },
    requiredScope,
    rateLimit,
    requestId,
    clientIp,
    userAgent,
  };
}

export async function finalizePublicApiResponse(
  request: Request,
  auth: AuthSuccess,
  response: NextResponse,
  message?: string,
): Promise<NextResponse> {
  applyRateLimitHeaders(response, auth.rateLimit);
  await auditPublicApiRequest({
    timestamp: new Date().toISOString(),
    requestId: auth.requestId,
    keyId: auth.principal.id,
    scope: auth.requiredScope,
    method: request.method,
    path: new URL(request.url).pathname,
    status: response.status,
    clientIp: auth.clientIp,
    userAgent: auth.userAgent,
    message,
    rateLimit: auth.rateLimit,
  });
  return response;
}

export async function queryPublicApiAuditLogs(
  query: PublicApiAuditQuery,
): Promise<PublicApiAuditRecord[]> {
  const auditPath = await resolveStorePath(
    "PIPELINEX_PUBLIC_API_AUDIT_LOG_FILE",
    AUDIT_LOG_RELATIVE_PATH,
  );

  if (!(await pathExists(auditPath))) {
    return [];
  }

  const requestedLimit =
    typeof query.limit === "number" && Number.isFinite(query.limit)
      ? Math.floor(query.limit)
      : DEFAULT_AUDIT_QUERY_LIMIT;
  const limit = Math.min(Math.max(requestedLimit, 1), MAX_AUDIT_QUERY_LIMIT);

  const sinceMs = query.since ? Date.parse(query.since) : Number.NaN;
  const untilMs = query.until ? Date.parse(query.until) : Number.NaN;

  const content = await readFile(auditPath, "utf8");
  const lines = content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  const filtered: PublicApiAuditRecord[] = [];

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(lines[index]);
    } catch {
      continue;
    }

    if (!parsed || typeof parsed !== "object") {
      continue;
    }

    const record = parsed as Partial<PublicApiAuditRecord>;
    if (
      typeof record.timestamp !== "string" ||
      typeof record.requestId !== "string" ||
      typeof record.keyId !== "string" ||
      typeof record.scope !== "string" ||
      typeof record.method !== "string" ||
      typeof record.path !== "string" ||
      typeof record.status !== "number" ||
      typeof record.clientIp !== "string" ||
      typeof record.userAgent !== "string"
    ) {
      continue;
    }

    const ts = Date.parse(record.timestamp);
    if (query.keyId && record.keyId !== query.keyId) {
      continue;
    }
    if (query.scope && record.scope !== query.scope) {
      continue;
    }
    if (query.method && record.method.toUpperCase() !== query.method.toUpperCase()) {
      continue;
    }
    if (query.pathContains && !record.path.includes(query.pathContains)) {
      continue;
    }
    if (typeof query.status === "number" && record.status !== query.status) {
      continue;
    }
    if (Number.isFinite(sinceMs) && Number.isFinite(ts) && ts < sinceMs) {
      continue;
    }
    if (Number.isFinite(untilMs) && Number.isFinite(ts) && ts > untilMs) {
      continue;
    }

    filtered.push({
      timestamp: record.timestamp,
      requestId: record.requestId,
      keyId: record.keyId,
      scope: record.scope,
      method: record.method,
      path: record.path,
      status: record.status,
      clientIp: record.clientIp,
      userAgent: record.userAgent,
      message: typeof record.message === "string" ? record.message : undefined,
      rateLimit:
        record.rateLimit &&
        typeof record.rateLimit.limit === "number" &&
        typeof record.rateLimit.remaining === "number" &&
        typeof record.rateLimit.resetEpochSeconds === "number"
          ? {
              limit: record.rateLimit.limit,
              remaining: record.rateLimit.remaining,
              resetEpochSeconds: record.rateLimit.resetEpochSeconds,
            }
          : undefined,
    });

    if (filtered.length >= limit) {
      break;
    }
  }

  return filtered;
}
