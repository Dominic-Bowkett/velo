import { describe, it, expect, beforeEach, vi } from "vitest";

const execute = vi.fn();
const select = vi.fn();
const listMyMailboxes = vi.fn();

vi.mock("../db/connection", () => ({
  getDb: () => Promise.resolve({ execute, select }),
}));

vi.mock("../auth/authService", () => ({
  listMyMailboxes: () => listMyMailboxes(),
}));

import { syncProvisionedMailboxes } from "./mailboxBootstrap";

const sampleMailbox = {
  id: "mb1",
  ownerUserId: "u1",
  email: "test@example.com",
  displayName: "Test",
  imapHost: "imap.example.com",
  imapPort: 993,
  imapSecurity: "tls",
  smtpHost: "smtp.example.com",
  smtpPort: 465,
  smtpSecurity: "tls",
  username: null,
  acceptInvalidCerts: false,
};

describe("syncProvisionedMailboxes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    execute.mockResolvedValue({ rowsAffected: 1 });
    select.mockResolvedValue([]); // no pre-existing bootstrapped accounts
  });

  it("deletes any account sharing the email but a different id before upserting", async () => {
    listMyMailboxes.mockResolvedValue([sampleMailbox]);

    await syncProvisionedMailboxes();

    // First execute call must be the email-dedup DELETE, guarding the UNIQUE
    // constraint on accounts.email.
    const firstCall = execute.mock.calls[0];
    expect(firstCall[0]).toContain("DELETE FROM accounts WHERE email = $1 AND id != $2");
    expect(firstCall[1]).toEqual(["test@example.com", "mbx-mb1"]);
  });

  it("returns one account id per mailbox", async () => {
    listMyMailboxes.mockResolvedValue([sampleMailbox]);
    const ids = await syncProvisionedMailboxes();
    expect(ids).toEqual(["mbx-mb1"]);
  });

  it("removes previously-bootstrapped accounts whose mailbox is gone", async () => {
    listMyMailboxes.mockResolvedValue([sampleMailbox]);
    // An old bootstrapped account that no longer corresponds to a mailbox.
    select.mockResolvedValue([{ id: "mbx-old" }, { id: "mbx-mb1" }]);

    await syncProvisionedMailboxes();

    const deletedOld = execute.mock.calls.some(
      (c) => c[0] === "DELETE FROM accounts WHERE id = $1" && c[1]?.[0] === "mbx-old",
    );
    expect(deletedOld).toBe(true);
  });
});
