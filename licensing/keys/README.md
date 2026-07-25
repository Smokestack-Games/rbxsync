# keys/

`scripts/gen-keys.mjs` writes the **public** key here:

- `public-key.txt` — base64 of the raw 32 public-key bytes.
- `public-key.rs` — a ready-to-paste Rust const (`LICENSE_PUBLIC_KEY: [u8; 32]`) for the CLI.

Both are **safe to commit** — they contain only the public key.

The **private** key is never written to disk. `gen-keys.mjs` prints it to your
terminal once, for you to paste into `wrangler secret put LICENSE_SIGNING_PRIVATE_KEY`.
`.gitignore` guards against any private-key material landing in this folder.
