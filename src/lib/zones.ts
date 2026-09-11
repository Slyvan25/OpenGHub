/**
 * Where a lighting zone sits on the device art.
 *
 * The device tells us a zone's *location* (primary, logo, …) but not where that
 * is in the picture, so the position is approximated per device category. Good
 * enough to read at a glance: a G502's logo glow lands on the palm, its primary
 * glow on the scroll wheel — which is what the real mouse does.
 *
 * Coordinates are fractions of the art box, so they hold at any size and work
 * for both the SVG drawing and a user-supplied photo.
 */
import type { DeviceKind } from "$lib/types";

export interface ZoneSpot {
  /** 0–1 across, 0–1 down. */
  x: number;
  y: number;
  /** Blob radius as a fraction of the box width. */
  r: number;
}

const MOUSE: Record<string, ZoneSpot> = {
  Primary: { x: 0.5, y: 0.26, r: 0.16 },
  Logo: { x: 0.5, y: 0.64, r: 0.26 },
  Left: { x: 0.2, y: 0.45, r: 0.16 },
  Right: { x: 0.8, y: 0.45, r: 0.16 },
};

const KEYBOARD: Record<string, ZoneSpot> = {
  Primary: { x: 0.5, y: 0.5, r: 0.55 },
  Logo: { x: 0.08, y: 0.3, r: 0.12 },
};

const HEADSET: Record<string, ZoneSpot> = {
  Primary: { x: 0.18, y: 0.62, r: 0.16 },
  Logo: { x: 0.82, y: 0.62, r: 0.16 },
};

const GENERIC: Record<string, ZoneSpot> = {
  Primary: { x: 0.5, y: 0.5, r: 0.34 },
  Logo: { x: 0.5, y: 0.5, r: 0.34 },
};

function table(kind: DeviceKind): Record<string, ZoneSpot> {
  switch (kind) {
    case "mouse":
      return MOUSE;
    case "keyboard":
      return KEYBOARD;
    case "headset":
      return HEADSET;
    default:
      return GENERIC;
  }
}

/** Falls back to a centred blob for locations we have no position for. */
export function spotFor(kind: DeviceKind, locationName: string, index: number): ZoneSpot {
  const t = table(kind);
  if (t[locationName]) return t[locationName];
  // Unknown location: spread extra zones horizontally so they stay distinct.
  const spread = 0.25 + index * 0.25;
  return { x: Math.min(0.85, spread), y: 0.5, r: 0.2 };
}

/**
 * Where each assignable control sits on the device art, and where its callout
 * label goes — the layout G HUB uses to label buttons around the render.
 *
 * `dot` is the marker on the device; `label` is the text anchor. Both are
 * fractions of the art box. `side` decides which way the label reads.
 */
export interface ControlSpot {
  dot: { x: number; y: number };
  label: { x: number; y: number };
  side: "left" | "right" | "top";
}

export const MOUSE_CONTROLS: Record<string, ControlSpot> = {
  "button-1": { dot: { x: 0.4, y: 0.2 }, label: { x: 0.04, y: 0.12 }, side: "left" },
  "button-2": { dot: { x: 0.61, y: 0.2 }, label: { x: 0.96, y: 0.12 }, side: "right" },
  "button-3": { dot: { x: 0.5, y: 0.3 }, label: { x: 0.5, y: 0.02 }, side: "top" },
  "button-4": { dot: { x: 0.31, y: 0.38 }, label: { x: 0.04, y: 0.33 }, side: "left" },
  "button-5": { dot: { x: 0.31, y: 0.46 }, label: { x: 0.04, y: 0.46 }, side: "left" },
  "button-6": { dot: { x: 0.5, y: 0.45 }, label: { x: 0.96, y: 0.4 }, side: "right" },
  "wheel-left": { dot: { x: 0.43, y: 0.33 }, label: { x: 0.04, y: 0.6 }, side: "left" },
  "wheel-right": { dot: { x: 0.57, y: 0.33 }, label: { x: 0.96, y: 0.6 }, side: "right" },
};

export const KEYBOARD_CONTROLS: Record<string, ControlSpot> = Object.fromEntries(
  Array.from({ length: 12 }, (_, i) => [
    `g-${i + 1}`,
    {
      dot: { x: 0.1 + (i % 6) * 0.14, y: i < 6 ? 0.36 : 0.52 },
      label: { x: i < 6 ? 0.04 : 0.96, y: 0.08 + (i % 6) * 0.13 },
      side: i < 6 ? ("left" as const) : ("right" as const),
    },
  ]),
);

export function controlSpots(kind: DeviceKind): Record<string, ControlSpot> {
  return kind === "keyboard" ? KEYBOARD_CONTROLS : MOUSE_CONTROLS;
}
