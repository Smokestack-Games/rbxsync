# RbxSync License Fulfillment

A Cloudflare Worker that turns a Stripe payment into a delivered RbxSync license.

**Flow:** buyer pays via Stripe Checkout → Stripe fires `checkout.session.completed`
→ this Worker mints an **Ed25519-signed license token** + a gated download token,
stores both in KV, and emails the buyer their key + download link via ZeptoMail.
The CLI verifies the license **offline** against an embedded public key — no
phone-home required.

> **Nothing here is live.** The maintainer wires the real secrets, creates the KV
> namespace, and reviews before `wrangler deploy`. No key material is committed.

---

## Endpoints

| Method + path | Purpose |
|---|---|
| `POST /webhook/stripe` | Verify the Stripe signature, and on `checkout.session.completed` mint + email a license. Returns `200` on success/duplicate/ignored, `400` on bad signature, `502` if the email send fails (so Stripe retries). |
| `GET /download?token=<downloadToken>` | `302`-redirect to the gated installer (`INSTALLER_BASE_URL`). Increments a download counter (soft cap 25, then still serves but logs). `403` on unknown/missing token. |
| `POST /license/validate` | Body `{ "license": "<token>" }`. Verifies the Ed25519 signature and returns `{ valid, email?, licenseId?, issued? }`. Optional online check; the CLI mainly verifies offline. |
| `GET /health` | `{ ok: true }`. |

---

## License token spec (implement this exactly in the Rust CLI)

A license is a compact signed token:

```
token = base64url(payloadJSON) + "." + base64url(ed25519Signature)
```

- **`payloadJSON`** is the UTF-8 JSON encoding of:

  ```json
  {
    "v": 1,
    "product": "rbxsync",
    "plan": "lifetime-v1",
    "email": "buyer@example.com",
    "licenseId": "<uuid>",
    "issued": 1700000000
  }
  ```

  `issued` is unix **seconds**.

- **base64url** is RFC 4648 §5 (URL-safe alphabet `-` / `_`), **unpadded** (no `=`).

- **The signature is computed over the UTF-8 bytes of the ENCODED payload segment**
  — i.e. the ASCII bytes of `base64url(payloadJSON)`, the substring *before* the `.`.
  It is **not** computed over the decoded JSON. This is the JWT-style convention and
  it sidesteps JSON canonicalization entirely: the CLI never re-serializes anything.

### CLI verification algorithm

1. Split the token on the single `.` into `encodedPayload` and `encodedSig`.
   Reject if there isn't exactly one `.`, or either half is empty.
2. `sig = base64url_decode(encodedSig)` (64 bytes).
3. Verify the Ed25519 signature `sig` over the **ASCII bytes of `encodedPayload`**
   using the embedded public key.
4. If valid, `payload = json_decode(base64url_decode(encodedPayload))`.
5. Reject unless `payload.v == 1` and `payload.product == "rbxsync"`.

The public key is the **raw 32 bytes** emitted by `scripts/gen-keys.mjs`. In Rust with
`ed25519-dalek`:

```rust
// From licensing/keys/public-key.rs (generated — paste it in):
pub const LICENSE_PUBLIC_KEY: [u8; 32] = [/* 32 bytes */];

use ed25519_dalek::{Signature, VerifyingKey, Verifier};

let vk = VerifyingKey::from_bytes(&LICENSE_PUBLIC_KEY)?;
let sig = Signature::from_slice(&sig_bytes)?;
vk.verify(encoded_payload.as_bytes(), &sig)?; // encoded_payload = the pre-"." string
```

> Wiring this const into the CLI is a **separate follow-up task**. This service only
> produces the token and the public key; it does not touch the Rust crates.

---

## One-time setup (maintainer runbook)

Everything runs from the `licensing/` directory.

```bash
cd licensing
npm install
```

### 1. Generate the signing keypair

```bash
node scripts/gen-keys.mjs
```

This:
- Writes the **public** key (safe to commit) to `keys/public-key.txt` (base64, raw 32
  bytes) and `keys/public-key.rs` (a ready-to-paste Rust const for the CLI).
- **Prints** the private key + the exact `wrangler secret put` command to your
  terminal only. The private key is **never** written to any file.

Keep that terminal output until you've set the secret in step 4. If you lose the
private key, re-run gen-keys and re-ship the CLI with the new public key (old
licenses stop verifying).

### 2. Create the KV namespace

```bash
wrangler kv namespace create LICENSES
```

Paste the returned `id` into `wrangler.toml` under the `[[kv_namespaces]]` block
(`binding = "LICENSES"`).

### 3. Set the plain vars

Edit `wrangler.toml` `[vars]`:
- `FROM_EMAIL` — a verified sender on your ZeptoMail domain (e.g. `licenses@smokestackgames.com`).
- `INSTALLER_BASE_URL` — where `/download` redirects (Vercel Blob / R2 installer URL). Placeholder for now.
- `WORKER_URL` — set to the deployed worker origin (fill in after the first deploy, then redeploy).

### 4. Set the secrets

Each of these is set with `wrangler secret put <NAME>` and pasted when prompted —
**never** written into `wrangler.toml` or any committed file.

```bash
wrangler secret put LICENSE_SIGNING_PRIVATE_KEY   # base64 PKCS8 printed by gen-keys.mjs
wrangler secret put STRIPE_WEBHOOK_SECRET         # whsec_... (from step 6)
wrangler secret put ZEPTOMAIL_TOKEN               # ZeptoMail send token
# Optional — only if you later expand the checkout session server-side:
wrangler secret put STRIPE_SECRET_KEY             # sk_live_... / sk_test_...
```

### 5. Deploy

```bash
wrangler deploy
```

Note the deployed URL, put it in `WORKER_URL` in `wrangler.toml`, and `wrangler deploy`
again so the download links in emails point at the right origin.

### 6. Add the Stripe webhook

In the Stripe Dashboard → **Developers → Webhooks → Add endpoint**:
- **Endpoint URL:** `<deployed-worker-url>/webhook/stripe`
- **Events to send:** select **`checkout.session.completed`** (only).
- After creating it, copy the **Signing secret** (`whsec_...`) and set it as
  `STRIPE_WEBHOOK_SECRET` (step 4). Redeploy if you set it after the first deploy.

### 7. Enable ZeptoMail

- In ZeptoMail (Zoho), **verify the `smokestackgames.com` domain** (SPF + DKIM) so
  mail from `FROM_EMAIL` is deliverable.
- Create/find a **Send Mail token** and set it as `ZEPTOMAIL_TOKEN`. The Worker adds
  the `Zoho-enczapikey ` prefix automatically if you omit it.

---

## Secrets & config reference

| Name | Kind | Where set | Purpose |
|---|---|---|---|
| `LICENSE_SIGNING_PRIVATE_KEY` | secret | `wrangler secret put` | Ed25519 private key, base64 PKCS8. Signs licenses. |
| `STRIPE_WEBHOOK_SECRET` | secret | `wrangler secret put` | `whsec_...`; verifies webhook signatures. |
| `ZEPTOMAIL_TOKEN` | secret | `wrangler secret put` | ZeptoMail send token. |
| `STRIPE_SECRET_KEY` | secret (optional) | `wrangler secret put` | Only if expanding the session server-side. |
| `FROM_EMAIL` | var | `wrangler.toml` | License email From address. |
| `INSTALLER_BASE_URL` | var | `wrangler.toml` | `/download` redirect target. |
| `WORKER_URL` | var | `wrangler.toml` | This worker's public origin; builds email download links. |
| `LICENSES` | KV binding | `wrangler.toml` | Licenses, download tokens, processed event ids. |

Local development uses `licensing/.dev.vars` (gitignored) instead of real secrets —
copy `.dev.vars.example` to `.dev.vars` and fill it in. **Only `.dev.vars.example`
is committed.**

---

## KV layout

| Key | Value |
|---|---|
| `license:<licenseId>` | JSON `{ licenseId, email, license, downloadToken, created, downloadCount, stripeSessionId }` |
| `download:<downloadToken>` | `<licenseId>` (pointer) |
| `session:<stripeSessionId>` | `<licenseId>` (retry convergence) |
| `event:<stripeEventId>` | processed-event marker (idempotency) |

---

## Idempotency & delivery guarantees

- **Duplicate webhook deliveries** are deduped on Stripe's `event.id`.
- **Retries with a new event id but the same checkout session** converge on the
  existing license (keyed by `session.id`) — a buyer never gets two different keys.
- The email is **awaited**. If ZeptoMail fails, the Worker returns `502` and does
  **not** mark the event processed, so Stripe retries. The worst case is a duplicate
  email, never a paid-but-no-license.

---

## Tests

```bash
npm test        # vitest, runs in Node
npm run typecheck
```

Covers Stripe signature verification (valid / tampered / wrong-secret / stale /
multi-`v1`), the license sign→verify round-trip (incl. wrong-key and tampered-payload
rejection), `/license/validate` good/bad, `/download` valid/invalid, and webhook
idempotency + session convergence + the email-failure `502` path. Test keys are
ephemeral (generated per run), never committed.

### Verifying Ed25519 on the real runtime (workerd)

Node's Web Crypto and the Workers runtime (workerd) are different engines. The unit
tests run in Node; before deploy, smoke-test the crypto on workerd:

```bash
cp .dev.vars.example .dev.vars      # fill LICENSE_SIGNING_PRIVATE_KEY with a gen-keys value
# temporarily give [[kv_namespaces]].id any non-empty string for local dev
wrangler dev --local

# in another shell:
curl -s localhost:8787/health
curl -s -X POST localhost:8787/license/validate \
  -H 'Content-Type: application/json' -d '{"license":"<a-signed-token>"}'
```

This service was smoke-tested this way: workerd correctly signs (`/webhook/stripe`),
verifies (`/license/validate`), and derives the public key from the PKCS8 private key,
and tokens signed in Node verify on workerd (cross-runtime interop confirmed).
