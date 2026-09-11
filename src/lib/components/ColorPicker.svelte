<script lang="ts" module>
  /** Colour maths kept out of the component instance so it stays testable. */
  export function hexToRgb(hex: string): [number, number, number] {
    const clean = hex.replace("#", "");
    const full =
      clean.length === 3
        ? clean
            .split("")
            .map((c) => c + c)
            .join("")
        : clean.padEnd(6, "0").slice(0, 6);
    return [
      parseInt(full.slice(0, 2), 16),
      parseInt(full.slice(2, 4), 16),
      parseInt(full.slice(4, 6), 16),
    ];
  }

  export function rgbToHex(r: number, g: number, b: number): string {
    const part = (n: number) =>
      Math.round(Math.min(255, Math.max(0, n)))
        .toString(16)
        .padStart(2, "0");
    return `#${part(r)}${part(g)}${part(b)}`;
  }

  export function hsvToHex(h: number, s: number, v: number): string {
    const c = v * s;
    const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
    const m = v - c;
    const [r, g, b] =
      h < 60
        ? [c, x, 0]
        : h < 120
          ? [x, c, 0]
          : h < 180
            ? [0, c, x]
            : h < 240
              ? [0, x, c]
              : h < 300
                ? [x, 0, c]
                : [c, 0, x];
    return rgbToHex((r + m) * 255, (g + m) * 255, (b + m) * 255);
  }

  export function hexToHsv(hex: string): [number, number, number] {
    const [r, g, b] = hexToRgb(hex).map((v) => v / 255);
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const d = max - min;
    let h = 0;
    if (d !== 0) {
      if (max === r) h = 60 * (((g - b) / d) % 6);
      else if (max === g) h = 60 * ((b - r) / d + 2);
      else h = 60 * ((r - g) / d + 4);
    }
    if (h < 0) h += 360;
    return [h, max === 0 ? 0 : d / max, max];
  }
</script>

<script lang="ts">
  interface Props {
    value: string;
    onchange?: (hex: string) => void;
  }

  let { value = $bindable(), onchange }: Props = $props();

  /** G HUB's preset swatch row. */
  const swatches = [
    "#ff0000",
    "#ff7b00",
    "#ffd400",
    "#57d945",
    "#00b5e2",
    "#1a82e2",
    "#7b3fe4",
    "#ff2d9b",
    "#ffffff",
  ];

  let hsv = $state(hexToHsv(value));
  let field: HTMLDivElement | undefined = $state();

  // Keep the wheel in sync when the colour changes from elsewhere (e.g. a preset).
  $effect(() => {
    const next = hexToHsv(value);
    if (hsvToHex(hsv[0], hsv[1], hsv[2]).toLowerCase() !== value.toLowerCase()) {
      hsv = next;
    }
  });

  function commit() {
    const hex = hsvToHex(hsv[0], hsv[1], hsv[2]);
    value = hex;
    onchange?.(hex);
  }

  function pickFromField(event: PointerEvent) {
    if (!field) return;
    const rect = field.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const y = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height));
    hsv = [hsv[0], x, 1 - y];
    commit();
  }

  function onFieldPointerDown(event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    pickFromField(event);
  }

  function onFieldPointerMove(event: PointerEvent) {
    if (event.buttons === 1) pickFromField(event);
  }

  function setHex(raw: string) {
    const candidate = raw.startsWith("#") ? raw : `#${raw}`;
    if (/^#[0-9a-f]{6}$/i.test(candidate)) {
      value = candidate.toLowerCase();
      hsv = hexToHsv(value);
      onchange?.(value);
    }
  }
</script>

<div class="picker">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="field"
    bind:this={field}
    role="application"
    aria-label="Saturation and brightness"
    style="--hue: {hsv[0]}"
    onpointerdown={onFieldPointerDown}
    onpointermove={onFieldPointerMove}
  >
    <div class="cursor" style="left: {hsv[1] * 100}%; top: {(1 - hsv[2]) * 100}%"></div>
  </div>

  <input
    class="hue"
    type="range"
    min="0"
    max="359"
    step="1"
    value={hsv[0]}
    aria-label="Hue"
    oninput={(e) => {
      hsv = [Number(e.currentTarget.value), hsv[1], hsv[2]];
      commit();
    }}
  />

  <div class="row">
    <div class="preview" style="background: {value}"></div>
    <input
      class="hex"
      type="text"
      value={value.toUpperCase()}
      spellcheck="false"
      maxlength="7"
      aria-label="Hex colour"
      onchange={(e) => setHex(e.currentTarget.value)}
    />
  </div>

  <div class="swatches">
    {#each swatches as swatch}
      <button
        class="swatch"
        class:active={swatch.toLowerCase() === value.toLowerCase()}
        style="background: {swatch}"
        aria-label={swatch}
        onclick={() => setHex(swatch)}
      ></button>
    {/each}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 236px;
  }

  .field {
    position: relative;
    height: 150px;
    border-radius: var(--radius-sm);
    background:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, hsl(var(--hue), 100%, 50%));
    cursor: crosshair;
    touch-action: none;
  }

  .cursor {
    position: absolute;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
    pointer-events: none;
  }

  .hue {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 12px;
    border-radius: var(--radius-pill);
    background: linear-gradient(
      to right,
      #f00 0%,
      #ff0 17%,
      #0f0 33%,
      #0ff 50%,
      #00f 67%,
      #f0f 83%,
      #f00 100%
    );
    cursor: pointer;
  }

  .hue::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    border-radius: 50%;
    background: transparent;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
  }

  .hue::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border: 2px solid #fff;
    border-radius: 50%;
    background: transparent;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .preview {
    width: 34px;
    height: 32px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line-strong);
  }

  .hex {
    flex: 1;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 12.5px;
    letter-spacing: 0.06em;
    user-select: text;
  }

  .hex:focus {
    border-color: var(--accent);
    outline: none;
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 5px;
  }

  .swatch {
    aspect-ratio: 1;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    transition: transform 110ms var(--ease);
  }

  .swatch:hover {
    transform: scale(1.12);
  }

  .swatch.active {
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 3px #fff;
  }
</style>
