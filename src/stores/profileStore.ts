import { create } from "zustand";

/**
 * Holds the web user's admin-controlled profile (display name + signature),
 * loaded once after login. Empty on desktop. The composer reads these to set
 * the From display name and the signature; members can't change them.
 */
interface ProfileState {
  displayName: string | null;
  signatureHtml: string | null;
  loaded: boolean;
  setProfile: (p: { displayName: string | null; signatureHtml: string | null }) => void;
}

export const useProfileStore = create<ProfileState>((set) => ({
  displayName: null,
  signatureHtml: null,
  loaded: false,
  setProfile: (p) =>
    set({ displayName: p.displayName, signatureHtml: p.signatureHtml, loaded: true }),
}));
