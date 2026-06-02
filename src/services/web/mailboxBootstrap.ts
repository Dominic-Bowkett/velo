/**
 * Web-only: turn the server's provisioned mailbox list into local `accounts`
 * rows so the rest of the app (sync, lists, composer) works unchanged.
 *
 * Each mailbox becomes an IMAP account whose `mailbox_id` points back at the
 * server mailbox. IMAP/SMTP operations then send the `mailbox_id` (not
 * credentials) — the server resolves the password and enforces ownership.
 * The local password column is left empty: members never hold credentials.
 */

import { getDb } from "../db/connection";
import { listMyMailboxes, type Mailbox } from "../auth/authService";

/** Map a server mailbox security string to the DB-stored value. */
function dbSecurity(s: string): string {
  return s; // server already uses tls/starttls/none, same as DB config
}

/**
 * Upsert an accounts row for each provisioned mailbox, and remove local
 * accounts whose mailbox no longer exists on the server. Returns the account
 * ids (one per mailbox) so startup can initialise providers for them.
 */
export async function syncProvisionedMailboxes(): Promise<string[]> {
  const mailboxes = await listMyMailboxes();
  const db = await getDb();

  const ids: string[] = [];
  for (const m of mailboxes) {
    const accountId = `mbx-${m.id}`;
    ids.push(accountId);
    await upsertMailboxAccount(db, accountId, m);
  }

  // Remove any previously-bootstrapped accounts whose mailbox is gone.
  const keep = new Set(ids);
  const existing = await db.select<{ id: string }[]>(
    "SELECT id FROM accounts WHERE mailbox_id IS NOT NULL",
  );
  for (const row of existing) {
    if (!keep.has(row.id)) {
      await db.execute("DELETE FROM accounts WHERE id = $1", [row.id]);
    }
  }

  return ids;
}

async function upsertMailboxAccount(
  db: Awaited<ReturnType<typeof getDb>>,
  accountId: string,
  m: Mailbox,
): Promise<void> {
  // accounts.email has a UNIQUE constraint. A stale row may exist with this
  // email but a DIFFERENT id (e.g. a previously-bootstrapped account whose
  // mailbox id changed, or a manually-added account). Remove those first so the
  // upsert below doesn't hit "UNIQUE constraint failed: accounts.email".
  await db.execute("DELETE FROM accounts WHERE email = $1 AND id != $2", [
    m.email,
    accountId,
  ]);

  await db.execute(
    `INSERT INTO accounts (
        id, email, display_name, avatar_url, access_token, refresh_token,
        provider, imap_host, imap_port, imap_security, smtp_host, smtp_port,
        smtp_security, auth_method, imap_password, imap_username,
        accept_invalid_certs, mailbox_id, is_active
     ) VALUES ($1,$2,$3,NULL,NULL,NULL,'imap',$4,$5,$6,$7,$8,$9,'password','',$10,$11,$12,1)
     ON CONFLICT(id) DO UPDATE SET
        email = $2, display_name = $3, imap_host = $4, imap_port = $5,
        imap_security = $6, smtp_host = $7, smtp_port = $8, smtp_security = $9,
        imap_username = $10, accept_invalid_certs = $11, mailbox_id = $12,
        updated_at = unixepoch()`,
    [
      accountId,
      m.email,
      m.displayName,
      m.imapHost,
      m.imapPort,
      dbSecurity(m.imapSecurity),
      m.smtpHost,
      m.smtpPort,
      dbSecurity(m.smtpSecurity),
      m.username,
      m.acceptInvalidCerts ? 1 : 0,
      m.id,
    ],
  );
}
