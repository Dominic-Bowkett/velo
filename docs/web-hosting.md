# Hosting Velo on your own server (web version)

The web version is a single Rust binary (`velo-server`) that serves both the
built frontend and the JSON API the browser talks to. Everything is stored in
one SQLite file — no separate database service.

> **Status:** Phase 2. The email transport, SQL gateway, web build, and static
> serving all work end-to-end. Per-user login/roles and the web OAuth redirect
> are Phase 3 — until then, treat a deployment as trusted/single-user and keep
> it behind your own access control (VPN, basic-auth proxy, etc.).

## 1. Build the two artifacts

On a build machine with Node + Rust:

```bash
# Frontend (produces ./dist — static files)
npm install
npm run build:web

# Server binary (produces ./src-tauri/target/release/velo-server)
cd src-tauri
cargo build -p velo-server --release
```

## 2. Upload to your server

Copy these to the server (anywhere, e.g. `/opt/velo`):

- `dist/`  → the built frontend
- `src-tauri/target/release/velo-server` → the binary

## 3. Run it

```bash
VELO_BIND=127.0.0.1:8080 \
VELO_DB=/opt/velo/velo.db \
VELO_STATIC_DIR=/opt/velo/dist \
/opt/velo/velo-server
```

Environment variables:

| Var | Default | Meaning |
|-----|---------|---------|
| `VELO_BIND` | `127.0.0.1:8080` | Address/port to listen on |
| `VELO_DB` | `velo.db` | SQLite file (created if missing) |
| `VELO_STATIC_DIR` | _(unset)_ | Directory of the built frontend; when set, the SPA is served with deep-link fallback |
| `VITE_API_BASE` | _(same origin)_ | Build-time only: set if the API lives on a different origin than the page |

Then open `http://your-server:8080/` in a browser.

## 4. Put it behind TLS (recommended)

Run `velo-server` bound to localhost and reverse-proxy it with nginx/Caddy so
the browser uses HTTPS. Example (Caddy):

```
mail.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

## Backups

Everything is in the single `VELO_DB` file. Stop the server (or use the SQLite
backup API) and copy `velo.db` (plus `velo.db-wal`/`velo.db-shm` if present).

## What works today vs. later phases

- ✅ Add IMAP/SMTP accounts, list folders, fetch/read messages, reply/send,
  archive/star/move, **move to another mailbox**, attachments — all over HTTP.
- ✅ SQLite (incl. FTS5 trigram search) via the SQL gateway.
- ⏳ Per-user login + admin/member roles + access enforcement — **Phase 3**.
- ⏳ Web OAuth (Gmail) redirect flow — **Phase 3** (IMAP/password accounts work now).
- ⏳ Server-side encryption of stored secrets — **Phase 3** (today the web build
  keeps the encryption key in the browser, fine for single-user testing).
