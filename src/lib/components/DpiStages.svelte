<script lang="ts">
  /**
   * G HUB's sensitivity track: up to five DPI stages laid out along the sensor's
   * full range. Handles are draggable, clicking empty track adds a stage and the
   * highlighted one is the stage the mouse is currently using.
   */
  import Icon from "./Icon.svelte";

  interface Props {
    stages: number[];
    activeStage: number;
    min: number;
    max: number;
    /** Sensor granularity; 0 means "any value in range". */
    step?: number;
    /** Valid values for sensors that only accept a fixed list. */
    allowed?: number[];
    maxStages?: number;
    onchange?: (stages: number[]) => void;
    onselect?: (index: number) => void;
    /** Fires when a drag finishes, i.e. when it is worth writing to the device. */
    oncommit?: (dpi: number) => void;
  }

  let {
    stages = $bindable(),
    activeStage = $bindable(),
    min,
    max,
    step = 50,
    allowed = [],
    maxStages = 5,
    onchange,
    onselect,
    oncommit,
  }: Props = $props();

  let track: HTMLDivElement | undefined = $state();
  let dragging = $state(-1);

  const ratio = (dpi: number) => (max > min ? (dpi - min) / (max - min) : 0);

  function snap(dpi: number): number {
    const clamped = Math.min(max, Math.max(min, dpi));
    if (allowed.length) {
      return allowed.reduce((a, b) => (Math.abs(b - clamped) < Math.abs(a - clamped) ? b : a));
    }
    if (step > 0) {
      return Math.min(max, min + Math.round((clamped - min) / step) * step);
    }
    return Math.round(clamped);
  }

  function dpiAt(clientX: number): number {
    if (!track) return min;
    const rect = track.getBoundingClientRect();
    const t = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return snap(min + t * (max - min));
  }

  function startDrag(index: number, event: PointerEvent) {
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragging = index;
    select(index);
  }

  function onMove(event: PointerEvent) {
    if (dragging < 0) return;
    const next = [...stages];
    next[dragging] = dpiAt(event.clientX);
    stages = next;
    onchange?.(next);
  }

  function endDrag() {
    if (dragging < 0) return;
    const value = stages[dragging];
    dragging = -1;
    oncommit?.(value);
  }

  function select(index: number) {
    activeStage = index;
    onselect?.(index);
    oncommit?.(stages[index]);
  }

  function addStage(event: MouseEvent) {
    if (stages.length >= maxStages) return;
    const value = dpiAt(event.clientX);
    const next = [...stages, value].sort((a, b) => a - b);
    stages = next;
    activeStage = next.indexOf(value);
    onchange?.(next);
    onselect?.(activeStage);
  }

  function removeStage(index: number, event: MouseEvent) {
    event.stopPropagation();
    if (stages.length <= 1) return;
    const next = stages.filter((_, i) => i !== index);
    stages = next;
    activeStage = Math.min(activeStage, next.length - 1);
    onchange?.(next);
    onselect?.(activeStage);
  }

  function nudge(index: number, delta: number) {
    const next = [...stages];
    next[index] = snap(next[index] + delta * (step || 50));
    stages = next;
    onchange?.(next);
    oncommit?.(next[index]);
  }
</script>

<svelte:window onpointermove={onMove} onpointerup={endDrag} />

<div class="wrap">
  <div class="scale">
    <span>{min.toLocaleString()}</span>
    <span class="hint">
      {stages.length < maxStages ? "Click the bar to add a DPI stage" : "Maximum stages reached"}
    </span>
    <span>{max.toLocaleString()} DPI</span>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="track" bind:this={track} onclick={addStage}>
    <div class="fill" style="width: {ratio(stages[activeStage] ?? min) * 100}%"></div>

    {#each stages as dpi, index (index)}
      <div class="stage" class:active={index === activeStage} style="left: {ratio(dpi) * 100}%">
        <div class="bubble">
          {dpi.toLocaleString()}
          {#if stages.length > 1}
            <button class="remove" aria-label="Remove stage" onclick={(e) => removeStage(index, e)}>
              <Icon name="close" size={9} strokeWidth={2.4} />
            </button>
          {/if}
        </div>
        <button
          class="handle"
          aria-label="DPI stage {index + 1}: {dpi}"
          onpointerdown={(e) => startDrag(index, e)}
          onclick={(e) => {
            e.stopPropagation();
            select(index);
          }}
          onkeydown={(e) => {
            if (e.key === "ArrowLeft") nudge(index, -1);
            if (e.key === "ArrowRight") nudge(index, 1);
          }}
        ></button>
      </div>
    {/each}
  </div>

  <div class="stages">
    {#each stages as dpi, index (index)}
      <button class="chip" class:active={index === activeStage} onclick={() => select(index)}>
        <span class="chip-index">{index + 1}</span>
        <span class="chip-dpi">{dpi.toLocaleString()}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .scale {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--text-dimmer);
    letter-spacing: 0.04em;
  }

  .hint {
    color: var(--text-dimmer);
    opacity: 0.7;
  }

  .track {
    position: relative;
    height: 8px;
    margin: 34px 0 6px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    cursor: copy;
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: var(--radius-pill);
    background: linear-gradient(90deg, var(--cyan), var(--accent));
  }

  .stage {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
  }

  .handle {
    display: block;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #cfcfd4;
    box-shadow: 0 1px 6px rgba(0, 0, 0, 0.7);
    cursor: grab;
    transition:
      transform 110ms var(--ease),
      background 110ms var(--ease);
  }

  .handle:active {
    cursor: grabbing;
  }

  .stage.active .handle {
    background: #fff;
    transform: scale(1.2);
  }

  .bubble {
    position: absolute;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 7px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    color: var(--text-dim);
    opacity: 0;
    transition: opacity 120ms var(--ease);
    pointer-events: none;
  }

  /* Stages cluster together on wide-range sensors, so only the one being
     touched shows its label; the chip row below always lists them all. */
  .stage.active .bubble,
  .stage:hover .bubble {
    opacity: 1;
    pointer-events: auto;
  }

  .stage.active .bubble {
    border-color: var(--accent);
    color: var(--text);
  }

  .remove {
    display: grid;
    place-items: center;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    color: var(--text-dimmer);
  }

  .remove:hover {
    background: var(--danger);
    color: #fff;
  }

  .stages {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px 7px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    transition: border-color 120ms var(--ease);
  }

  .chip:hover {
    border-color: var(--line-strong);
  }

  .chip.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .chip-index {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    background: var(--surface-3);
    font-size: 10.5px;
    color: var(--text-dim);
  }

  .chip.active .chip-index {
    background: var(--accent);
    color: #fff;
  }

  .chip-dpi {
    font-family: var(--font);
    font-size: 13.5px;
    font-weight: 600;
  }
</style>
