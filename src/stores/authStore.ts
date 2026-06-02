import { create } from "zustand";
import type { AuthUser } from "../services/auth/authService";

interface AuthState {
  /** null = unknown/loading, set once /me resolves. */
  user: AuthUser | null;
  checked: boolean; // whether the initial /me check has completed
  setUser: (user: AuthUser | null) => void;
  setChecked: (checked: boolean) => void;
  isAdmin: () => boolean;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  user: null,
  checked: false,
  setUser: (user) => set({ user }),
  setChecked: (checked) => set({ checked }),
  isAdmin: () => get().user?.role === "admin",
}));
