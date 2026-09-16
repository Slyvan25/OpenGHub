<script lang="ts">
  /**
   * G HUB's screen-sampler region: a box on a 16:9 stand-in for the monitor
   * that you drag to move and pull by the corner to resize. Values are
   * fractions of the screen so they hold across resolutions.
   */
  import type { Region } from "$lib/types";

  interface Props {
    value: Region;
    onchange: (region: Region) => void;
  }
  let { value, onchange }: Props = $props();

  let box: HTMLDivElement | undefined = $state();
  let drag = $state<{ mode: "move" | "resize"; startX: number; startY: number; start: Region } | null>(null);

  const PRESETS: { label: string; region: Region }[] = [
    { label: "Full", region: { x: 0, y: 0, w: 1, h: 1 } },
    { label: "Left", region: { x: 0, y: 0, w: 0.5, h: 1 } },
    { label: "Right", region: { x: 0.5, y: 0, w: 0.5, h: 1 } },
    { label: "Top", region: { x: 0, y: 0, w: 1, h: 0.4 } },
    { label: "Bottom", region: { x: 0, y: 0.6, w: 1, h: 0.4 } },
    { label: "Centre", region: { x: 0.25, y: 0.25, w: 0.5, h: 0.5 } },
  ];

  function clamp(r: Region): Region {
    const w = Math.min(1, Math.max(0.05, r.w));
    const h = Math.min(1, Math.max(0.05, r.h));
    return { w, h, x: Math.min(1 - w, Math.max(0, r.x)), y: Math.min(1 - h, Math.max(0, r.y)) };
  }

  function begin(e: PointerEvent, mode: "move" | "resize") {
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { mode, startX: e.clientX, startY: e.clientY, start: { ...value } };
  }

  function move(e: PointerEvent) {
    if (!drag || !box) return;
    const rect = box.getBoundingClientRect();
    const dx = (e.clientX - drag.startX) / rect.width;
    const dy = (e.clientY - drag.startY) / rect.height;
    const s = drag.start;
    const next =
      drag.mode === "move"
        ? { ...s, x: s.x + dx, y: s.y + dy }
        : { ...s, w: s.w + dx, h: s.h + dy };
    onchange(clamp(next));
  }

  function end() {
    drag = null;
  }
</script>

<div class="picker">
  <div class="screen" bind:this={box}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="region"
      style="left: {value.x * 100}%; top: {value.y * 100}%; width: {value.w * 100}%; height: {value.h * 100}%"
      onpointerdown={(e) => begin(e, "move")}
      onpointermove={move}
      onpointerup={end}
      onpointercancel={end}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="handle"
        onpointerdown={(e) => begin(e, "resize")}
        onpointermove={move}
        onpointerup={end}
        onpointercancel={end}
      ></span>
    </div>
  </div>
  <div class="presets">
    {#each PRESETS as p (p.label)}
      <button
        class:active={p.region.x === value.x && p.region.y === value.y && p.region.w === value.w && p.region.h === value.h}
        onclick={() => onchange(p.region)}
      >
        {p.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .screen {
    position: relative;
    aspect-ratio: 16 / 9;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: linear-gradient(135deg, #1a1b1f, #0b0c0e);
    overflow: hidden;
    touch-action: none;
  }

  .region {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid var(--cyan);
    background: rgba(0, 169, 224, 0.18);
    cursor: move;
  }

  .handle {
    position: absolute;
    right: -6px;
    bottom: -6px;
    width: 12px;
    height: 12px;
    border-radius: 2px;
    background: var(--cyan);
    cursor: nwse-resize;
  }

  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .presets button {
    padding: 4px 9px;
    border-radius: 4px;
    background: var(--surface-3);
    font-family: var(--font);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .presets button.active {
    background: var(--cyan);
    color: #fff;
  }
</style>
