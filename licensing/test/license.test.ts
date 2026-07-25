import { describe, it, expect } from "vitest";
import {
  importPrivateKey,
  signLicense,
  verifyLicense,
  bytesToBase64url,
  base64urlToBytes,
  base64ToBytes,
  type LicensePayload,
} from "../src/crypto";
import { genEphemeralPrivateKeyB64 } from "./helpers";

/** Derive the public verify key from a base64 PKCS8 private key (mirrors the Worker). */
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
  return crypto.subtle.importKey("jwk", pubJwk, { name: "Ed25519" }, false, ["verify"]);
}

function samplePayload(): LicensePayload {
  return {
    v: 1,
    product: "rbxsync",
    plan: "lifetime-v1",
    email: "buyer@example.com",
    licenseId: "11111111-2222-3333-4444-555555555555",
    issued: 1_700_000_000,
  };
}

describe("base64url", () => {
  it("round-trips arbitrary bytes, unpadded and URL-safe", () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 251, 252, 253, 254, 255]);
    const enc = bytesToBase64url(bytes);
    expect(enc).not.toMatch(/[+/=]/);
    expect(Array.from(base64urlToBytes(enc))).toEqual(Array.from(bytes));
  });
});

describe("license sign → verify round-trip", () => {
  it("signs and verifies a valid token", async () => {
    const privB64 = await genEphemeralPrivateKeyB64();
    const priv = await importPrivateKey(privB64);
    const pub = await derivePublicKey(privB64);

    const payload = samplePayload();
    const token = await signLicense(payload, priv);

    // Structure: exactly one "."; both segments non-empty.
    expect(token.split(".")).toHaveLength(2);

    const result = await verifyLicense(token, pub);
    expect(result.valid).toBe(true);
    expect(result.payload).toEqual(payload);
  });

  it("rejects a token signed by a DIFFERENT key", async () => {
    const priv = await importPrivateKey(await genEphemeralPrivateKeyB64());
    const otherPub = await derivePublicKey(await genEphemeralPrivateKeyB64());
    const token = await signLicense(samplePayload(), priv);
    const result = await verifyLicense(token, otherPub);
    expect(result.valid).toBe(false);
  });

  it("rejects a token whose payload was tampered with", async () => {
    const privB64 = await genEphemeralPrivateKeyB64();
    const priv = await importPrivateKey(privB64);
    const pub = await derivePublicKey(privB64);

    const token = await signLicense(samplePayload(), priv);
    const [, sig] = token.split(".");
    // Swap in a different payload but keep the original signature.
    const forgedPayload = bytesToBase64url(
      new TextEncoder().encode(
        JSON.stringify({ ...samplePayload(), email: "attacker@example.com" }),
      ),
    );
    const forged = `${forgedPayload}.${sig}`;
    const result = await verifyLicense(forged, pub);
    expect(result.valid).toBe(false);
  });

  it("rejects malformed tokens", async () => {
    const pub = await derivePublicKey(await genEphemeralPrivateKeyB64());
    for (const bad of ["", "nodot", "a.b.c", ".", "abc.", ".abc"]) {
      const result = await verifyLicense(bad, pub);
      expect(result.valid).toBe(false);
    }
  });

  it("the signed bytes are the ENCODED payload segment (JWT-style), so the Rust CLI can verify byte-for-byte", async () => {
    const privB64 = await genEphemeralPrivateKeyB64();
    const priv = await importPrivateKey(privB64);
    const pub = await derivePublicKey(privB64);
    const token = await signLicense(samplePayload(), priv);
    const [encodedPayload, encodedSig] = token.split(".");

    // Independently verify: signature over UTF-8 bytes of the encoded payload string.
    const ok = await crypto.subtle.verify(
      { name: "Ed25519" },
      pub,
      base64urlToBytes(encodedSig) as unknown as ArrayBuffer,
      new TextEncoder().encode(encodedPayload) as unknown as ArrayBuffer,
    );
    expect(ok).toBe(true);
  });
});
