import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { isWeb } from "./transport";
import { getUnreadInboxCount } from "./db/threads";

let lastCount = -1;

export async function updateBadgeCount(): Promise<void> {
  // Taskbar badge + tray tooltip are desktop-only; nothing to do on the web.
  if (isWeb()) return;
  try {
    const count = await getUnreadInboxCount();
    if (count === lastCount) return;
    lastCount = count;

    try {
      await getCurrentWindow().setBadgeCount(count > 0 ? count : undefined);
    } catch {
      // badge count may not be supported on all platforms
    }

    const tooltip = count > 0 ? `Velo - ${count} unread` : "Velo";
    try {
      await invoke("set_tray_tooltip", { tooltip });
    } catch {
      // tray tooltip update is best-effort
    }
  } catch (err) {
    console.error("Failed to update badge count:", err);
  }
}
