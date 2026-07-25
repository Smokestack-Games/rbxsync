/**
 * Test-only helpers. All key material here is EPHEMERAL — generated per test run,
 * never committed.
 */
import type { Env } from "../src/types";

const encoder = new TextEncoder();

/** Standard base64 (NOT url-safe) of a byte array, without relying on Node's Buffer. */
function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}

/** Generate an ephemeral Ed25519 keypair and return the private key as base64 PKCS8. */
export async function genEphemeralPrivateKeyB64(): Promise<string> {
  const kp = await crypto.subtle.generateKey({ name: "Ed25519" }, true, [
    "sign",
    "verify",
  ]);
  const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", kp.privateKey));
  return toBase64(pkcs8);
}

/** Compute a Stripe-style `Stripe-Signature` header for a raw body + secret. */
export async function makeStripeSigHeader(
  rawBody: string,
  secret: string,
  timestamp: number,
): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const mac = await crypto.subtle.sign("HMAC", key, encoder.encode(`${timestamp}.${rawBody}`));
  const hex = Array.from(new Uint8Array(mac))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return `t=${timestamp},v1=${hex}`;
}

/** Minimal in-memory KVNamespace covering get/put/delete used by the Worker. */
export class MockKV {
  store = new Map<string, string>();

  async get(key: string, type?: "json"): Promise<any> {
    const v = this.store.get(key);
    if (v === undefined) return null;
    return type === "json" ? JSON.parse(v) : v;
  }
  async put(key: string, value: string): Promise<void> {
    this.store.set(key, value);
  }
  async delete(key: string): Promise<void> {
    this.store.delete(key);
  }
}

export interface TestEnvOverrides {
  privateKeyB64: string;
  webhookSecret?: string;
}

export function makeEnv(kv: MockKV, o: TestEnvOverrides): Env {
  return {
    LICENSES: kv as unknown as KVNamespace,
    STRIPE_WEBHOOK_SECRET: o.webhookSecret ?? "whsec_test_secret",
    ZEPTOMAIL_TOKEN: "test-token",
    LICENSE_SIGNING_PRIVATE_KEY: o.privateKeyB64,
    FROM_EMAIL: "licenses@example.com",
    INSTALLER_BASE_URL: "https://downloads.example.com/rbxsync/latest",
    WORKER_URL: "https://worker.example.com",
  };
}

/** A checkout.session.completed event body. */
export function checkoutEvent(opts: {
  eventId: string;
  sessionId: string;
  email: string;
}): string {
  return JSON.stringify({
    id: opts.eventId,
    type: "checkout.session.completed",
    data: {
      object: {
        id: opts.sessionId,
        object: "checkout.session",
        customer_details: { email: opts.email },
      },
    },
  });
}
