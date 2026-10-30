/**
 * A key map built from an imported G HUB depot layout, for keyboards OpenGHub
 * has not drawn by hand (the G910 generation). The depot's `PERKEY_*` zones
 * are one rectangle per key, with the key's HID usage as the component id;
 * G-keys and logo LEDs are numbered from 1.
 *
 * LED ids follow the first-generation per-key feature (`0x8080`), which
 * addresses keys by HID usage: the usage itself for keys, `0xe7 + n` for logo
 * LEDs and `0xf0 + n` for G-keys — the same scheme `per_key_v1` in
 * `features.rs` decodes. Keyboards on `0x8081` number their LEDs differently
 * and need a drawn map (see `g915.ts`).
 */
import type { ArtworkLayout, ArtworkView } from "$lib/types";
import { keyMapFor, type Key, type KeyMap } from "./g915";

const LOGO_BASE = 0xe7;
const GKEY_BASE = 0xf0;

/** Labels by HID keyboard usage. Unlisted usages show their number. */
const LABELS: Record<number, string> = {
  40: "Enter", 41: "Esc", 42: "⌫", 43: "Tab", 44: "Space", 45: "-", 46: "=", 47: "[", 48: "]", 49: "\\",
  50: "#", 51: ";", 52: "'", 53: "`", 54: ",", 55: ".", 56: "/", 57: "Caps",
  70: "PrtSc", 71: "ScrLk", 72: "Pause", 73: "Ins", 74: "Home", 75: "PgUp", 76: "Del", 77: "End", 78: "PgDn",
  79: "→", 80: "←", 81: "↓", 82: "↑", 83: "Num", 84: "/", 85: "*", 86: "-", 87: "+", 88: "Enter", 98: "0", 99: ".",
  100: "\\", 101: "Menu",
  224: "Ctrl", 225: "Shift", 226: "Alt", 227: "Win", 228: "Ctrl", 229: "Shift", 230: "Alt", 231: "Win",
};
for (let u = 4; u <= 29; u++) LABELS[u] = String.fromCharCode(65 + u - 4);
for (let u = 30; u <= 38; u++) LABELS[u] = String(u - 29);
LABELS[39] = "0";
for (let u = 58; u <= 69; u++) LABELS[u] = `F${u - 57}`;
for (let u = 89; u <= 97; u++) LABELS[u] = String(u - 88);

/** The map for one depot view, or `null` when it has no per-key zones. */
export function keyMapFromLayout(view: ArtworkView): KeyMap | null {
  const keys: Key[] = [];
  for (const z of view.zones) {
    if (z.component === undefined || z.component === null) continue;
    const px = { x: z.x * view.width, y: z.y * view.height, w: z.width * view.width, h: z.height * view.height };
    if (z.id === "PERKEY_KEYBOARD") {
      const usage = z.component;
      keys.push({ id: `k${usage}`, label: LABELS[usage] ?? String(usage), usage, led: usage, ...px });
    } else if (z.id === "PERKEY_GKEY") {
      keys.push({ id: `g${z.component}`, label: `G${z.component}`, led: GKEY_BASE + z.component, ...px });
    } else if (z.id === "PERKEY_BRANDING") {
      keys.push({ id: `logo${z.component}`, label: "G", led: LOGO_BASE + z.component, ...px });
    }
  }
  if (keys.length === 0) return null;
  // Crop to the keys: the render has a wrist rest and empty margins around them.
  const pad = Math.max(...keys.map((k) => k.w)) * 0.5;
  const minX = Math.min(...keys.map((k) => k.x)) - pad;
  const minY = Math.min(...keys.map((k) => k.y)) - pad;
  const maxX = Math.max(...keys.map((k) => k.x + k.w)) + pad;
  const maxY = Math.max(...keys.map((k) => k.y + k.h)) + pad;
  for (const k of keys) {
    k.x -= minX;
    k.y -= minY;
  }
  return { keys, size: { w: maxX - minX, h: maxY - minY } };
}

/**
 * The key map for a device: the drawn one when OpenGHub has it, otherwise
 * the one from its imported depot layout, otherwise `null`.
 */
export function keyMapForDevice(ids: number[], layoutCode: number | null | undefined, layout: ArtworkLayout | null): KeyMap | null {
  const drawn = keyMapFor(ids, layoutCode);
  if (drawn) return drawn;
  const view = layout?.views.find((v) => v.view === "front") ?? layout?.views[0];
  return view ? keyMapFromLayout(view) : null;
}
