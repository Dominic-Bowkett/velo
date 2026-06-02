import { describe, it, expect, afterEach } from "vitest";
import { readNotifyParam } from "./notifyDeepLink";

function setHash(hash: string) {
  window.location.hash = hash;
}

describe("readNotifyParam", () => {
  afterEach(() => {
    window.location.hash = "";
  });

  it("parses mailboxId:uid from a hash-router query", () => {
    setHash("#/mail/INBOX?notify=abc123:42");
    expect(readNotifyParam()).toEqual({ mailboxId: "abc123", uid: "42" });
  });

  it("returns null when no notify param is present", () => {
    setHash("#/mail/INBOX");
    expect(readNotifyParam()).toBeNull();
  });

  it("returns null for a malformed value", () => {
    setHash("#/mail/INBOX?notify=onlyonepart");
    expect(readNotifyParam()).toBeNull();
  });
});
