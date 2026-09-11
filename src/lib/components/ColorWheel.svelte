<script lang="ts">
  /**
   * G HUB's colour control: a hue/saturation wheel with a value slider beneath,
   * numeric RGB readouts and a row of saved swatches.
   */
  import { hexToHsv, hsvToHex, hexToRgb, rgbToHex } from "./ColorPicker.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    value: string;
    /** Swatches persist per profile; the parent owns them. */
    swatches?: string[];
    onchange?: (hex: string) => void;
    onswatches?: (swatches: string[]) => void;
  }

  let {
    value = $bindable(),
    swatches = ["#00b5e2", "#ff2d2d", "#ffffff", "#9002ff"],
    onchange,
    onswatches,
  }: Props = $props();

  let wheel: HTMLDivElement | undefined = $state();
  let hsv = $state(hexToHsv(value));

  /**
   * Hue 0 sits at 12 o'clock and increases clockwise. `pick()`, `marker` and the
   * painted wheel must all agree on that, so the gradient stops are generated
   * from the same convention rather than written out by hand — getting these out
   * of step rotates the wheel and every colour comes out wrong.
   */
  const WHEEL_GRADIENT = [0, 60, 120, 180, 240, 300, 360]
    .map((hue) => `${hsvToHex(hue % 360, 1, 1)} ${((hue / 360) * 100).toFixed(2)}%`)
    .join(", ");

  // Follow external changes (preset picked, profile loaded) without fighting
  // the user's own dragging.
  $effect(() => {
    if (hsvToHex(hsv[0], hsv[1], hsv[2]).toLowerCase() !== value.toLowerCase()) {
      hsv = hexToHsv(value);
    }
  });

  const rgb = $derived(hexToRgb(value));

  function commit() {
    const hex = hsvToHex(hsv[0], hsv[1], hsv[2]);
    value = hex;
    onchange?.(hex);
  }

  /** Polar pick: angle is hue, distance from centre is saturation. */
  function pick(event: PointerEvent) {
    if (!wheel) return;
    const rect = wheel.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const dx = event.clientX - cx;
    const dy = event.clientY - cy;

    let angle = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
    if (angle < 0) angle += 360;

    const radius = rect.width / 2;
    const saturation = Math.min(1, Math.hypot(dx, dy) / radius);

    hsv = [angle, saturation, hsv[2] || 1];
    commit();
  }

  function onPointerDown(event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    pick(event);
  }

  function setChannel(index: 0 | 1 | 2, raw: string) {
    const n = Math.max(0, Math.min(255, Number(raw) || 0));
    const next = [...rgb] as [number, number, number];
    next[index] = n;
    const hex = rgbToHex(next[0], next[1], next[2]);
    value = hex;
    hsv = hexToHsv(hex);
    onchange?.(hex);
  }

  function useSwatch(hex: string) {
    value = hex;
    hsv = hexToHsv(hex);
    onchange?.(hex);
  }

  function addSwatch() {
    if (swatches.includes(value)) return;
    onswatches?.([...swatches, value].slice(-8));
  }

  /** Marker position, mirroring the polar mapping above. */
  const marker = $derived.by(() => {
    const angle = ((hsv[0] - 90) * Math.PI) / 180;
    return {
      x: 50 + Math.cos(angle) * hsv[1] * 50,
      y: 50 + Math.sin(angle) * hsv[1] * 50,
    };
  });

  const fullValue = $derived(hsvToHex(hsv[0], hsv[1], 1));
</script>

<div class="wrap">
  <div class="top">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="wheel"
      bind:this={wheel}
      role="application"
      aria-label="Hue and saturation"
      onpointerdown={onPointerDown}
      onpointermove={(e) => e.buttons === 1 && pick(e)}
      style="--wheel: {WHEEL_GRADIENT}"
    >
      <div class="marker" style="left: {marker.x}%; top: {marker.y}%"></div>
    </div>

    <div class="channels">
      {#each [["R", 0], ["G", 1], ["B", 2]] as [label, index] (label)}
        <label>
          <span>{label}</span>
          <input
            type="number"
            min="0"
            max="255"
            value={rgb[index as 0 | 1 | 2]}
            onchange={(e) => setChannel(index as 0 | 1 | 2, e.currentTarget.value)}
          />
        </label>
      {/each}
    </div>
  </div>

  <input
    class="value"
    type="range"
    min="0"
    max="100"
    value={Math.round(hsv[2] * 100)}
    aria-label="Colour value"
    style="--to: {fullValue}"
    oninput={(e) => {
      hsv = [hsv[0], hsv[1], Number(e.currentTarget.value) / 100];
      commit();
    }}
  />

  <div class="swatches">
    {#each swatches as swatch (swatch)}
      <button
        class="swatch"
        class:active={swatch.toLowerCase() === value.toLowerCase()}
        style="background: {swatch}"
        aria-label={swatch}
        onclick={() => useSwatch(swatch)}
      ></button>
    {/each}
    <button class="add" onclick={addSwatch} aria-label="Save this colour">
      <Icon name="plus" size={13} />
    </button>
  </div>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 18px;
  }

  .wheel {
    position: relative;
    width: 108px;
    height: 108px;
    flex: none;
    border-radius: 50%;
    cursor: crosshair;
    touch-action: none;
    /* Hue around the rim, desaturating to white in the middle. `from 0deg`
       puts the first stop at 12 o'clock, matching the hue convention above. */
    background:
      radial-gradient(circle closest-side, #fff, transparent 100%),
      conic-gradient(from 0deg, var(--wheel));
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }

  .marker {
    position: absolute;
    width: 13px;
    height: 13px;
    margin: -6.5px 0 0 -6.5px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.65);
    pointer-events: none;
  }

  .channels {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .channels label {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .channels span {
    width: 12px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .channels input {
    -moz-appearance: textfield;
    appearance: textfield;
    width: 58px;
    padding: 4px 7px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    font-family: var(--font);
    font-size: 14px;
    font-weight: 600;
    user-select: text;
  }

  .channels input::-webkit-outer-spin-button,
  .channels input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .channels input:hover,
  .channels input:focus {
    border-color: var(--line-strong);
    background: var(--surface-2);
    outline: none;
  }

  .value {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 14px;
    border-radius: 3px;
    background: linear-gradient(to right, #000, var(--to));
    cursor: pointer;
  }

  .value::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 18px;
    border: 2px solid #fff;
    border-radius: 3px;
    background: transparent;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
  }

  .value::-moz-range-thumb {
    width: 12px;
    height: 16px;
    border: 2px solid #fff;
    border-radius: 3px;
    background: transparent;
  }

  .swatches {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .swatch {
    width: 19px;
    height: 19px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.18);
    transition: transform 110ms var(--ease);
  }

  .swatch:hover {
    transform: scale(1.15);
  }

  .swatch.active {
    box-shadow: 0 0 0 2px var(--surface), 0 0 0 3px #fff;
  }

  .add {
    display: grid;
    place-items: center;
    width: 19px;
    height: 19px;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
    color: var(--text-dim);
  }

  .add:hover {
    color: var(--text);
    border-color: var(--text-dim);
  }
</style>
