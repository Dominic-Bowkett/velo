/**
 * Web auth client. Talks to velo-server's /api/auth and /api/admin endpoints.
 * Only used in the web build — on the desktop there are no users (isWeb() is
 * false and the auth gate is skipped entirely).
 */

const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined) ?? "";

export interface AuthUser {
  id: string;
  email: string;
  displayName: string | null;
  role: "admin" | "member";
}

async function req<T>(
  path: string,
  method: "GET" | "POST" | "DELETE",
  body?: unknown,
): Promise<T> {
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
      /* keep status message */
    }
    throw new Error(message);
  }
  const text = await resp.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

/** Returns the current user, or null if not signed in. */
export async function fetchMe(): Promise<AuthUser | null> {
  try {
    return await req<AuthUser>("/api/auth/me", "GET");
  } catch {
    return null;
  }
}

export function login(email: string, password: string): Promise<AuthUser> {
  return req<AuthUser>("/api/auth/login", "POST", { email, password });
}

export async function logout(): Promise<void> {
  await req("/api/auth/logout", "POST");
}

// ---- admin user management ----

export function listUsers(): Promise<AuthUser[]> {
  return req<AuthUser[]>("/api/admin/users", "GET");
}

export function createUser(input: {
  email: string;
  password: string;
  displayName?: string;
  role?: "admin" | "member";
}): Promise<AuthUser> {
  return req<AuthUser>("/api/admin/users", "POST", input);
}

export function deleteUser(id: string): Promise<void> {
  return req(`/api/admin/users/${id}`, "DELETE").then(() => undefined);
}

export function setUserPassword(id: string, password: string): Promise<void> {
  return req(`/api/admin/users/${id}/password`, "POST", { password }).then(
    () => undefined,
  );
}
