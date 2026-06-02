/**
 * Picks the active transport once, at module load.
 *
 * Selection order:
 *   1. `VITE_TARGET` build flag ("web" | "tauri"), if set.
 *   2. Runtime detection: presence of the Tauri global on `window`.
 *
 * Use `getTransport()` everywhere instead of importing a concrete transport,
 * and `isWeb()` to gate desktop-only features (tray, windows, file dialogs…).
 */

import type { Transport } from "./types";
import { tauriTransport } from "./tauriTransport";
import { httpTransport } from "./httpTransport";

export type { Transport, ExecuteResult } from "./types";

function detectIsTauri(): boolean {
  const target = import.meta.env.VITE_TARGET as string | undefined;
  if (target === "web") return false;
  if (target === "tauri") return true;
  // Runtime fallback: the Tauri webview injects these globals.
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

const runningInTauri = detectIsTauri();

export function isTauri(): boolean {
  return runningInTauri;
}

export function isWeb(): boolean {
  return !runningInTauri;
}

const transport: Transport = runningInTauri ? tauriTransport : httpTransport;

export function getTransport(): Transport {
  return transport;
}

/** Convenience: invoke a backend command through the active transport. */
export function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return transport.invoke<T>(command, args);
}
