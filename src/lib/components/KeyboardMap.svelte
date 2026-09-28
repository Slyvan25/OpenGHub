<script lang="ts">
  /**
   * A drawn keyboard: click a key, or drag a box over several, to act on them
   * (G HUB's Freestyle and Game Mode pickers). Purely presentational — the
   * caller decides what a "paint" means and supplies the colours.
   */
  import type { Key } from "$lib/keyboards";

  interface Props {
    keys: Key[];
    size: { w: number; h: number };
    /** Fill per key id; keys without one are drawn dark. */
    fill?: Record<string, string>;
    /** Keys drawn as marked (e.g. disabled in Game Mode). */
    marked?: Set<string>;
    /** Keys that cannot be picked are dimmed and ignore clicks. */
    pickable?: (key: Key) => boolean;
    /** The device render to draw the keys on (G HUB depot layouts). */
    image?: string | null;
    onpick: (keys: Key[]) => void;
  }
  let { keys, size, fill = {}, marked = new Set(), pickable = () => true, image = null, onpick }: Props = $props();

  let box: HTMLDivElement | undefined = $state();
  let drag = $state<{ x0: number; y0: number; x1: number; y1: number } | null>(null);

  /** Pointer position in key units. */
  function unitsAt(e: PointerEvent) {
    const r = box!.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * size.w, y: ((e.clientY - r.top) / r.height) * size.h };
  }

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    const p = unitsAt(e);
    drag = { x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    box!.setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const p = unitsAt(e);
    drag = { ...drag, x1: p.x, y1: p.y };
  }

  function up() {
    if (!drag) return;
    const { x0, y0, x1, y1 } = drag;
    const [l, r] = [Math.min(x0, x1), Math.max(x0, x1)];
    const [t, b] = [Math.min(y0, y1), Math.max(y0, y1)];
    const click = r - l < 0.1 && b - t < 0.1;
    const hit = keys.filter((k) =>
      pickable(k) &&
      (click
        ? l >= k.x && l <= k.x + k.w && t >= k.y && t <= k.y + k.h
        : k.x < r && k.x + k.w > l && k.y < b && k.y + k.h > t),
    );
    drag = null;
    if (hit.length) onpick(hit);
  }
</script>

<div
  class="kb"
  class:photo={!!image}
  style:background-image={image ? `url("${image}")` : undefined}
  bind:this={box}
  style="aspect-ratio: {size.w} / {size.h}"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={() => (drag = null)}
  role="application"
  aria-label="Keyboard"
>
  {#each keys as k (k.id)}
    {@const colour = fill[k.id]}
    <div
      class="key"
      class:marked={marked.has(k.id)}
      class:off={!pickable(k)}
      class:lit={!!colour}
      style="left: {(k.x / size.w) * 100}%; top: {(k.y / size.h) * 100}%; width: calc({(k.w / size.w) * 100}% - 3px);
             height: calc({(k.h / size.h) * 100}% - 3px); {colour ? `--c: ${colour};` : ''}"
      title={k.label}
    >
      {#if !image}<span>{k.label}</span>{/if}
    </div>
  {/each}
  {#if drag}
    {@const l = Math.min(drag.x0, drag.x1)}
    {@const t = Math.min(drag.y0, drag.y1)}
    <div
      class="band"
      style="left: {(l / size.w) * 100}%; top: {(t / size.h) * 100}%;
             width: {(Math.abs(drag.x1 - drag.x0) / size.w) * 100}%; height: {(Math.abs(drag.y1 - drag.y0) / size.h) * 100}%"
    ></div>
  {/if}
</div>

<style>
  .kb {
    position: relative;
    width: 100%;
    user-select: none;
    touch-action: none;
    cursor: crosshair;
  }

  .key {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    background: #2a2b2f;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--text-dimmer);
    font-size: clamp(7px, 0.8vw, 11px);
    font-weight: 600;
    overflow: hidden;
  }

  .key.lit {
    background: color-mix(in srgb, var(--c) 55%, #1a1b1e);
    border-color: var(--c);
    color: #fff;
    box-shadow: 0 0 8px color-mix(in srgb, var(--c) 45%, transparent);
  }

  .key.marked {
    background: #f2f2f2;
    color: #111;
    border-color: #fff;
  }

  .key.off {
    opacity: 0.35;
  }

  /* On the device render the keys are outlines over the photo, and a lit
     key is a wash of its colour, the way G HUB's Freestyle looks. */
  .kb.photo {
    background-size: 100% 100%;
    background-repeat: no-repeat;
  }

  .kb.photo .key {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.12);
  }

  .kb.photo .key.lit {
    background: color-mix(in srgb, var(--c) 60%, transparent);
    border-color: var(--c);
  }

  .kb.photo .key.marked {
    background: rgba(255, 255, 255, 0.75);
  }

  .band {
    position: absolute;
    border: 1px dashed #fff;
    background: rgba(255, 255, 255, 0.08);
    pointer-events: none;
  }
</style>
