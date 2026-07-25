/**
 * RbxSync — License Fulfillment Worker
 *
 * Flow: buyer pays via Stripe Checkout → Stripe fires `checkout.session.completed`
 * → this Worker mints an Ed25519-signed license token + a gated download token,
 * stores them in KV, and emails the buyer their key + download link. The CLI
 * verifies the license OFFLINE against an embedded public key.
 *
 * Endpoints:
 *   POST /webhook/stripe    — verify Stripe signature, fulfill on checkout.session.completed
 *   GET  /download?token=   — 302 to the gated installer (INSTALLER_BASE_URL)
 *   POST /license/validate  — optional online Ed25519 verify of a token
 *   GET  /health            — { ok: true }
 */

import type { Env } from "./types";
import { verifyStripeSignature } from "./stripe";
import {
  importPrivateKey,
  signLicense,
  verifyLicense,
  bytesToBase64url,
  base64ToBytes,
  type LicensePayload,
} from "./crypto";
import { sendLicenseEmail } from "./email";

// Generous cap; past this we still serve but log for abuse review.
const DOWNLOAD_CAP = 25;

interface StoredLicense {
  licenseId: string;
  email: string;
  license: string; // the signed token
  downloadToken: string;
  created: string; // ISO
  downloadCount: number;
  stripeSessionId?: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    try {
      if (path === "/health" && request.method === "GET") {
        return json({ ok: true });
      }
      if (path === "/webhook/stripe" && request.method === "POST") {
        return await handleStripeWebhook(request, env);
      }
      if (path === "/download" && request.method === "GET") {
        return await handleDownload(url, env);
      }
      if (path === "/license/validate" && request.method === "POST") {
        return await handleValidate(request, env);
      }
      return new Response("Not found", { status: 404 });
    } catch (err) {
      // Never leak internals; log for the operator.
      console.error("Unhandled error:", err instanceof Error ? err.stack : err);
      return new Response("Internal error", { status: 500 });
    }
  },
};

// ─── KV key helpers ──────────────────────────────────────────────────────────

const kLicense = (id: string) => `license:${id}`;
const kDownload = (token: string) => `download:${token}`;
const kEvent = (eventId: string) => `event:${eventId}`;
const kSession = (sessionId: string) => `session:${sessionId}`;

// ─── POST /webhook/stripe ────────────────────────────────────────────────────

async function handleStripeWebhook(request: Request, env: Env): Promise<Response> {
  // Read the raw body EXACTLY ONCE and HMAC those exact bytes.
  const rawBody = await request.text();
  const sigHeader = request.headers.get("Stripe-Signature");

  const verified = await verifyStripeSignature(
    rawBody,
    sigHeader,
    env.STRIPE_WEBHOOK_SECRET,
  );
  if (!verified.valid) {
    // 400 → Stripe marks the delivery failed (it does NOT retry on 4xx here,
    // which is correct: a bad signature is not a transient problem).
    return new Response(`Bad signature: ${verified.reason ?? "invalid"}`, { status: 400 });
  }

  let event: any;
  try {
    event = JSON.parse(rawBody);
  } catch {
    return new Response("Bad payload", { status: 400 });
  }

  // Only act on completed checkouts. Acknowledge everything else with 200 so
  // Stripe doesn't retry events we intentionally ignore.
  if (event?.type !== "checkout.session.completed") {
    return json({ received: true, ignored: event?.type ?? "unknown" });
  }

  const eventId: string | undefined = event.id;
  if (!eventId) return new Response("Missing event id", { status: 400 });

  // Idempotency on Stripe's event.id: if we've already processed it, ack 200.
  const seen = await env.LICENSES.get(kEvent(eventId));
  if (seen) {
    return json({ received: true, duplicate: true });
  }

  const session = event.data?.object ?? {};
  const email: string | undefined = session.customer_details?.email ?? session.customer_email;
  const sessionId: string | undefined = session.id;

  if (!email) {
    // We can't deliver a license with no email. Mark the event processed so we
    // don't loop on retries, and surface it to the operator via logs.
    console.error("checkout.session.completed with no email; event", eventId, "session", sessionId);
    await env.LICENSES.put(kEvent(eventId), JSON.stringify({ error: "no-email", at: nowIso() }));
    return json({ received: true, warning: "no email on session" });
  }

  // Converge on the checkout SESSION id: if a prior (retried) delivery already
  // minted a license for this session, reuse it instead of minting a second key.
  let stored: StoredLicense | null = null;
  if (sessionId) {
    const existingId = await env.LICENSES.get(kSession(sessionId));
    if (existingId) {
      stored = await env.LICENSES.get<StoredLicense>(kLicense(existingId), "json");
    }
  }

  if (!stored) {
    stored = await mintLicense(env, email, sessionId);
  }

  const downloadUrl = `${trimSlash(env.WORKER_URL)}/download?token=${stored.downloadToken}`;

  // AWAIT the email. If ZeptoMail fails, return non-2xx so Stripe RETRIES the
  // webhook; idempotency (event.id + session.id) ensures the retry reuses the
  // same license rather than minting a new one. The unacceptable outcome is
  // "buyer paid, got nothing" — a duplicate email is acceptable, a lost one isn't.
  const emailResult = await sendLicenseEmail(
    { toEmail: email, licenseKey: stored.license, downloadUrl },
    env,
  );

  if (!emailResult.ok) {
    console.error("License email failed:", emailResult.status, emailResult.body, "event", eventId);
    return new Response("Email delivery failed; will retry", { status: 502 });
  }

  // Mark the event processed only AFTER successful delivery.
  await env.LICENSES.put(
    kEvent(eventId),
    JSON.stringify({ licenseId: stored.licenseId, at: nowIso() }),
  );

  return json({ received: true, licenseId: stored.licenseId });
}

/** Generate + persist a signed license and its download token. */
async function mintLicense(
  env: Env,
  email: string,
  sessionId?: string,
): Promise<StoredLicense> {
  const licenseId = crypto.randomUUID();
  const downloadToken = randomToken();
  const issued = Math.floor(Date.now() / 1000);

  const payload: LicensePayload = {
    v: 1,
    product: "rbxsync",
    plan: "lifetime-v1",
    email,
    licenseId,
    issued,
  };

  const privateKey = await importPrivateKey(env.LICENSE_SIGNING_PRIVATE_KEY);
  const license = await signLicense(payload, privateKey);

  const record: StoredLicense = {
    licenseId,
    email,
    license,
    downloadToken,
    created: nowIso(),
    downloadCount: 0,
    stripeSessionId: sessionId,
  };

  // Store under BOTH keys (licenseId and download token) so /download can look
  // up by token, plus a session→licenseId pointer for retry convergence.
  await env.LICENSES.put(kLicense(licenseId), JSON.stringify(record));
  await env.LICENSES.put(kDownload(downloadToken), licenseId);
  if (sessionId) {
    await env.LICENSES.put(kSession(sessionId), licenseId);
  }

  return record;
}

// ─── GET /download?token= ────────────────────────────────────────────────────

async function handleDownload(url: URL, env: Env): Promise<Response> {
  const token = url.searchParams.get("token");
  if (!token) return new Response("Missing token", { status: 403 });

  const licenseId = await env.LICENSES.get(kDownload(token));
  if (!licenseId) return new Response("Invalid or expired download link", { status: 403 });

  const record = await env.LICENSES.get<StoredLicense>(kLicense(licenseId), "json");
  if (!record) return new Response("Invalid or expired download link", { status: 403 });

  record.downloadCount += 1;
  if (record.downloadCount > DOWNLOAD_CAP) {
    console.warn(
      `Download cap exceeded for license ${licenseId} (count ${record.downloadCount})`,
    );
  }
  await env.LICENSES.put(kLicense(licenseId), JSON.stringify(record));

  // Redirect to the gated installer. INSTALLER_BASE_URL is a placeholder for now.
  return Response.redirect(trimSlash(env.INSTALLER_BASE_URL), 302);
}

// ─── POST /license/validate ──────────────────────────────────────────────────

async function handleValidate(request: Request, env: Env): Promise<Response> {
  let body: any;
  try {
    body = await request.json();
  } catch {
    return json({ valid: false, error: "invalid JSON body" }, 400);
  }
  const token: unknown = body?.license;
  if (typeof token !== "string" || token.length === 0) {
    return json({ valid: false, error: "missing license" }, 400);
  }

  // Derive the public key from the private key so validate stays in sync with
  // signing without needing a separate public-key secret.
  const publicKey = await derivePublicKey(env.LICENSE_SIGNING_PRIVATE_KEY);
  const result = await verifyLicense(token, publicKey);
  if (!result.valid || !result.payload) {
    return json({ valid: false });
  }
  return json({
    valid: true,
    email: result.payload.email,
    licenseId: result.payload.licenseId,
    issued: result.payload.issued,
  });
}

/**
 * Derive the verifying (public) key from the PKCS8 private key so /license/validate
 * stays in lock-step with signing without a separate public-key secret.
 *
 * Web Crypto can't export a public CryptoKey from a private one directly, so we
 * import the private key as extractable, export its JWK, drop the private `d`
 * scalar, and re-import the remainder (`x` = the public coordinate) as a verify key.
 */
async function derivePublicKey(pkcs8Base64: string): Promise<CryptoKey> {
  const der = base64ToBytes(pkcs8Base64);
  const priv = await crypto.subtle.importKey(
    "pkcs8",
    der as unknown as ArrayBuffer,
    { name: "Ed25519" },
    true,
    ["sign"],
  );
  const jwk = await crypto.subtle.exportKey("jwk", priv);
  const pubJwk: JsonWebKey = { kty: jwk.kty, crv: jwk.crv, x: jwk.x };
  return crypto.subtle.importKey(
    "jwk",
    pubJwk,
    { name: "Ed25519" },
    false,
    ["verify"],
  );
}

// ─── utils ───────────────────────────────────────────────────────────────────

function json(obj: unknown, status = 200): Response {
  return new Response(JSON.stringify(obj), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function nowIso(): string {
  return new Date().toISOString();
}

function trimSlash(s: string): string {
  return s.replace(/\/+$/, "");
}

/** 32 bytes of randomness, base64url — used for the unguessable download token. */
function randomToken(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return bytesToBase64url(bytes);
}
