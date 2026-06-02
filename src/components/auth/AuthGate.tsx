import { useEffect, type ReactNode } from "react";
import { isWeb } from "../../services/transport";
import { fetchMe } from "../../services/auth/authService";
import { fetchMyProfile } from "../../services/web/profileService";
import { useAuthStore } from "../../stores/authStore";
import { useProfileStore } from "../../stores/profileStore";
import { LoginScreen } from "./LoginScreen";

/**
 * Web-only authentication gate. On the desktop (`isWeb()` false) it renders
 * children immediately — there are no users there. On the web it checks the
 * session once; if not signed in it shows the login screen, otherwise the app.
 *
 * This must wrap the app BEFORE the DB-heavy startup runs, because all /api/db
 * and /api/imap calls now require a session.
 */
export function AuthGate({ children }: { children: ReactNode }) {
  const user = useAuthStore((s) => s.user);
  const checked = useAuthStore((s) => s.checked);
  const setUser = useAuthStore((s) => s.setUser);
  const setChecked = useAuthStore((s) => s.setChecked);

  useEffect(() => {
    if (!isWeb()) return;
    let cancelled = false;
    fetchMe().then(async (u) => {
      if (cancelled) return;
      setUser(u);
      // Load the admin-controlled profile (display name + signature) once signed in.
      if (u) {
        try {
          const p = await fetchMyProfile();
          useProfileStore.getState().setProfile(p);
        } catch {
          /* profile is optional; ignore */
        }
      }
      if (!cancelled) setChecked(true);
    });
    return () => {
      cancelled = true;
    };
  }, [setUser, setChecked]);

  // Desktop: no auth.
  if (!isWeb()) return <>{children}</>;

  // Web: wait for the initial session check.
  if (!checked) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-bg-primary">
        <div className="text-sm text-text-secondary">Loading…</div>
      </div>
    );
  }

  if (!user) return <LoginScreen />;

  return <>{children}</>;
}
