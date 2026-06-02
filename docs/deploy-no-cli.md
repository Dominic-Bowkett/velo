# Deploying Velo with little/no command line

**Important:** Velo's web server is a long-running program that opens IMAP/SMTP
connections. It **cannot** run on shared/cPanel hosting (including Hostinger
shared plans) — those only run PHP and kill background processes. You need a
host that can run a container or a long-lived process. The options below need
**no SSH and no command line** — you click through a web dashboard.

Your code is already on GitHub `main`, and the repo includes a `Dockerfile`, so
these hosts can build and run it automatically.

---

## Option 1 — Render.com (recommended, click-through)

1. Create a free account at https://render.com and connect your GitHub.
2. **New → Blueprint**, pick this repo. Render reads `render.yaml` and sets up
   the service + a 5 GB persistent disk at `/data` automatically.
3. Click **Apply** / **Create**. The first build takes ~5–10 min.
4. Once it's live you get a URL like `https://velo.onrender.com`. Open the
   service's **Environment** tab and set:
   - `VELO_PUBLIC_URL` = your Render URL (e.g. `https://velo.onrender.com`)
   - `VELO_ADMIN_EMAIL` = the email you want to log in with
   - `VELO_ADMIN_PASSWORD` = a strong password you choose
5. Save (it redeploys). Open the URL and **log in with the email/password you
   just set**. That's your admin account.
6. Go back to **Environment** and delete `VELO_ADMIN_EMAIL` /
   `VELO_ADMIN_PASSWORD` (they're only needed once).

## Option 2 — Railway.app (also click-through)

1. https://railway.app → sign in with GitHub → **New Project → Deploy from
   GitHub repo** → pick this repo. Railway auto-detects the `Dockerfile`.
2. Add a **Volume** mounted at `/data` (Railway dashboard → your service →
   Volumes) so data persists.
3. In **Variables**, set `VELO_PUBLIC_URL`, `VELO_ADMIN_EMAIL`,
   `VELO_ADMIN_PASSWORD` (same as above), then deploy.
4. Open the generated URL, log in with those credentials, then clear the two
   admin variables.

## Option 3 — Hostinger VPS (keep it under Hostinger)

Hostinger's **VPS** product (not shared hosting) can run this. Their VPS panel
has a Docker template and a browser terminal. If you go this route, tell me and
I'll give you the exact three lines to paste — no Linux knowledge needed.

---

## What your admin login is

There is **no default password.** On the first launch you set
`VELO_ADMIN_EMAIL` and `VELO_ADMIN_PASSWORD` to values **you choose**, and the
server creates your admin account from them. You then log in at your site with
exactly those. Pick a strong password — this account can see all mailboxes.

## After you're in

Log in as admin → **Admin** panel (Users icon, bottom-left) to:
- create user accounts (member/admin),
- provision each user's IMAP/SMTP mailbox (members can use but not edit them),
- set the display name and signature (global or per user).

New-mail email notifications work once at least one **admin** mailbox is
provisioned (it's the sender) and `VELO_PUBLIC_URL` is set.

See `docs/web-hosting.md` for the full environment-variable reference.
