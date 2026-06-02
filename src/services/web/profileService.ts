/**
 * Web-only admin-controlled profile: the outgoing display name and signature,
 * resolved on the server (per-user override of a global default). Members cannot
 * change these — they only read their resolved profile here.
 */

const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined) ?? "";

export interface ResolvedProfile {
  displayName: string | null;
  signatureHtml: string | null;
}

async function req<T>(path: string, method: "GET" | "PUT", body?: unknown): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    credentials: "include",
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!resp.ok) {
    let message = `Request failed (${resp.status})`;
    try {
      const data = await resp.json();
      if (data?.error) message = data.error;
    } catch {
      /* keep */
    }
    throw new Error(message);
  }
  const text = await resp.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

/** Resolved profile for the current user. */
export function fetchMyProfile(): Promise<ResolvedProfile> {
  return req<ResolvedProfile>("/api/profile", "GET");
}

// ---- admin ----

export interface ProfileValue {
  displayName: string | null;
  signatureHtml: string | null;
}

export interface PerUserProfile extends ProfileValue {
  userId: string;
  email: string;
}

export interface AdminProfileView {
  global: ProfileValue;
  perUser: PerUserProfile[];
}

export function fetchAdminProfiles(): Promise<AdminProfileView> {
  return req<AdminProfileView>("/api/admin/profile", "GET");
}

export function setGlobalProfile(value: ProfileValue): Promise<void> {
  return req("/api/admin/profile/global", "PUT", value).then(() => undefined);
}

export function setUserProfile(userId: string, value: ProfileValue): Promise<void> {
  return req(`/api/admin/profile/user/${userId}`, "PUT", value).then(
    () => undefined,
  );
}
