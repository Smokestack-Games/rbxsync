/**
 * Cryptographic primitives for RbxSync license fulfillment.
 *
 * Licenses are Ed25519-signed tokens the CLI verifies OFFLINE (no phone-home).
 * The Worker holds the private key (PKCS8, base64, as a secret); the CLI embeds
 * the raw 32-byte public key.
 *
 * TOKEN FORMAT (see licensing/README.md for the full interop spec):
 *   token = base64url(utf8(JSON.stringify(payload))) + "." + base64url(signature)
 *
 * The Ed25519 signature is computed over the UTF-8 bytes of the ENCODED payload
 * segment (the string before the "."), NOT over the decoded JSON. This is the
 * JWT-style convention and sidesteps JSON canonicalization entirely, so the Rust
 * CLI can verify byte-for-byte without re-serializing.
 *
 * All base64url is UNPADDED and URL-safe (RFC 4648 §5, no "=").
 */

export interface LicensePayload {
  /** Format version. */
  v: 1;
  /** Product identifier. */
  product: "rbxsync";
  /** Plan / SKU. */
  plan: "lifetime-v1";
  /** Buyer email. */
  email: string;
  /** Opaque license id (UUID). */
  licenseId: string;
  /** Issue time, unix SECONDS. */
  issued: number;
}

// ─── base64url (unpadded, URL-safe) ──────────────────────────────────────────

export function bytesToBase64url(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function base64urlToBytes(b64url: string): Uint8Array {
  const b64 = b64url.replace(/-/g, "+").replace(/_/g, "/");
  const pad = b64.length % 4 === 0 ? "" : "=".repeat(4 - (b64.length % 4));
  const binary = atob(b64 + pad);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Decode a standard (non-url-safe) base64 string to bytes. Used for PKCS8/raw key material. */
export function base64ToBytes(b64: string): Uint8Array {
  const clean = b64.replace(/\s+/g, "");
  const binary = atob(clean);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

// ─── Key import ──────────────────────────────────────────────────────────────

/**
 * Import the Ed25519 PRIVATE key from a base64 PKCS8 DER string
 * (the value of the LICENSE_SIGNING_PRIVATE_KEY secret).
 */
export async function importPrivateKey(pkcs8Base64: string): Promise<CryptoKey> {
  const der = base64ToBytes(pkcs8Base64);
  return crypto.subtle.importKey(
    "pkcs8",
    der as unknown as ArrayBuffer,
    { name: "Ed25519" },
    false,
    ["sign"],
  );
}

/**
 * Import an Ed25519 PUBLIC key from raw 32 bytes (base64-encoded).
 * The CLI embeds these same 32 bytes.
 */
export async function importPublicKey(rawBase64: string): Promise<CryptoKey> {
  const raw = base64ToBytes(rawBase64);
  return crypto.subtle.importKey(
    "raw",
    raw as unknown as ArrayBuffer,
    { name: "Ed25519" },
    false,
    ["verify"],
  );
}

// ─── License sign / verify ───────────────────────────────────────────────────

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * Produce a signed license token from a payload and an imported private key.
 */
export async function signLicense(
  payload: LicensePayload,
  privateKey: CryptoKey,
): Promise<string> {
  const encodedPayload = bytesToBase64url(encoder.encode(JSON.stringify(payload)));
  const signature = await crypto.subtle.sign(
    { name: "Ed25519" },
    privateKey,
    encoder.encode(encodedPayload) as unknown as ArrayBuffer,
  );
  return `${encodedPayload}.${bytesToBase64url(new Uint8Array(signature))}`;
}

export interface VerifyResult {
  valid: boolean;
  payload?: LicensePayload;
}

/**
 * Verify a license token's Ed25519 signature against an imported public key.
 * Returns { valid:false } on any structural or cryptographic failure — never throws.
 */
export async function verifyLicense(
  token: string,
  publicKey: CryptoKey,
): Promise<VerifyResult> {
  try {
    const dot = token.indexOf(".");
    if (dot <= 0 || dot === token.length - 1) return { valid: false };
    const encodedPayload = token.slice(0, dot);
    const encodedSig = token.slice(dot + 1);
    // Reject anything with a second separator — a token has exactly one ".".
    if (encodedSig.indexOf(".") !== -1) return { valid: false };

    const signature = base64urlToBytes(encodedSig);
    const ok = await crypto.subtle.verify(
      { name: "Ed25519" },
      publicKey,
      signature as unknown as ArrayBuffer,
      encoder.encode(encodedPayload) as unknown as ArrayBuffer,
    );
    if (!ok) return { valid: false };

    const json = decoder.decode(base64urlToBytes(encodedPayload));
    const payload = JSON.parse(json) as LicensePayload;
    if (payload.v !== 1 || payload.product !== "rbxsync") return { valid: false };
    return { valid: true, payload };
  } catch {
    return { valid: false };
  }
}
