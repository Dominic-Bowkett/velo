import { useState } from "react";
import { login } from "../../services/auth/authService";
import { useAuthStore } from "../../stores/authStore";

export function LoginScreen() {
  const setUser = useAuthStore((s) => s.setUser);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const user = await login(email, password);
      setUser(user);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign in failed");
      setBusy(false);
    }
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-bg-primary">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-sm p-8 rounded-xl glass-modal border border-border-primary"
      >
        <h1 className="text-2xl font-semibold text-text-primary mb-1">Velo</h1>
        <p className="text-sm text-text-secondary mb-6">Sign in to your mailbox</p>

        <label className="block text-xs text-text-secondary mb-1">Email</label>
        <input
          type="email"
          autoFocus
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          className="w-full mb-4 px-3 py-2 rounded-lg bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none focus:border-accent"
        />

        <label className="block text-xs text-text-secondary mb-1">Password</label>
        <input
          type="password"
          required
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          className="w-full mb-5 px-3 py-2 rounded-lg bg-bg-secondary border border-border-secondary text-sm text-text-primary outline-none focus:border-accent"
        />

        {error && (
          <div className="mb-4 text-xs text-danger" role="alert">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={busy}
          className="w-full py-2 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover disabled:opacity-50 transition-colors"
        >
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </form>
    </div>
  );
}
