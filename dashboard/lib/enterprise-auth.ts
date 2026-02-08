import { createHmac, randomUUID, timingSafeEqual } from "node:crypto";

export type EnterpriseScope = "benchmarks:read" | "benchmarks:write" | "audit:read";

export type EnterpriseRole = "admin" | "analyst" | "ingest" | "viewer" | "auditor";

export interface EnterpriseSessionPrincipal {
  subject: string;
  email?: string;
  org?: string;
  roles: EnterpriseRole[];
  scopes: EnterpriseScope[];
  sessionId: string;
  expiresAt: string;
}

interface EnterpriseSessionPayload extends EnterpriseSessionPrincipal {
  version: 1;
  issuedAt: string;
}

export interface EnterpriseSsoAssertion {
  subject: string;
  email?: string;
  org?: string;
  roles?: EnterpriseRole[];
  scopes?: EnterpriseScope[];
  issuedAt: string;
  expiresAt: string;
  nonce?: string;
  signature: string;
}

export type EnterpriseSessionAuthResult =
  | {
      ok: true;
      principal: EnterpriseSessionPrincipal;
    }
  | {
      ok: false;
      status: number;
      message: string;
    };

export type EnterpriseSsoExchangeResult =
  | {
      ok: true;
      token: string;
      principal: EnterpriseSessionPrincipal;
    }
  | {
      ok: false;
      status: number;
      message: string;
    };

const ENTERPRISE_TOKEN_PREFIX = "pxe";
const DEFAULT_SESSION_TTL_SECONDS = 3600;
const MAX_SESSION_TTL_SECONDS = 86_400;
const MIN_SESSION_TTL_SECONDS = 60;
const CLOCK_SKEW_MS = 5 * 60_000;

const ROLE_SCOPES: Record<EnterpriseRole, EnterpriseScope[]> = {
  admin: ["benchmarks:read", "benchmarks:write", "audit:read"],
  analyst: ["benchmarks:read", "audit:read"],
  ingest: ["benchmarks:write"],
  viewer: ["benchmarks:read"],
  auditor: ["audit:read"],
};

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

function safeMatch(expected: string, provided: string): boolean {
  const expectedBuffer = Buffer.from(expected);
  const providedBuffer = Buffer.from(provided);

  if (expectedBuffer.length !== providedBuffer.length) {
    return false;
  }

  return timingSafeEqual(expectedBuffer, providedBuffer);
}

function dedupeScopes(scopes: EnterpriseScope[]): EnterpriseScope[] {
  return [...new Set(scopes)];
}

function dedupeRoles(roles: EnterpriseRole[]): EnterpriseRole[] {
  return [...new Set(roles)];
}

function normalizeRoles(rawRoles: unknown): EnterpriseRole[] {
  if (!Array.isArray(rawRoles)) {
    return [];
  }

  const allowed: EnterpriseRole[] = ["admin", "analyst", "ingest", "viewer", "auditor"];
  return rawRoles.filter((role): role is EnterpriseRole =>
    typeof role === "string" ? allowed.includes(role as EnterpriseRole) : false,
  );
}

function normalizeScopes(rawScopes: unknown): EnterpriseScope[] {
  if (!Array.isArray(rawScopes)) {
    return [];
  }

  const allowed: EnterpriseScope[] = ["benchmarks:read", "benchmarks:write", "audit:read"];
  return rawScopes.filter((scope): scope is EnterpriseScope =>
    typeof scope === "string" ? allowed.includes(scope as EnterpriseScope) : false,
  );
}

function scopesForRoles(roles: EnterpriseRole[]): EnterpriseScope[] {
  const scopes: EnterpriseScope[] = [];
  for (const role of roles) {
    scopes.push(...ROLE_SCOPES[role]);
  }
  return dedupeScopes(scopes);
}

function mergeScopesWithRoles(
  scopes: EnterpriseScope[],
  roles: EnterpriseRole[],
  fallbackScopes: EnterpriseScope[],
): EnterpriseScope[] {
  const merged = dedupeScopes([...scopes, ...scopesForRoles(roles)]);
  return merged.length > 0 ? merged : fallbackScopes;
}

function getSessionSecret(): string | null {
  const value = process.env.PIPELINEX_ENTERPRISE_SESSION_SECRET?.trim() || "";
  return value.length > 0 ? value : null;
}

function getSsoSharedSecret(): string | null {
  const value = process.env.PIPELINEX_SSO_SHARED_SECRET?.trim() || "";
  return value.length > 0 ? value : null;
}

function parseSessionTtlSeconds(): number {
  const raw = process.env.PIPELINEX_ENTERPRISE_SESSION_TTL_SECONDS;
  if (!raw) {
    return DEFAULT_SESSION_TTL_SECONDS;
  }

  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_SESSION_TTL_SECONDS;
  }

  return Math.min(MAX_SESSION_TTL_SECONDS, Math.max(MIN_SESSION_TTL_SECONDS, parsed));
}

function signValue(value: string, secret: string): string {
  return createHmac("sha256", secret).update(value).digest("base64url");
}

function encodePayload(payload: EnterpriseSessionPayload): string {
  return Buffer.from(JSON.stringify(payload), "utf8").toString("base64url");
}

function decodePayload(encoded: string): EnterpriseSessionPayload | null {
  try {
    const raw = Buffer.from(encoded, "base64url").toString("utf8");
    const parsed = JSON.parse(raw) as Partial<EnterpriseSessionPayload>;
    if (
      parsed.version !== 1 ||
      typeof parsed.subject !== "string" ||
      parsed.subject.trim().length === 0 ||
      !Array.isArray(parsed.roles) ||
      !Array.isArray(parsed.scopes) ||
      typeof parsed.sessionId !== "string" ||
      typeof parsed.issuedAt !== "string" ||
      typeof parsed.expiresAt !== "string"
    ) {
      return null;
    }

    const issuedAt = parseIsoTimestamp(parsed.issuedAt);
    const expiresAt = parseIsoTimestamp(parsed.expiresAt);
    if (!issuedAt || !expiresAt) {
      return null;
    }

    const roles = dedupeRoles(normalizeRoles(parsed.roles));
    const explicitScopes = normalizeScopes(parsed.scopes);
    const scopes = mergeScopesWithRoles(explicitScopes, roles, ["benchmarks:read"]);

    return {
      version: 1,
      subject: parsed.subject.trim(),
      email:
        typeof parsed.email === "string" && parsed.email.trim().length > 0
          ? parsed.email.trim()
          : undefined,
      org:
        typeof parsed.org === "string" && parsed.org.trim().length > 0
          ? parsed.org.trim()
          : undefined,
      roles,
      scopes,
      sessionId: parsed.sessionId,
      issuedAt,
      expiresAt,
    };
  } catch {
    return null;
  }
}

function serializeEnterpriseToken(encodedPayload: string, signature: string): string {
  return `${ENTERPRISE_TOKEN_PREFIX}.${encodedPayload}.${signature}`;
}

function parseEnterpriseToken(token: string): {
  encodedPayload: string;
  signature: string;
} | null {
  const parts = token.split(".");
  if (parts.length !== 3) {
    return null;
  }
  if (parts[0] !== ENTERPRISE_TOKEN_PREFIX) {
    return null;
  }
  const encodedPayload = parts[1]?.trim();
  const signature = parts[2]?.trim();
  if (!encodedPayload || !signature) {
    return null;
  }
  return { encodedPayload, signature };
}

function extractEnterpriseTokenFromRequest(request: Request): string | null {
  const headerToken = request.headers.get("x-enterprise-token")?.trim();
  if (headerToken) {
    return headerToken;
  }

  const authHeader = request.headers.get("authorization")?.trim() || "";
  if (authHeader.toLowerCase().startsWith("bearer ")) {
    const token = authHeader.slice(7).trim();
    if (token.startsWith(`${ENTERPRISE_TOKEN_PREFIX}.`)) {
      return token;
    }
  }

  return null;
}

export function isEnterpriseSessionAuthEnabled(): boolean {
  return Boolean(getSessionSecret());
}

function buildAssertionSigningPayload(assertion: {
  subject: string;
  email?: string;
  org?: string;
  roles: EnterpriseRole[];
  scopes: EnterpriseScope[];
  issuedAt: string;
  expiresAt: string;
  nonce?: string;
}): string {
  return [
    assertion.subject,
    assertion.email || "",
    assertion.org || "",
    [...assertion.roles].sort().join(","),
    [...assertion.scopes].sort().join(","),
    assertion.issuedAt,
    assertion.expiresAt,
    assertion.nonce || "",
  ].join("\n");
}

function normalizeAssertion(assertion: EnterpriseSsoAssertion): {
  subject: string;
  email?: string;
  org?: string;
  roles: EnterpriseRole[];
  scopes: EnterpriseScope[];
  issuedAt: string;
  expiresAt: string;
  nonce?: string;
  signature: string;
} | null {
  const subject = assertion.subject?.trim();
  const issuedAt = parseIsoTimestamp(assertion.issuedAt);
  const expiresAt = parseIsoTimestamp(assertion.expiresAt);
  const signature = assertion.signature?.trim();

  if (!subject || !issuedAt || !expiresAt || !signature) {
    return null;
  }

  const roles = dedupeRoles(normalizeRoles(assertion.roles));
  const explicitScopes = normalizeScopes(assertion.scopes);
  const scopes = mergeScopesWithRoles(explicitScopes, roles, ["benchmarks:read"]);

  return {
    subject,
    email:
      typeof assertion.email === "string" && assertion.email.trim().length > 0
        ? assertion.email.trim()
        : undefined,
    org:
      typeof assertion.org === "string" && assertion.org.trim().length > 0
        ? assertion.org.trim()
        : undefined,
    roles,
    scopes,
    issuedAt,
    expiresAt,
    nonce:
      typeof assertion.nonce === "string" && assertion.nonce.trim().length > 0
        ? assertion.nonce.trim()
        : undefined,
    signature,
  };
}

function verifySsoAssertionSignature(
  normalizedAssertion: {
    subject: string;
    email?: string;
    org?: string;
    roles: EnterpriseRole[];
    scopes: EnterpriseScope[];
    issuedAt: string;
    expiresAt: string;
    nonce?: string;
    signature: string;
  },
  sharedSecret: string,
): boolean {
  const payload = buildAssertionSigningPayload(normalizedAssertion);
  const expectedSignature = signValue(payload, sharedSecret);
  return safeMatch(expectedSignature, normalizedAssertion.signature);
}

function principalFromPayload(payload: EnterpriseSessionPayload): EnterpriseSessionPrincipal {
  return {
    subject: payload.subject,
    email: payload.email,
    org: payload.org,
    roles: payload.roles,
    scopes: payload.scopes,
    sessionId: payload.sessionId,
    expiresAt: payload.expiresAt,
  };
}

export function authenticateEnterpriseSessionRequest(
  request: Request,
  requiredScope?: EnterpriseScope,
): EnterpriseSessionAuthResult {
  const sessionSecret = getSessionSecret();
  if (!sessionSecret) {
    return {
      ok: false,
      status: 503,
      message: "Enterprise session auth is not configured.",
    };
  }

  const token = extractEnterpriseTokenFromRequest(request);
  if (!token) {
    return {
      ok: false,
      status: 401,
      message: "Missing enterprise session token.",
    };
  }

  const parsedToken = parseEnterpriseToken(token);
  if (!parsedToken) {
    return {
      ok: false,
      status: 401,
      message: "Invalid enterprise session token format.",
    };
  }

  const expectedSignature = signValue(parsedToken.encodedPayload, sessionSecret);
  if (!safeMatch(expectedSignature, parsedToken.signature)) {
    return {
      ok: false,
      status: 401,
      message: "Invalid enterprise session token signature.",
    };
  }

  const payload = decodePayload(parsedToken.encodedPayload);
  if (!payload) {
    return {
      ok: false,
      status: 401,
      message: "Invalid enterprise session token payload.",
    };
  }

  const nowMs = Date.now();
  const expiresMs = Date.parse(payload.expiresAt);
  if (!Number.isFinite(expiresMs) || nowMs >= expiresMs) {
    return {
      ok: false,
      status: 401,
      message: "Enterprise session token has expired.",
    };
  }

  const issuedMs = Date.parse(payload.issuedAt);
  if (Number.isFinite(issuedMs) && nowMs + CLOCK_SKEW_MS < issuedMs) {
    return {
      ok: false,
      status: 401,
      message: "Enterprise session token is not yet valid.",
    };
  }

  if (requiredScope && !payload.scopes.includes(requiredScope)) {
    return {
      ok: false,
      status: 403,
      message: `Insufficient scope. Required scope: ${requiredScope}.`,
    };
  }

  return {
    ok: true,
    principal: principalFromPayload(payload),
  };
}

export function exchangeEnterpriseSsoAssertion(
  assertion: EnterpriseSsoAssertion,
): EnterpriseSsoExchangeResult {
  const sharedSecret = getSsoSharedSecret();
  if (!sharedSecret) {
    return {
      ok: false,
      status: 503,
      message: "SSO assertion exchange is not configured.",
    };
  }

  const sessionSecret = getSessionSecret();
  if (!sessionSecret) {
    return {
      ok: false,
      status: 503,
      message: "Enterprise session auth is not configured.",
    };
  }

  const normalizedAssertion = normalizeAssertion(assertion);
  if (!normalizedAssertion) {
    return {
      ok: false,
      status: 400,
      message: "Invalid SSO assertion payload.",
    };
  }

  if (!verifySsoAssertionSignature(normalizedAssertion, sharedSecret)) {
    return {
      ok: false,
      status: 401,
      message: "Invalid SSO assertion signature.",
    };
  }

  const issuedAtMs = Date.parse(normalizedAssertion.issuedAt);
  const expiresAtMs = Date.parse(normalizedAssertion.expiresAt);
  if (!Number.isFinite(issuedAtMs) || !Number.isFinite(expiresAtMs) || expiresAtMs <= issuedAtMs) {
    return {
      ok: false,
      status: 400,
      message: "Invalid SSO assertion validity window.",
    };
  }

  const nowMs = Date.now();
  if (nowMs + CLOCK_SKEW_MS < issuedAtMs) {
    return {
      ok: false,
      status: 401,
      message: "SSO assertion is not yet valid.",
    };
  }
  if (nowMs > expiresAtMs + CLOCK_SKEW_MS) {
    return {
      ok: false,
      status: 401,
      message: "SSO assertion has expired.",
    };
  }

  const ttlMs = parseSessionTtlSeconds() * 1000;
  const sessionExpiresMs = Math.min(expiresAtMs, nowMs + ttlMs);

  const payload: EnterpriseSessionPayload = {
    version: 1,
    subject: normalizedAssertion.subject,
    email: normalizedAssertion.email,
    org: normalizedAssertion.org,
    roles: normalizedAssertion.roles,
    scopes: normalizedAssertion.scopes,
    sessionId: randomUUID(),
    issuedAt: new Date(nowMs).toISOString(),
    expiresAt: new Date(sessionExpiresMs).toISOString(),
  };

  const encodedPayload = encodePayload(payload);
  const signature = signValue(encodedPayload, sessionSecret);
  const token = serializeEnterpriseToken(encodedPayload, signature);

  return {
    ok: true,
    token,
    principal: principalFromPayload(payload),
  };
}
