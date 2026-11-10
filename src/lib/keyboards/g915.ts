/**
 * Key map of the G915 LIGHTSPEED (full size, ISO), drawn by OpenGHub itself.
 *
 * Positions are in key units (1 = one letter key). `usage` is the HID keyboard
 * usage (what Game Mode disables); `led` is the id the keyboard's per-key
 * lighting (0x8081) takes. For ordinary keys that is `usage - 3`, modifiers
 * `usage - 0x78`, G-keys `0xb3 + n`, the logo `0xd2`, media keys their own
 * codes — the numbering OpenRGB documents for this keyboard.
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

// Main block starts after the G-key column.
const X = 1.5;

const keys: Key[] = [
  // G logo and the media keys have LEDs but are not keys Game Mode can take.
  { id: "logo", label: "G", x: 0, y: 0, w: 1, h: 1, led: 0xd2 },
  ...row(0, X, [["Esc", 0x29]]),
  ...row(0, X + 2, [["F1", 0x3a], ["F2", 0x3b], ["F3", 0x3c], ["F4", 0x3d]]),
  ...row(0, X + 6.5, [["F5", 0x3e], ["F6", 0x3f], ["F7", 0x40], ["F8", 0x41]]),
  ...row(0, X + 11, [["F9", 0x42], ["F10", 0x43], ["F11", 0x44], ["F12", 0x45]]),
  ...row(0, X + 15.25, [["PrtSc", 0x46], ["ScrLk", 0x47], ["Pause", 0x48]]),
  { id: "prev", label: "⏮", x: X + 18.5, y: 0, w: 1, h: 1, led: 0x9e },
  { id: "play", label: "⏯", x: X + 19.5, y: 0, w: 1, h: 1, led: 0x9b },
  { id: "next", label: "⏭", x: X + 20.5, y: 0, w: 1, h: 1, led: 0x9d },
  { id: "mute", label: "🔇", x: X + 21.5, y: 0, w: 1, h: 1, led: 0x9c },

  ...[1, 2, 3, 4, 5].map((n) => ({ id: `g${n}`, label: `G${n}`, x: 0, y: n + 0.25, w: 1, h: 1, led: 0xb3 + n })),

  ...row(1.25, X, [
    ["§", 0x35], ["1", 0x1e], ["2", 0x1f], ["3", 0x20], ["4", 0x21], ["5", 0x22], ["6", 0x23], ["7", 0x24],
    ["8", 0x25], ["9", 0x26], ["0", 0x27], ["+", 0x2d], ["´", 0x2e], ["⌫", 0x2a, 2],
  ]),
  ...row(1.25, X + 15.25, [["Ins", 0x49], ["Home", 0x4a], ["PgUp", 0x4b]]),
  ...row(1.25, X + 18.5, [["Num", 0x53], ["/", 0x54], ["*", 0x55], ["−", 0x56]]),

  ...row(2.25, X, [
    ["Tab", 0x2b, 1.5], ["Q", 0x14], ["W", 0x1a], ["E", 0x08], ["R", 0x15], ["T", 0x17], ["Y", 0x1c], ["U", 0x18],
    ["I", 0x0c], ["O", 0x12], ["P", 0x13], ["Å", 0x2f], ["¨", 0x30],
  ]),
  { id: "enter", label: "↵", x: X + 13.75, y: 2.25, w: 1.25, h: 2, usage: 0x28, led: led(0x28) },
  ...row(2.25, X + 15.25, [["Del", 0x4c], ["End", 0x4d], ["PgDn", 0x4e]]),
  ...row(2.25, X + 18.5, [["7", 0x5f], ["8", 0x60], ["9", 0x61]]),
  { id: "kp+", label: "+", x: X + 21.5, y: 2.25, w: 1, h: 2, usage: 0x57, led: led(0x57) },

  ...row(3.25, X, [
    ["Caps", 0x39, 1.75], ["A", 0x04], ["S", 0x16], ["D", 0x07], ["F", 0x09], ["G", 0x0a], ["H", 0x0b], ["J", 0x0d],
    ["K", 0x0e], ["L", 0x0f], ["Ö", 0x33], ["Ä", 0x34], ["'", 0x32],
  ]),
  ...row(3.25, X + 18.5, [["4", 0x5c], ["5", 0x5d], ["6", 0x5e]]),

  ...row(4.25, X, [
    ["Shift", 0xe1, 1.25], ["<", 0x64], ["Z", 0x1d], ["X", 0x1b], ["C", 0x06], ["V", 0x19], ["B", 0x05], ["N", 0x11],
    ["M", 0x10], [",", 0x36], [".", 0x37], ["-", 0x38], ["Shift", 0xe5, 2.75],
  ]),
  ...row(4.25, X + 16.25, [["↑", 0x52]]),
  ...row(4.25, X + 18.5, [["1", 0x59], ["2", 0x5a], ["3", 0x5b]]),
  { id: "kpenter", label: "↵", x: X + 21.5, y: 4.25, w: 1, h: 2, usage: 0x58, led: led(0x58) },

  ...row(5.25, X, [
    ["Ctrl", 0xe0, 1.25], ["Win", 0xe3, 1.25], ["Alt", 0xe2, 1.25], ["", 0x2c, 6.25], ["AltGr", 0xe6, 1.25],
    ["Win", 0xe7, 1.25], ["Menu", 0x65, 1.25], ["Ctrl", 0xe4, 1.25],
  ]),
  ...row(5.25, X + 15.25, [["←", 0x50], ["↓", 0x51], ["→", 0x4f]]),
  ...row(5.25, X + 18.5, [["0", 0x62, 2], [",", 0x63]]),
];

export const G915_KEYS: Key[] = keys;
export const G915_SIZE = { w: X + 22.5, h: 6.25 };

/**
 * Product ids of keyboards this map fits. The G915 X wired (`c359`, #5) is
 * full size with G1-G5 and a numpad too; its LED ids are assumed to follow
 * the G915's until someone confirms them on the hardware.
 */
export const G915_IDS = [0xc33e, 0x407c, 0xb354, 0xc359];
