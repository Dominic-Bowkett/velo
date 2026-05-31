/**
 * Cross-account message move / assignment.
 *
 * Moves a thread from one account's mailbox into a DIFFERENT account's mailbox
 * (e.g. a support-desk triage flow: reassign a message from info@ to mark@).
 *
 * IMAP has no native cross-server move, so this works by:
 *   1. fetching the original raw RFC822 from the source account,
 *   2. appending it into the destination account's folder (headers, sender,
 *      date and attachments are all preserved because they live in the raw MIME),
 *   3. trashing the original from the source account — but ONLY after every
 *      append has succeeded, so a failure never loses the source message.
 *
 * Built on the transport-agnostic EmailProvider layer, so it works for any
 * provider combination (IMAP↔IMAP, IMAP↔Gmail, …).
 *
 * v1 is online-only: the append step talks to two live servers. Offline-queue
 * support (a `moveToAccount` op type) is a possible follow-up.
 */

import { getEmailProvider } from "./email/providerFactory";
import { getMessagesForThread } from "./db/messages";
import { trashThread } from "./emailActions";
import { useThreadStore } from "@/stores/threadStore";

/**
 * Encode a (possibly UTF-8) raw message string to base64url, mirroring the
 * base64UrlDecode used by the IMAP provider when reading raw messages.
 */
function base64UrlEncode(raw: string): string {
  const bytes = new TextEncoder().encode(raw);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

export interface CrossAccountMoveResult {
  /** Number of messages successfully appended to the destination. */
  moved: number;
}

/**
 * Move/assign all messages of a thread from `sourceAccountId` into
 * `destAccountId`'s `destFolderPath` (an IMAP folder path or Gmail label ID).
 *
 * Throws if the accounts are the same, the thread has no messages, or any
 * append fails (in which case the source is left untouched).
 */
export async function moveThreadToAccount(
  sourceAccountId: string,
  threadId: string,
  destAccountId: string,
  destFolderPath: string,
): Promise<CrossAccountMoveResult> {
  if (sourceAccountId === destAccountId) {
    throw new Error("Source and destination accounts are the same");
  }

  const messages = await getMessagesForThread(sourceAccountId, threadId);
  if (messages.length === 0) {
    throw new Error("No messages found for this thread");
  }

  const srcProvider = await getEmailProvider(sourceAccountId);
  const destProvider = await getEmailProvider(destAccountId);

  // 1. Fetch raw + append each message to the destination. If any append
  //    throws, we abort here and never delete from the source — at worst the
  //    destination gains a duplicate, but no message is lost.
  for (const msg of messages) {
    const raw = await srcProvider.fetchRawMessage(msg.id);
    const rawBase64Url = base64UrlEncode(raw);
    const flags = msg.is_read ? "(\\Seen)" : undefined;
    await destProvider.appendRawMessage(destFolderPath, rawBase64Url, flags);
  }

  // 2. Every append succeeded — remove the thread from the source account.
  await trashThread(
    sourceAccountId,
    threadId,
    messages.map((m) => m.id),
  );

  // 3. Reflect locally and trigger a sync so the destination picks it up.
  useThreadStore.getState().removeThread(threadId);
  window.dispatchEvent(new Event("velo-sync-done"));

  return { moved: messages.length };
}
