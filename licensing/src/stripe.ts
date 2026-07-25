/**
 * Stripe webhook signature verification, implemented directly with Web Crypto.
 *
 * We do NOT depend on the `stripe` npm SDK: signature verification is just
 * HMAC-SHA256, which behaves identically on Node and the Workers runtime, and
 * avoiding the SDK removes a large dependency and any SDK-on-workerd risk.
 *
 * Stripe's scheme (https://stripe.com/docs/webhooks/signatures):
 *   - Header `Stripe-Signature: t=<ts>,v1=<sig>,v1=<sig2>,...`
 *   - signed_payload = `${t}.${rawRequestBody}`
 *   - expected = HMAC-SHA256(signed_payload, webhookSecret) as lowercase hex
 *   - Accept if ANY provided v1 matches (constant-time), reject if |now - t| > tolerance.
 *
 * IMPORTANT: `rawBody` MUST be the exact bytes read from the request
 * (`await request.text()`), read once. Never HMAC a re-stringified object.
 */

const encoder = new TextEncoder();

function toHex(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    hex += bytes[i].toString(16).padStart(2, "0");
  }
  return hex;
}

/** Constant-time string comparison (both args are lowercase hex of equal expected length). */
function timingSafeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) {
    mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return mismatch === 0;
}

interface ParsedSigHeader {
  timestamp: number | null;
  signatures: string[];
}

/** Parse `t=..,v1=..,v1=..` into a timestamp and the list of v1 signatures. */
export function parseStripeSigHeader(header: string): ParsedSigHeader {
  let timestamp: number | null = null;
  const signatures: string[] = [];
  for (const part of header.split(",")) {
    const eq = part.indexOf("=");
    if (eq === -1) continue;
    const key = part.slice(0, eq).trim();
    const value = part.slice(eq + 1).trim();
    if (key === "t") {
      const n = parseInt(value, 10);
      if (!Number.isNaN(n)) timestamp = n;
    } else if (key === "v1") {
      signatures.push(value);
    }
  }
  return { timestamp, signatures };
}

export interface StripeVerifyResult {
  valid: boolean;
  reason?: string;
}

/**
 * Verify a Stripe webhook signature.
 *
 * @param rawBody         Exact request body bytes as a string (read once).
 * @param sigHeader       Value of the `Stripe-Signature` header.
 * @param secret          STRIPE_WEBHOOK_SECRET (the `whsec_...` value).
 * @param toleranceSec    Max allowed clock skew (default 300s = 5 min).
 * @param nowSec          Override current time (unix seconds) — for tests.
 */
export async function verifyStripeSignature(
  rawBody: string,
  sigHeader: string | null,
  secret: string,
  toleranceSec = 300,
  nowSec: number = Math.floor(Date.now() / 1000),
): Promise<StripeVerifyResult> {
  if (!sigHeader) return { valid: false, reason: "missing signature header" };
  if (!secret) return { valid: false, reason: "missing webhook secret" };

  const { timestamp, signatures } = parseStripeSigHeader(sigHeader);
  if (timestamp === null) return { valid: false, reason: "no timestamp in header" };
  if (signatures.length === 0) return { valid: false, reason: "no v1 signatures in header" };

  // Reject stale (or future-dated) events outside the tolerance window.
  if (Math.abs(nowSec - timestamp) > toleranceSec) {
    return { valid: false, reason: "timestamp outside tolerance" };
  }

  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret) as unknown as ArrayBuffer,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signedPayload = `${timestamp}.${rawBody}`;
  const mac = await crypto.subtle.sign(
    "HMAC",
    key,
    encoder.encode(signedPayload) as unknown as ArrayBuffer,
  );
  const expected = toHex(mac);

  // Accept if ANY provided v1 signature matches (constant-time).
  for (const provided of signatures) {
    if (timingSafeEqual(expected, provided.toLowerCase())) {
      return { valid: true };
    }
  }
  return { valid: false, reason: "no matching signature" };
}
