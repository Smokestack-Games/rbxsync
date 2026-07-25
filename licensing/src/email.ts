/**
 * Transactional email via ZeptoMail (Zoho) HTTP API.
 *
 * POST https://api.zeptomail.com/v1.1/email
 *   Authorization: Zoho-enczapikey <ZEPTOMAIL_TOKEN>
 *
 * The license-delivery email carries the license key, the gated download link,
 * and short numbered install steps.
 */

const ZEPTOMAIL_ENDPOINT = "https://api.zeptomail.com/v1.1/email";

export interface LicenseEmailParams {
  toEmail: string;
  licenseKey: string;
  downloadUrl: string;
}

export interface SendResult {
  ok: boolean;
  status: number;
  body: string;
}

/**
 * Send the license-delivery email. Returns { ok:false } (never throws) so the
 * caller can decide whether to fail the webhook (→ Stripe retry) or log & move on.
 */
export async function sendLicenseEmail(
  params: LicenseEmailParams,
  env: { ZEPTOMAIL_TOKEN: string; FROM_EMAIL: string },
): Promise<SendResult> {
  const token = env.ZEPTOMAIL_TOKEN.startsWith("Zoho-enczapikey")
    ? env.ZEPTOMAIL_TOKEN
    : `Zoho-enczapikey ${env.ZEPTOMAIL_TOKEN}`;

  const payload = {
    from: { address: env.FROM_EMAIL, name: "RbxSync" },
    to: [{ email_address: { address: params.toEmail } }],
    subject: "Your RbxSync license",
    htmlbody: renderLicenseEmailHtml(params),
    textbody: renderLicenseEmailText(params),
  };

  try {
    const res = await fetch(ZEPTOMAIL_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: token,
      },
      body: JSON.stringify(payload),
    });
    const body = await res.text();
    return { ok: res.ok, status: res.status, body };
  } catch (err) {
    return { ok: false, status: 0, body: String(err) };
  }
}

// ─── Templates ───────────────────────────────────────────────────────────────

export function renderLicenseEmailText(p: LicenseEmailParams): string {
  return [
    "Thanks for buying RbxSync!",
    "",
    "Your license key:",
    p.licenseKey,
    "",
    "Download RbxSync:",
    p.downloadUrl,
    "",
    "Get set up in a minute:",
    "1. Install the RbxSync VS Code extension.",
    "2. Paste your license key when prompted.",
    "3. RbxSync downloads and sets itself up automatically.",
    "4. Install the Roblox Studio plugin.",
    "5. Connect Studio to VS Code and start syncing.",
    "",
    "Questions? Just reply to this email.",
    "— The RbxSync team",
  ].join("\n");
}

export function renderLicenseEmailHtml(p: LicenseEmailParams): string {
  // Inline styles only — email clients strip <style> blocks unpredictably.
  return `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
<body style="margin:0;padding:0;background:#faf9f5;font-family:-apple-system,Segoe UI,Roboto,Helvetica,Arial,sans-serif;color:#141413;">
  <div style="max-width:520px;margin:0 auto;padding:32px 24px;">
    <h1 style="font-size:22px;margin:0 0 8px;">Thanks for buying RbxSync! 🎉</h1>
    <p style="font-size:15px;line-height:1.5;color:#5c5c5a;margin:0 0 24px;">
      You're all set. Here's your license key and download link.
    </p>

    <div style="background:#ffffff;border:1px solid #e5e4df;border-radius:12px;padding:20px;margin-bottom:16px;">
      <p style="font-size:12px;text-transform:uppercase;letter-spacing:0.06em;color:#8c8c8a;margin:0 0 8px;">Your license key</p>
      <code style="display:block;word-break:break-all;font-size:13px;line-height:1.5;background:#f5f4ef;border:1px solid #e5e4df;border-radius:8px;padding:12px;color:#141413;">${escapeHtml(
        p.licenseKey,
      )}</code>
    </div>

    <a href="${escapeAttr(p.downloadUrl)}"
       style="display:block;text-align:center;background:#D97757;color:#ffffff;text-decoration:none;font-weight:600;font-size:15px;padding:14px 24px;border-radius:12px;margin-bottom:24px;">
      Download RbxSync
    </a>

    <div style="background:#ffffff;border:1px solid #e5e4df;border-radius:12px;padding:20px;margin-bottom:24px;">
      <p style="font-size:14px;font-weight:600;margin:0 0 12px;">Get set up in a minute</p>
      <ol style="margin:0;padding-left:20px;font-size:14px;line-height:1.7;color:#3d3d3b;">
        <li>Install the RbxSync VS Code extension.</li>
        <li>Paste your license key when prompted.</li>
        <li>RbxSync downloads and sets itself up automatically.</li>
        <li>Install the Roblox Studio plugin.</li>
        <li>Connect Studio to VS Code and start syncing.</li>
      </ol>
    </div>

    <p style="font-size:13px;color:#8c8c8a;line-height:1.5;margin:0;">
      Questions? Just reply to this email — we read every one.<br>
      &mdash; The RbxSync team
    </p>
  </div>
</body>
</html>`;
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function escapeAttr(s: string): string {
  return escapeHtml(s).replace(/"/g, "&quot;");
}
