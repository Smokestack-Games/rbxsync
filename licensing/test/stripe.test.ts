import { describe, it, expect } from "vitest";
import { verifyStripeSignature, parseStripeSigHeader } from "../src/stripe";
import { makeStripeSigHeader } from "./helpers";

const SECRET = "whsec_test_secret";
const BODY = JSON.stringify({ id: "evt_1", type: "checkout.session.completed" });

describe("verifyStripeSignature", () => {
  it("accepts a valid signature within tolerance", async () => {
    const now = 1_700_000_000;
    const header = await makeStripeSigHeader(BODY, SECRET, now);
    const res = await verifyStripeSignature(BODY, header, SECRET, 300, now);
    expect(res.valid).toBe(true);
  });

  it("rejects a tampered body", async () => {
    const now = 1_700_000_000;
    const header = await makeStripeSigHeader(BODY, SECRET, now);
    const res = await verifyStripeSignature(BODY + "x", header, SECRET, 300, now);
    expect(res.valid).toBe(false);
    expect(res.reason).toBe("no matching signature");
  });

  it("rejects a tampered/forged signature", async () => {
    const now = 1_700_000_000;
    const header = `t=${now},v1=deadbeef${"0".repeat(56)}`;
    const res = await verifyStripeSignature(BODY, header, SECRET, 300, now);
    expect(res.valid).toBe(false);
  });

  it("rejects the wrong secret", async () => {
    const now = 1_700_000_000;
    const header = await makeStripeSigHeader(BODY, SECRET, now);
    const res = await verifyStripeSignature(BODY, header, "whsec_other", 300, now);
    expect(res.valid).toBe(false);
  });

  it("rejects a stale timestamp (> 5 min old)", async () => {
    const eventTime = 1_700_000_000;
    const header = await makeStripeSigHeader(BODY, SECRET, eventTime);
    const now = eventTime + 301; // 1s past tolerance
    const res = await verifyStripeSignature(BODY, header, SECRET, 300, now);
    expect(res.valid).toBe(false);
    expect(res.reason).toBe("timestamp outside tolerance");
  });

  it("rejects a future-dated timestamp beyond tolerance", async () => {
    const eventTime = 1_700_000_500;
    const header = await makeStripeSigHeader(BODY, SECRET, eventTime);
    const now = eventTime - 301;
    const res = await verifyStripeSignature(BODY, header, SECRET, 300, now);
    expect(res.valid).toBe(false);
  });

  it("accepts when ANY of multiple v1 signatures matches", async () => {
    const now = 1_700_000_000;
    const good = await makeStripeSigHeader(BODY, SECRET, now); // t=..,v1=<good>
    const goodSig = good.split("v1=")[1];
    const header = `t=${now},v1=${"0".repeat(64)},v1=${goodSig}`;
    const res = await verifyStripeSignature(BODY, header, SECRET, 300, now);
    expect(res.valid).toBe(true);
  });

  it("rejects a missing header", async () => {
    const res = await verifyStripeSignature(BODY, null, SECRET);
    expect(res.valid).toBe(false);
    expect(res.reason).toBe("missing signature header");
  });

  it("rejects a header with no timestamp", async () => {
    const res = await verifyStripeSignature(BODY, "v1=abc", SECRET);
    expect(res.valid).toBe(false);
    expect(res.reason).toBe("no timestamp in header");
  });
});

describe("parseStripeSigHeader", () => {
  it("parses timestamp and multiple v1 signatures", () => {
    const parsed = parseStripeSigHeader("t=123,v1=aaa,v1=bbb,v0=ignored");
    expect(parsed.timestamp).toBe(123);
    expect(parsed.signatures).toEqual(["aaa", "bbb"]);
  });
});
