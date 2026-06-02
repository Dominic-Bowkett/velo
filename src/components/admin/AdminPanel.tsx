import { useEffect, useState, useCallback } from "react";
import {
  listUsers,
  createUser,
  deleteUser,
  listAllMailboxes,
  createMailbox,
  deleteMailbox,
  type AuthUser,
  type Mailbox,
  type NewMailbox,
} from "../../services/auth/authService";

/**
 * Admin-only management panel: create users (admin/member) and provision their
 * IMAP/SMTP mailboxes. Members never see this — it's gated on role === "admin"
 * by the caller and by the server (admin routes return 403 otherwise).
 *
 * Mailbox passwords are write-only: entered here, stored server-side encrypted,
 * and never returned to any browser.
 */
export function AdminPanel() {
  const [users, setUsers] = useState<AuthUser[]>([]);
  const [mailboxes, setMailboxes] = useState<Mailbox[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [u, m] = await Promise.all([listUsers(), listAllMailboxes()]);
      setUsers(u);
      setMailboxes(m);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-8">
      <h1 className="text-xl font-semibold text-text-primary">Admin</h1>
      {error && <div className="text-sm text-danger">{error}</div>}

      <UsersSection users={users} onChange={refresh} />
      <MailboxesSection
        users={users}
        mailboxes={mailboxes}
        onChange={refresh}
      />
    </div>
  );
}

function UsersSection({
  users,
  onChange,
}: {
  users: AuthUser[];
  onChange: () => void;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState<"member" | "admin">("member");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr(null);
    setBusy(true);
    try {
      await createUser({ email, password, role });
      setEmail("");
      setPassword("");
      setRole("member");
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to create user");
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="space-y-3">
      <h2 className="text-sm font-medium text-text-primary uppercase tracking-wide">
        Users
      </h2>
      <div className="rounded-lg border border-border-secondary divide-y divide-border-secondary">
        {users.map((u) => (
          <div
            key={u.id}
            className="flex items-center justify-between px-3 py-2 text-sm"
          >
            <div>
              <span className="text-text-primary">{u.email}</span>
              <span className="ml-2 text-[10px] uppercase tracking-wider text-text-tertiary">
                {u.role}
              </span>
            </div>
            {u.role !== "admin" && (
              <button
                className="text-xs text-danger hover:underline"
                onClick={async () => {
                  await deleteUser(u.id);
                  onChange();
                }}
              >
                Remove
              </button>
            )}
          </div>
        ))}
      </div>

      <form onSubmit={add} className="flex flex-wrap items-end gap-2">
        <input
          type="email"
          required
          placeholder="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          className="flex-1 min-w-[180px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
        />
        <input
          type="password"
          required
          placeholder="password (min 8)"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          className="flex-1 min-w-[160px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
        />
        <select
          value={role}
          onChange={(e) => setRole(e.target.value as "member" | "admin")}
          className="px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary"
        >
          <option value="member">Member</option>
          <option value="admin">Admin</option>
        </select>
        <button
          type="submit"
          disabled={busy}
          className="px-3 py-1.5 rounded bg-accent text-white text-sm hover:bg-accent-hover disabled:opacity-50"
        >
          Add user
        </button>
      </form>
      {err && <div className="text-xs text-danger">{err}</div>}
    </section>
  );
}

const SECURITY_OPTIONS = [
  { label: "SSL/TLS", value: "tls" },
  { label: "STARTTLS", value: "starttls" },
  { label: "None", value: "none" },
];

function MailboxesSection({
  users,
  mailboxes,
  onChange,
}: {
  users: AuthUser[];
  mailboxes: Mailbox[];
  onChange: () => void;
}) {
  const [form, setForm] = useState<NewMailbox>(emptyMailbox());
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const userEmail = (id: string) =>
    users.find((u) => u.id === id)?.email ?? id;

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    setErr(null);
    setBusy(true);
    try {
      await createMailbox(form);
      setForm(emptyMailbox());
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Failed to create mailbox");
    } finally {
      setBusy(false);
    }
  };

  const set = <K extends keyof NewMailbox>(key: K, value: NewMailbox[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  return (
    <section className="space-y-3">
      <h2 className="text-sm font-medium text-text-primary uppercase tracking-wide">
        Mailboxes
      </h2>
      <div className="rounded-lg border border-border-secondary divide-y divide-border-secondary">
        {mailboxes.length === 0 && (
          <div className="px-3 py-2 text-sm text-text-tertiary">
            No mailboxes provisioned yet.
          </div>
        )}
        {mailboxes.map((m) => (
          <div
            key={m.id}
            className="flex items-center justify-between px-3 py-2 text-sm"
          >
            <div>
              <span className="text-text-primary">{m.email}</span>
              <span className="ml-2 text-xs text-text-tertiary">
                → {userEmail(m.ownerUserId)}
              </span>
              <span className="ml-2 text-[10px] text-text-tertiary">
                {m.imapHost}:{m.imapPort}
              </span>
            </div>
            <button
              className="text-xs text-danger hover:underline"
              onClick={async () => {
                await deleteMailbox(m.id);
                onChange();
              }}
            >
              Remove
            </button>
          </div>
        ))}
      </div>

      <form onSubmit={add} className="space-y-2 rounded-lg border border-border-secondary p-3">
        <div className="flex flex-wrap gap-2">
          <select
            required
            value={form.ownerUserId}
            onChange={(e) => set("ownerUserId", e.target.value)}
            className="px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary"
          >
            <option value="">Assign to user…</option>
            {users.map((u) => (
              <option key={u.id} value={u.id}>
                {u.email}
              </option>
            ))}
          </select>
          <input
            required
            placeholder="mailbox email"
            value={form.email}
            onChange={(e) => set("email", e.target.value)}
            className="flex-1 min-w-[160px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
        </div>

        <div className="flex flex-wrap gap-2">
          <input
            required
            placeholder="IMAP host"
            value={form.imapHost}
            onChange={(e) => set("imapHost", e.target.value)}
            className="flex-1 min-w-[140px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
          <input
            required
            type="number"
            placeholder="port"
            value={form.imapPort}
            onChange={(e) => set("imapPort", Number(e.target.value))}
            className="w-20 px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
          <select
            value={form.imapSecurity}
            onChange={(e) => set("imapSecurity", e.target.value)}
            className="px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary"
          >
            {SECURITY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                IMAP {o.label}
              </option>
            ))}
          </select>
        </div>

        <div className="flex flex-wrap gap-2">
          <input
            required
            placeholder="SMTP host"
            value={form.smtpHost}
            onChange={(e) => set("smtpHost", e.target.value)}
            className="flex-1 min-w-[140px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
          <input
            required
            type="number"
            placeholder="port"
            value={form.smtpPort}
            onChange={(e) => set("smtpPort", Number(e.target.value))}
            className="w-20 px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
          <select
            value={form.smtpSecurity}
            onChange={(e) => set("smtpSecurity", e.target.value)}
            className="px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary"
          >
            {SECURITY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                SMTP {o.label}
              </option>
            ))}
          </select>
        </div>

        <div className="flex flex-wrap gap-2">
          <input
            placeholder="username (optional, defaults to email)"
            value={form.username ?? ""}
            onChange={(e) => set("username", e.target.value)}
            className="flex-1 min-w-[160px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
          <input
            required
            type="password"
            placeholder="password"
            value={form.password}
            onChange={(e) => set("password", e.target.value)}
            className="flex-1 min-w-[160px] px-2 py-1.5 rounded bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none"
          />
        </div>

        <button
          type="submit"
          disabled={busy || !form.ownerUserId}
          className="px-3 py-1.5 rounded bg-accent text-white text-sm hover:bg-accent-hover disabled:opacity-50"
        >
          Add mailbox
        </button>
        {err && <div className="text-xs text-danger">{err}</div>}
      </form>
    </section>
  );
}

function emptyMailbox(): NewMailbox {
  return {
    ownerUserId: "",
    email: "",
    imapHost: "",
    imapPort: 993,
    imapSecurity: "tls",
    smtpHost: "",
    smtpPort: 465,
    smtpSecurity: "tls",
    username: "",
    password: "",
  };
}
