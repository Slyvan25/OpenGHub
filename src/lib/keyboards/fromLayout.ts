/**
 * A keyboard's key map from its G HUB depot layout: every per-key zone is a
 * key at the position G HUB draws it, named by its HID usage. Positions only
 * ever come from the depot: no device has a hardcoded key map.
 *
 * LED ids are the per-key numbering these keyboards use over 0x8081 (as
 * OpenRGB documents it): usage − 3 for ordinary keys, usage − 0x78 for
 * modifiers, 0xb3 + n for G-keys, 0xd2 for the logo, fixed codes for the
 * media keys and the brightness indicator.
 */
import type { ArtworkView } from "$lib/types";
/** One key of a drawn keyboard, in the layout's own units. */
export interface Key {
  /** Stable id for the UI. */
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** HID usage, for keys Game Mode can disable. */
  usage?: number;
  /** Per-key lighting id. */
  led?: number;
}

const CONSUMER_LED: Record<number, number> = { 0xb6: 0x9e, 0xcd: 0x9b, 0xb5: 0x9d, 0xe2: 0x9c };

/** Short labels, for when the key map is drawn without the photo. */
const NAMES: Record<number, string> = {
  0x28: "↵", 0x29: "Esc", 0x2a: "⌫", 0x2b: "Tab", 0x2c: "", 0x39: "Caps", 0x46: "PrtSc", 0x47: "ScrLk",
  0x48: "Pause", 0x49: "Ins", 0x4a: "Home", 0x4b: "PgUp", 0x4c: "Del", 0x4d: "End", 0x4e: "PgDn",
  0x4f: "→", 0x50: "←", 0x51: "↓", 0x52: "↑", 0x53: "Num", 0x58: "↵", 0x65: "Menu",
  0xe0: "Ctrl", 0xe1: "Shift", 0xe2: "Alt", 0xe3: "Win", 0xe4: "Ctrl", 0xe5: "Shift", 0xe6: "AltGr", 0xe7: "Win",
};

function usageLabel(u: number): string {
  if (u >= 0x04 && u <= 0x1d) return String.fromCharCode(65 + u - 0x04);
  if (u >= 0x1e && u <= 0x26) return String(u - 0x1d);
  if (u === 0x27) return "0";
  if (u >= 0x3a && u <= 0x45) return `F${u - 0x39}`;
  if (u >= 0x59 && u <= 0x61) return String(u - 0x58);
  if (u === 0x62) return "0";
  return NAMES[u] ?? "";
}

export function keysFromLayout(view: ArtworkView): { keys: Key[]; size: { w: number; h: number } } | null {
  const keys: Key[] = [];
  for (const z of view.zones) {
    const c = z.component;
    if (c == null) continue;
    const base = { x: z.x * view.width, y: z.y * view.height, w: z.width * view.width, h: z.height * view.height };
    const id = `${z.id}:${c}`;
    switch (z.id) {
      case "PERKEY_KEYBOARD":
        keys.push({ id, label: usageLabel(c), usage: c, led: c >= 0xe0 ? c - 0x78 : c - 3, ...base });
        break;
      case "PERKEY_GKEY":
        keys.push({ id, label: `G${c}`, led: 0xb3 + c, ...base });
        break;
      case "PERKEY_BRANDING":
        keys.push({ id, label: "G", led: 0xd2, ...base });
        break;
      case "PERKEY_CONSUMER":
        if (CONSUMER_LED[c] !== undefined) keys.push({ id, label: "", led: CONSUMER_LED[c], ...base });
        break;
      case "PERKEY_INDICATOR":
        keys.push({ id, label: "", led: 0x99, ...base });
        break;
    }
  }
  // A layout without per-key zones (mice, older depots) has no key map.
  return keys.filter((k) => k.usage !== undefined).length > 20 ? { keys, size: { w: view.width, h: view.height } } : null;
}
