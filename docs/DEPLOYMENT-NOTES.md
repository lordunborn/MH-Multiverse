# Deployment notes (launcher file, content gate, TLS monitor)

Written 2026-10-24. These are facts and decisions from the launcher and website setup. No secret values are recorded here.

## `Mho.Deployment.json` (the launcher's deployment file)

- The file holds `UpdateBaseUrl`, `UpdatePublicKey`, `ContentTokenSecret` and the `Servers` list. It is handed to players with the launcher
  (it is never inside the public GitHub release) and may be hosted on the website for download.
- The filled-in production copy is kept outside every repo, in `C:\Users\lordu\MHOLauncher-Private\upload-ready\site\`. Never commit it.
- `UpdateBaseUrl` is `https://mhorenaissance.com/client/` (assumed layout: the VPS `htdocs\client` folder). Check that URL in a browser before publishing the file.
- Servers stay on the IP over http (`http://147.93.146.38/SiteConfig153.xml`), which is the address that works today.

## `ContentTokenSecret`: what it is, and why it is fine to publish

- It is the key for a small signed token: `mho1.<contentVersion>.<first 32 hex of HMAC-SHA256(secret, "mho1.<contentVersion>")>`.
  The launcher passes the token to the game client as `-twofactorauthname`, and the server checks it at login.
- It only drives the "patch required" check that keeps out-of-date clients away. The server code itself says it "keeps stale clients out and
  casual guessing off; it is not strong protection, because the secret ships with the launcher".
- It does not protect accounts, passwords, downloads or any admin action. The update files and manifest are public by design.
  The manifest is protected by a separate signature; `UpdatePublicKey` is a public key (safe to publish) and the signing private key
  (`mho-private.pem`) is never in the file.
- **The live server enforces the gate: `RequiredContentVersion=1`** (the local test bed uses `0`, because server and client are synced on save).
  So the launcher file must keep the secret, otherwise no player could log in ("patch required").
- If the secret is public, the only consequence is that someone could forge a token and get past the version check with an old client.
  Do not reuse the secret anywhere else. To rotate it, change it in the launcher file and the live `Config.ini` together and restart the server.

## Planned: TLS certificate monitor for the Apache MH-Multiverse manages

Requested while setting up the website. Status: not started. Reading done first (2026-10-24):

- Apache is started by `start_apache` and stopped by `stop_apache` in `src-tauri/src/server.rs`
  (`...\Apache24\bin\httpd.exe`, env `APACHE_SERVER_ROOT`, own Job Object). The game server has separate commands and must not change.
- Settings live in `AppConfig` (`src-tauri/src/config.rs`, JSON via `load_config` / `save_config`) and the frontend `appConfig` store.
- There are no Rust tests yet; new ones go in the new module.

Requirements:

1. Settings (persisted, no hard-coded paths): certificate file path (empty = off), warning threshold in days (default 14), auto-apply (default off).
2. Read only the public certificate file; never read, log, copy or display the private key.
3. Show expiry date, days remaining, and whether Apache runs the latest cert on disk (SHA-256 fingerprint recorded when Apache starts, compared with the file).
4. Check on startup and about hourly. Show "New certificate available. Restart Apache to apply." when they differ;
   make it prominent when days remaining < threshold and Apache is still on the old cert.
5. "Restart Apache now": run the equivalent of `httpd.exe -t` (same env var) first; abort with a UI error and leave Apache untouched if it fails.
6. Auto-apply (when on): restart only Apache, never the game server, after the same config test passes; log it.
7. Do not change how the game server is started, stopped or restarted.
8. Missing or unreadable cert: show "certificate not found", no crash.
9. Unit tests for the fingerprint comparison and the days-remaining logic.

Context: a win-acme scheduled task renews the Let's Encrypt certificate about every two months and writes PEM files
(currently `C:\MHEmu Server\certs\mhorenaissance\mhorenaissance.com-chain.pem`). Apache only reads certificates at startup, so a long-running
Apache keeps serving the old one until it expires.

Files expected to change: new `src-tauri/src/tls.rs`; `server.rs`, `config.rs`, `lib.rs`, `Cargo.toml` (sha2, an X.509 parser);
`src/lib/store.ts`, `src/lib/serverEvents.ts`; `src/components/ServerPanel.svelte`, `src/components/ConfigPanel.svelte`.
