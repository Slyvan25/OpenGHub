/**
 * Shared bits for the Community screens: the cached index, poster lookup and
 * a few presentational helpers, so the landing page and the detail page agree.
 */
import * as api from "$lib/api";
import type { CommunityEntry, DeviceKind } from "$lib/types";

let indexCache: CommunityEntry[] | null = null;
let posterCache: Record<string, string> | null = null;

/** The repository index; cached for the session unless `refresh`. */
export async function loadIndex(refresh = false): Promise<CommunityEntry[]> {
  if (!refresh && indexCache) return indexCache;
  indexCache = (await api.getCommunityIndex(refresh)).profiles;
  return indexCache;
}

/** application id → poster URL from Logitech's public database; decorative only. */
export async function loadPosters(): Promise<Record<string, string>> {
  if (posterCache) return posterCache;
  try {
    const apps = await api.getApplications();
    posterCache = Object.fromEntries(apps.filter((a) => a.posterUrl).map((a) => [a.id, a.posterUrl!]));
  } catch {
    posterCache = {};
  }
  return posterCache;
}

export function kindOf(e: { device: { kind: string } }): DeviceKind {
  const k = e.device.kind.toLowerCase();
  return (["mouse", "keyboard", "headset", "speaker", "microphone", "light", "webcam", "wheel"].includes(k)
    ? k
    : "other") as DeviceKind;
}

/** First letter for the author avatar, as G HUB draws one when there is no picture. */
export function initial(author: string): string {
  return (author.trim()[0] ?? "?").toUpperCase();
}

/** A stable hue per author, so avatars differ without storing images. */
export function avatarColor(author: string): string {
  let h = 0;
  for (const c of author) h = (h * 31 + c.charCodeAt(0)) % 360;
  return `hsl(${h} 55% 45%)`;
}
