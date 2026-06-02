/**
 * Web-only: handle the `?notify=<mailboxId>:<uid>` deep link from a new-mail
 * notification email. Resolves the message to its thread and opens it.
 *
 * The notification link is `<base>/#/mail/INBOX?notify=<mailboxId>:<uid>`. The
 * corresponding local IMAP message id is `imap-mbx-<mailboxId>-INBOX-<uid>`
 * (web accounts are bootstrapped with id `mbx-<mailboxId>`).
 */

import { getDb } from "../db/connection";
import { navigateToThread } from "../../router/navigate";
import { useAccountStore } from "../../stores/accountStore";

/** Parse the notify param from the current URL (hash or search). */
export function readNotifyParam(): { mailboxId: string; uid: string } | null {
  // Hash router: the query lives after the in-hash path, e.g. #/mail/INBOX?notify=ab:12
  const hash = window.location.hash;
  const qIndex = hash.indexOf("?");
  const search =
    qIndex >= 0 ? hash.slice(qIndex + 1) : window.location.search.replace(/^\?/, "");
  const params = new URLSearchParams(search);
  const value = params.get("notify");
  if (!value) return null;
  const [mailboxId, uid] = value.split(":");
  if (!mailboxId || !uid) return null;
  return { mailboxId, uid };
}

/**
 * If a notify deep link is present, find the message's thread and open it.
 * Safe to call after the initial sync. Returns true if it navigated.
 */
export async function handleNotifyDeepLink(): Promise<boolean> {
  const parsed = readNotifyParam();
  if (!parsed) return false;

  const accountId = `mbx-${parsed.mailboxId}`;
  const messageId = `imap-${accountId}-INBOX-${parsed.uid}`;

  const db = await getDb();
  const rows = await db.select<{ thread_id: string }[]>(
    "SELECT thread_id FROM messages WHERE account_id = $1 AND id = $2 LIMIT 1",
    [accountId, messageId],
  );
  const threadId = rows[0]?.thread_id;
  if (!threadId) return false;

  useAccountStore.getState().setActiveAccount(accountId);
  navigateToThread(threadId);
  return true;
}
