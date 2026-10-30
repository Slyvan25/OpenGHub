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

/**
 * Per-product overrides, keyed by product id.
 *
 * The device reports only a *location* ("Primary", "Logo"), never a position,
 * and what those mean physically differs per model: a G502's Primary zone is
 * the DPI indicator on its left flank, not the scroll wheel — it has no wheel
 * lighting at all. There is no way to derive this, so it is data, and the user
 * can override it by dragging (see `zonePositions` in the device profile).
 */
const BY_PRODUCT: Record<string, Record<string, ZoneSpot>> = {
  // G502 LIGHTSPEED / HERO / X — DPI LEDs on the left flank, G logo on the palm.
  "407f": { Primary: { x: 0.3, y: 0.38, r: 0.1 }, Logo: { x: 0.52, y: 0.66, r: 0.2 } },
  c08d: { Primary: { x: 0.3, y: 0.38, r: 0.1 }, Logo: { x: 0.52, y: 0.66, r: 0.2 } },
  c08b: { Primary: { x: 0.3, y: 0.38, r: 0.1 }, Logo: { x: 0.52, y: 0.66, r: 0.2 } },
  // G502 X PLUS (CDN top-angle render): the RGB strip wraps the heel, the G logo sits on the palm.
  c095: { Primary: { x: 0.52, y: 0.73, r: 0.14 }, Logo: { x: 0.44, y: 0.59, r: 0.06 } },
  "4099": { Primary: { x: 0.52, y: 0.73, r: 0.14 }, Logo: { x: 0.44, y: 0.59, r: 0.06 } },
};

/**
 * Category fallbacks, used when a product is not in the table above.
 *
 * Deliberately vague: a generic "Primary" glow sits centrally rather than
 * claiming a specific component the device may not even have.
 */
const MOUSE: Record<string, ZoneSpot> = {
  Primary: { x: 0.5, y: 0.45, r: 0.16 },
  Logo: { x: 0.5, y: 0.64, r: 0.22 },
  Left: { x: 0.24, y: 0.45, r: 0.14 },
  Right: { x: 0.76, y: 0.45, r: 0.14 },
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

/**
 * Where to draw a zone's glow, best source first:
 *
 * 1. the user's own dragged position for this device,
 * 2. a per-product entry,
 * 3. the device category's fallback.
 *
 * Only step 1 can be right for user-supplied artwork, since a different photo
 * of the same mouse puts everything somewhere else.
 */
export function spotFor(
  kind: DeviceKind,
  locationName: string,
  index: number,
  options: { productIds?: number[]; overrides?: Record<string, ZoneSpot> } = {},
): ZoneSpot {
  const override = options.overrides?.[String(index)];
  if (override) return override;

  for (const id of options.productIds ?? []) {
    const spot = BY_PRODUCT[id.toString(16).padStart(4, "0")]?.[locationName];
    if (spot) return spot;
  }

  const t = table(kind);
  if (t[locationName]) return t[locationName];
  // Unknown location: spread extra zones horizontally so they stay distinct.
  return { x: Math.min(0.85, 0.25 + index * 0.25), y: 0.5, r: 0.2 };
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
  // Wheels have no generic geometry: their controls come from the depot
  // layout only, so without one nothing is drawn rather than a mouse's dots.
  if (kind === "wheel") return {};
  return kind === "keyboard" ? KEYBOARD_CONTROLS : MOUSE_CONTROLS;
}

/**
 * Button positions drawn for a product's CDN render, for devices whose G HUB
 * depot cannot be imported. Fractions of the image, like the generic tables.
 * G502 X PLUS: the top-angle gallery render `fetch-artwork.sh` downloads.
 * Button numbers follow the device's onboard table (see `G502X` in
 * Assignments.svelte): 4 = G4 back, 5 = G6 DPI shift, 6 = G5 forward,
 * 9 = G9, 10 = G8, 11 = G7.
 */
const G502X_CONTROLS: Record<string, ControlSpot> = {
  "button-1": { dot: { x: 0.43, y: 0.25 }, label: { x: 0.04, y: 0.08 }, side: "left" },
  "button-2": { dot: { x: 0.61, y: 0.25 }, label: { x: 0.96, y: 0.12 }, side: "right" },
  "button-3": { dot: { x: 0.527, y: 0.3 }, label: { x: 0.527, y: 0.02 }, side: "top" },
  "button-7": { dot: { x: 0.48, y: 0.29 }, label: { x: 0.04, y: 0.17 }, side: "left" },
  "button-8": { dot: { x: 0.575, y: 0.29 }, label: { x: 0.96, y: 0.3 }, side: "right" },
  "button-9": { dot: { x: 0.527, y: 0.47 }, label: { x: 0.96, y: 0.47 }, side: "right" },
  "button-10": { dot: { x: 0.375, y: 0.26 }, label: { x: 0.04, y: 0.26 }, side: "left" },
  "button-11": { dot: { x: 0.37, y: 0.32 }, label: { x: 0.04, y: 0.35 }, side: "left" },
  "button-6": { dot: { x: 0.365, y: 0.4 }, label: { x: 0.04, y: 0.44 }, side: "left" },
  "button-4": { dot: { x: 0.35, y: 0.455 }, label: { x: 0.04, y: 0.53 }, side: "left" },
  "button-5": { dot: { x: 0.32, y: 0.54 }, label: { x: 0.04, y: 0.62 }, side: "left" },
};

const CONTROLS_BY_PRODUCT: Record<string, Record<string, ControlSpot>> = {
  c095: G502X_CONTROLS,
  "4099": G502X_CONTROLS,
};

/** The product's own button positions when drawn for its render, else the category's. */
export function controlSpotsFor(kind: DeviceKind, productIds: number[]): Record<string, ControlSpot> {
  for (const id of productIds) {
    const own = CONTROLS_BY_PRODUCT[id.toString(16).padStart(4, "0")];
    if (own) return own;
  }
  return controlSpots(kind);
}
