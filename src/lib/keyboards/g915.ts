/**
 * Key maps of the G915 family, drawn by OpenGHub itself: the G915 LIGHTSPEED
 * and the G915 X, in ISO or ANSI.
 *
 * Positions are in key units (1 = one letter key). `usage` is the HID keyboard
 * usage (what Game Mode disables); `led` is the id the keyboard's per-key
 * lighting (0x8081) takes. For ordinary keys that is `usage - 3`, modifiers
 * `usage - 0x78`, G-keys `0xb3 + n`, the logo `0xd2`, media keys their own
 * codes — the numbering OpenRGB documents for these keyboards.
 *
 * The G915 X adds a strip above the F row — G6–G9 over F1–F4, the brightness
 * key over F6 and the media keys over the numpad — and has no M keys. Its
 * LED ids were verified on a wired G915 X (`c359`); they match OpenRGB's.
 */

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

export interface KeyMap {
  keys: Key[];
  size: { w: number; h: number };
}

const led = (usage: number) => (usage >= 0xe0 ? usage - 0x78 : usage - 3);

function row(y: number, x0: number, keys: [string, number | null, number?][]): Key[] {
  let x = x0;
  return keys.map(([label, usage, w = 1]) => {
    const k: Key = { id: `${label}@${x},${y}`, label, x, y, w, h: 1 };
    if (usage !== null) {
      k.usage = usage;
      k.led = led(usage);
    }
    x += w;
    return k;
  });
}

interface Variant {
  /** ISO physical layout (tall Enter, extra key by the left Shift) rather than ANSI. */
  iso: boolean;
  /** G915 X: the strip above the F row instead of the G915's M keys. */
  x: boolean;
}

function build({ iso, x }: Variant): KeyMap {
  // Main block starts after the G-key column.
  const X = 1.5;
  // The G915 X's strip takes row 0; everything else moves down one unit.
  const T = x ? 1 : 0;

  const keys: Key[] = [
    // G logo and the media keys have LEDs but are not keys Game Mode can take.
    { id: "logo", label: "G", x: 0, y: 0, w: 1, h: 1, led: 0xd2 },
    ...(x
      ? [
          ...[6, 7, 8, 9].map((n, i) => ({ id: `g${n}`, label: `G${n}`, x: X + 2 + i, y: 0, w: 1, h: 1, led: 0xb3 + n })),
          { id: "brightness", label: "☀", x: X + 7.5, y: 0, w: 1, h: 1, led: 0x99 },
        ]
      : []),
    ...row(T, X, [["Esc", 0x29]]),
    ...row(T, X + 2, [["F1", 0x3a], ["F2", 0x3b], ["F3", 0x3c], ["F4", 0x3d]]),
    ...row(T, X + 6.5, [["F5", 0x3e], ["F6", 0x3f], ["F7", 0x40], ["F8", 0x41]]),
    ...row(T, X + 11, [["F9", 0x42], ["F10", 0x43], ["F11", 0x44], ["F12", 0x45]]),
    ...row(T, X + 15.25, [["PrtSc", 0x46], ["ScrLk", 0x47], ["Pause", 0x48]]),
    { id: "prev", label: "⏮", x: X + 18.5, y: 0, w: 1, h: 1, led: 0x9e },
    { id: "play", label: "⏯", x: X + 19.5, y: 0, w: 1, h: 1, led: 0x9b },
    { id: "next", label: "⏭", x: X + 20.5, y: 0, w: 1, h: 1, led: 0x9d },
    { id: "mute", label: "🔇", x: X + 21.5, y: 0, w: 1, h: 1, led: 0x9c },

    ...[1, 2, 3, 4, 5].map((n) => ({ id: `g${n}`, label: `G${n}`, x: 0, y: n + 0.25 + T, w: 1, h: 1, led: 0xb3 + n })),

    ...row(1.25 + T, X, [
      [iso ? "§" : "`", 0x35], ["1", 0x1e], ["2", 0x1f], ["3", 0x20], ["4", 0x21], ["5", 0x22], ["6", 0x23], ["7", 0x24],
      ["8", 0x25], ["9", 0x26], ["0", 0x27], [iso ? "+" : "-", 0x2d], [iso ? "´" : "=", 0x2e], ["⌫", 0x2a, 2],
    ]),
    ...row(1.25 + T, X + 15.25, [["Ins", 0x49], ["Home", 0x4a], ["PgUp", 0x4b]]),
    ...row(1.25 + T, X + 18.5, [["Num", 0x53], ["/", 0x54], ["*", 0x55], ["−", 0x56]]),

    ...row(2.25 + T, X, [
      ["Tab", 0x2b, 1.5], ["Q", 0x14], ["W", 0x1a], ["E", 0x08], ["R", 0x15], ["T", 0x17], ["Y", 0x1c], ["U", 0x18],
      ["I", 0x0c], ["O", 0x12], ["P", 0x13], [iso ? "Å" : "[", 0x2f], [iso ? "¨" : "]", 0x30],
    ]),
    ...(iso
      ? [{ id: "enter", label: "↵", x: X + 13.75, y: 2.25 + T, w: 1.25, h: 2, usage: 0x28, led: led(0x28) }]
      : [
          ...row(2.25 + T, X + 13.5, [["\\", 0x31, 1.5]]),
          ...row(3.25 + T, X + 12.75, [["↵", 0x28, 2.25]]),
        ]),
    ...row(2.25 + T, X + 15.25, [["Del", 0x4c], ["End", 0x4d], ["PgDn", 0x4e]]),
    ...row(2.25 + T, X + 18.5, [["7", 0x5f], ["8", 0x60], ["9", 0x61]]),
    { id: "kp+", label: "+", x: X + 21.5, y: 2.25 + T, w: 1, h: 2, usage: 0x57, led: led(0x57) },

    ...row(3.25 + T, X, [
      ["Caps", 0x39, 1.75], ["A", 0x04], ["S", 0x16], ["D", 0x07], ["F", 0x09], ["G", 0x0a], ["H", 0x0b], ["J", 0x0d],
      ["K", 0x0e], ["L", 0x0f], [iso ? "Ö" : ";", 0x33], [iso ? "Ä" : "'", 0x34],
      ...(iso ? ([["'", 0x32]] as [string, number][]) : []),
    ]),
    ...row(3.25 + T, X + 18.5, [["4", 0x5c], ["5", 0x5d], ["6", 0x5e]]),

    ...row(4.25 + T, X, [
      ...(iso ? ([["Shift", 0xe1, 1.25], ["<", 0x64]] as [string, number, number?][]) : ([["Shift", 0xe1, 2.25]] as [string, number, number?][])),
      ["Z", 0x1d], ["X", 0x1b], ["C", 0x06], ["V", 0x19], ["B", 0x05], ["N", 0x11],
      ["M", 0x10], [",", 0x36], [".", 0x37], [iso ? "-" : "/", 0x38], ["Shift", 0xe5, 2.75],
    ]),
    ...row(4.25 + T, X + 16.25, [["↑", 0x52]]),
    ...row(4.25 + T, X + 18.5, [["1", 0x59], ["2", 0x5a], ["3", 0x5b]]),
    { id: "kpenter", label: "↵", x: X + 21.5, y: 4.25 + T, w: 1, h: 2, usage: 0x58, led: led(0x58) },

    ...row(5.25 + T, X, [
      ["Ctrl", 0xe0, 1.25], ["Win", 0xe3, 1.25], ["Alt", 0xe2, 1.25], ["", 0x2c, 6.25], [iso ? "AltGr" : "Alt", 0xe6, 1.25],
      ["Win", 0xe7, 1.25], ["Menu", 0x65, 1.25], ["Ctrl", 0xe4, 1.25],
    ]),
    ...row(5.25 + T, X + 15.25, [["←", 0x50], ["↓", 0x51], ["→", 0x4f]]),
    ...row(5.25 + T, X + 18.5, [["0", 0x62, 2], [iso ? "," : ".", 0x63]]),
  ];

  return { keys, size: { w: X + 22.5, h: 6.25 + T } };
}

const cache = new Map<string, KeyMap>();
function variant(v: Variant): KeyMap {
  const key = `${v.iso ? "iso" : "ansi"}-${v.x ? "x" : "g915"}`;
  let m = cache.get(key);
  if (!m) {
    m = build(v);
    cache.set(key, m);
  }
  return m;
}

/** The G915 LIGHTSPEED map as first drawn (ISO). */
export const G915_KEYS: Key[] = variant({ iso: true, x: false }).keys;
export const G915_SIZE = variant({ iso: true, x: false }).size;

/** Product ids of keyboards the G915 map fits. */
export const G915_IDS = [0xc33e, 0x407c, 0xb354];
/** Product ids of the G915 X. */
export const G915X_IDS = [0xc359];

/**
 * Layout codes from HID++ `0x4540` that are physically ISO. The rest — US (1)
 * first among them — are drawn ANSI. The list follows OpenRGB's.
 */
const ISO_LAYOUTS = new Set([
  0x02, 0x03, 0x04, 0x05, 0x08, 0x0d, 0x0e, 0x0f, 0x11, 0x14, 0x16, 0x19, 0x1a, 0x1d, 0x1f, 0x21, 0x24, 0x37, 0x41,
]);

/**
 * The drawn map for a device, from its product ids and the layout code it
 * reports, or `null` for keyboards OpenGHub has no map for. Without a layout
 * code the G915 stays ISO (how it was drawn before the code was read) and the
 * G915 X is ANSI (the unit it was verified on).
 */
export function keyMapFor(ids: number[], layout?: number | null): KeyMap | null {
  const x = ids.some((id) => G915X_IDS.includes(id));
  if (!x && !ids.some((id) => G915_IDS.includes(id))) {
    return null;
  }
  const iso = layout ? ISO_LAYOUTS.has(layout) : !x;
  return variant({ iso, x });
}
