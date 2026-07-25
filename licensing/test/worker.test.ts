import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import worker from "../src/index";
import {
  MockKV,
  makeEnv,
  makeStripeSigHeader,
  genEphemeralPrivateKeyB64,
  checkoutEvent,
} from "./helpers";

const SECRET = "whsec_test_secret";

let privateKeyB64: string;
let fetchMock: ReturnType<typeof vi.fn>;

function stripeRequest(rawBody: string, header: string): Request {
  return new Request("https://worker.example.com/webhook/stripe", {
    method: "POST",
    headers: { "Stripe-Signature": header, "Content-Type": "application/json" },
    body: rawBody,
  });
}

beforeEach(async () => {
  privateKeyB64 = await genEphemeralPrivateKeyB64();
  // Mock global fetch so no real ZeptoMail call happens; default = success.
  fetchMock = vi.fn(async () => new Response("{}", { status: 200 }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("GET /health", () => {
  it("returns { ok: true }", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64 });
    const res = await worker.fetch(new Request("https://worker.example.com/health"), env);
    expect(res.status).toBe(200);
    expect(await res.json()).toEqual({ ok: true });
  });
});

describe("POST /webhook/stripe", () => {
  it("fulfills a valid checkout.session.completed: mints license, stores KV, emails once", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = checkoutEvent({ eventId: "evt_1", sessionId: "cs_1", email: "buyer@example.com" });
    const header = await makeStripeSigHeader(body, SECRET, now);

    const res = await worker.fetch(stripeRequest(body, header), env);
    expect(res.status).toBe(200);
    const json = (await res.json()) as any;
    expect(json.received).toBe(true);
    expect(json.licenseId).toBeTruthy();

    // Email sent exactly once, to ZeptoMail.
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toContain("zeptomail");

    // KV: license record, download pointer, event marker, session pointer all present.
    expect(kv.store.has(`license:${json.licenseId}`)).toBe(true);
    expect(kv.store.has(`event:evt_1`)).toBe(true);
    expect(kv.store.has(`session:cs_1`)).toBe(true);
    const record = JSON.parse(kv.store.get(`license:${json.licenseId}`)!);
    expect(record.email).toBe("buyer@example.com");
    expect(kv.store.get(`download:${record.downloadToken}`)).toBe(json.licenseId);
  });

  it("rejects a bad signature with 400 and does not send email", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = checkoutEvent({ eventId: "evt_x", sessionId: "cs_x", email: "b@example.com" });
    const header = await makeStripeSigHeader(body + "tamper", SECRET, now);

    const res = await worker.fetch(stripeRequest(body, header), env);
    expect(res.status).toBe(400);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("is idempotent on duplicate event.id: second delivery emails 0 more times", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = checkoutEvent({ eventId: "evt_dup", sessionId: "cs_dup", email: "b@example.com" });
    const header = await makeStripeSigHeader(body, SECRET, now);

    const res1 = await worker.fetch(stripeRequest(body, header), env);
    expect(res1.status).toBe(200);
    const res2 = await worker.fetch(stripeRequest(body, header), env);
    expect(res2.status).toBe(200);
    expect((await res2.json() as any).duplicate).toBe(true);

    // Only the first delivery sent an email.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("converges on session.id: a retry with a NEW event id reuses the same license", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);

    const body1 = checkoutEvent({ eventId: "evt_a", sessionId: "cs_same", email: "b@example.com" });
    const res1 = await worker.fetch(
      stripeRequest(body1, await makeStripeSigHeader(body1, SECRET, now)),
      env,
    );
    const id1 = (await res1.json() as any).licenseId;

    const body2 = checkoutEvent({ eventId: "evt_b", sessionId: "cs_same", email: "b@example.com" });
    const res2 = await worker.fetch(
      stripeRequest(body2, await makeStripeSigHeader(body2, SECRET, now)),
      env,
    );
    const id2 = (await res2.json() as any).licenseId;

    expect(id2).toBe(id1); // same license, not a second key
  });

  it("returns 502 and does NOT mark the event processed when email delivery fails", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    // ZeptoMail returns 500 → email send fails.
    fetchMock.mockImplementation(async () => new Response("err", { status: 500 }));

    const body = checkoutEvent({ eventId: "evt_fail", sessionId: "cs_fail", email: "b@example.com" });
    const header = await makeStripeSigHeader(body, SECRET, now);
    const res = await worker.fetch(stripeRequest(body, header), env);

    expect(res.status).toBe(502);
    // Event NOT marked processed → Stripe retry will re-attempt delivery.
    expect(kv.store.has("event:evt_fail")).toBe(false);
  });

  it("acknowledges non-checkout events with 200 without emailing", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = JSON.stringify({ id: "evt_ping", type: "payment_intent.created", data: { object: {} } });
    const header = await makeStripeSigHeader(body, SECRET, now);
    const res = await worker.fetch(stripeRequest(body, header), env);
    expect(res.status).toBe(200);
    expect((await res.json() as any).ignored).toBe("payment_intent.created");
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

describe("POST /license/validate", () => {
  it("validates a freshly minted token as good and returns claims", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = checkoutEvent({ eventId: "evt_v", sessionId: "cs_v", email: "claims@example.com" });
    await worker.fetch(stripeRequest(body, await makeStripeSigHeader(body, SECRET, now)), env);

    const licenseId = (await kv.get("event:evt_v", "json")).licenseId;
    const record = await kv.get(`license:${licenseId}`, "json");

    const res = await worker.fetch(
      new Request("https://worker.example.com/license/validate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ license: record.license }),
      }),
      env,
    );
    expect(res.status).toBe(200);
    const json = (await res.json()) as any;
    expect(json.valid).toBe(true);
    expect(json.email).toBe("claims@example.com");
    expect(json.licenseId).toBe(licenseId);
    expect(typeof json.issued).toBe("number");
  });

  it("rejects a garbage token", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64 });
    const res = await worker.fetch(
      new Request("https://worker.example.com/license/validate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ license: "not.a.valid.token" }),
      }),
      env,
    );
    expect(res.status).toBe(200);
    expect((await res.json() as any).valid).toBe(false);
  });
});

describe("GET /download", () => {
  it("302-redirects to the installer for a valid token and increments count", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64, webhookSecret: SECRET });
    const now = Math.floor(Date.now() / 1000);
    const body = checkoutEvent({ eventId: "evt_d", sessionId: "cs_d", email: "d@example.com" });
    await worker.fetch(stripeRequest(body, await makeStripeSigHeader(body, SECRET, now)), env);

    const licenseId = (await kv.get("event:evt_d", "json")).licenseId;
    const record = await kv.get(`license:${licenseId}`, "json");

    const res = await worker.fetch(
      new Request(`https://worker.example.com/download?token=${record.downloadToken}`),
      env,
    );
    expect(res.status).toBe(302);
    expect(res.headers.get("Location")).toBe("https://downloads.example.com/rbxsync/latest");

    const after = await kv.get(`license:${licenseId}`, "json");
    expect(after.downloadCount).toBe(1);
  });

  it("returns 403 for an invalid token", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64 });
    const res = await worker.fetch(
      new Request("https://worker.example.com/download?token=nope"),
      env,
    );
    expect(res.status).toBe(403);
  });

  it("returns 403 for a missing token", async () => {
    const kv = new MockKV();
    const env = makeEnv(kv, { privateKeyB64 });
    const res = await worker.fetch(new Request("https://worker.example.com/download"), env);
    expect(res.status).toBe(403);
  });
});
