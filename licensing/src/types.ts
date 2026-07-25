/** Environment bindings for the RbxSync license-fulfillment Worker. */
export interface Env {
  // ─── KV ────────────────────────────────────────────────────────────────────
  /** KV namespace storing licenses, download tokens, and processed Stripe event ids. */
  LICENSES: KVNamespace;

  // ─── Secrets (wrangler secret put …) ─────────────────────────────────────────
  /** Stripe webhook signing secret (whsec_...). */
  STRIPE_WEBHOOK_SECRET: string;
  /** Stripe secret key — optional; only needed if you expand the session server-side. */
  STRIPE_SECRET_KEY?: string;
  /** ZeptoMail API token (send token). Stored with or without the "Zoho-enczapikey " prefix. */
  ZEPTOMAIL_TOKEN: string;
  /** Ed25519 private key, PKCS8 DER, base64-encoded. */
  LICENSE_SIGNING_PRIVATE_KEY: string;

  // ─── Vars (wrangler.toml [vars]) ─────────────────────────────────────────────
  /** From address for license emails, e.g. licenses@smokestackgames.com. */
  FROM_EMAIL: string;
  /** Base URL the /download endpoint redirects to (Vercel Blob / R2 installer root). */
  INSTALLER_BASE_URL: string;
  /** Public origin of THIS worker, e.g. https://rbxsync-licensing.smokestackgames.workers.dev.
   *  Used to build the download link in the email. */
  WORKER_URL: string;
}
