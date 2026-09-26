import type { HandleClientError } from "@sveltejs/kit";
import { isTauri } from "$lib/api";

/**
 * Errors SvelteKit catches itself (a page that throws while loading or
 * rendering) never reach `window.onerror`, so forward them here as well.
 */
export const handleError: HandleClientError = async ({ error, event }) => {
  const e = error as Error | undefined;
  const message = `${event.url.pathname}: ${e?.stack ?? e?.message ?? String(error)}`;
  console.error(message);
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("frontend_log", { level: "error", message }).catch(() => {});
  }
};
