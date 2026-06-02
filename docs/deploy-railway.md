# Deploy Velo on Railway (no command line)

Railway builds this repo's `Dockerfile` and runs the server for you. Everything
below is done in the Railway web dashboard — no SSH, no terminal.

## 1. Create the project

1. Go to https://railway.app and sign in with GitHub.
2. **New Project → Deploy from GitHub repo** → choose your `velo` repo.
3. Railway detects the `Dockerfile` (and `railway.json`) and starts the first
   build. It takes ~5–10 minutes. Let it finish.

## 2. Add a persistent volume (so data survives restarts)

1. Open your service → **Variables**/**Settings** area → **Volumes** → **New Volume**.
2. Mount path: **`/data`**  (this is where users, mailboxes, and mail are stored).
3. Save. Railway will redeploy with the volume attached.

> Without this volume your data resets on each redeploy — don't skip it.

## 3. Generate the public URL

1. Service → **Settings → Networking → Generate Domain**.
2. You'll get something like `https://velo-production-xxxx.up.railway.app`.
   Copy it.

## 4. Set the first-run variables

Service → **Variables** → add these three (then **Deploy**):

| Variable | Value |
|----------|-------|
| `VELO_PUBLIC_URL` | your Railway URL from step 3 (e.g. `https://velo-production-xxxx.up.railway.app`) |
| `VELO_ADMIN_EMAIL` | the email you want to log in with (your choice) |
| `VELO_ADMIN_PASSWORD` | a strong password you choose |

Railway sets `PORT` automatically; the server binds to it — you don't configure
the port yourself.

## 5. Log in

Open your Railway URL in a browser → sign in with the **email and password you
set in step 4**. That's your admin account (there is no default password — it's
created from those variables on first launch).

## 6. Remove the admin bootstrap variables

Back in **Variables**, delete `VELO_ADMIN_EMAIL` and `VELO_ADMIN_PASSWORD`.
They're only needed for the very first launch; your admin account already exists
in the database now.

## 7. Set up your mailboxes

As admin → **Admin** panel (Users icon, bottom-left of the sidebar):
- **Create users** (member or admin).
- **Provision each user's IMAP/SMTP mailbox** — enter host/port/password.
  Members can use these but can't see or edit the credentials.
- Set the **display name** and **signature** (global, or per user).

New-mail email notifications start working once you've provisioned at least one
**admin** mailbox (it's the "from" sender) — Velo will email each user a link
that opens new messages directly in the browser.

---

### Updating later

Push to `main` (or merge a PR). Railway auto-redeploys from GitHub. Your `/data`
volume — users, mailboxes, mail — is preserved across deploys.

### Notes

- Use real IMAP/SMTP details from your mail provider (e.g. Fastmail/your host's
  mail server). Gmail isn't supported on the web build (no OAuth) — by design.
- Railway's free trial may sleep/limit usage; a small paid plan keeps the IMAP
  poller and notifications running continuously.
