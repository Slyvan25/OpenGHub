<script lang="ts">
  /**
   * G HUB's "DPI SPEEDS" slider: a horizontal track with the sensor's range on
   * a bent scale (linear to 4000, compressed above), one marker per stage with
   * its value above it, the current stage underlined and the DPI-shift stage
   * drawn as a yellow diamond.
   *
   * Interactions match G HUB: drag a marker to change it, drag it well off the
   * track to delete it, click an empty spot on the track to add a stage, click
   * a value to type one in.
   */
  interface Props {
    stages: number[];
    activeStage: number;
    shiftStage: number | null;
    min: number;
    max: number;
    step: number;
    /** Fixed list of values for sensors without a step; empty = any on the grid. */
    allowed?: number[];
    maxStages?: number;
    disabled?: boolean;
    /** Stage list changed (value edited, added or removed). */
    onchange: (stages: number[], activeStage: number, shiftStage: number | null) => void;
    /** A marker was released or a value typed — write it to the device. */
    oncommit: (value: number, index: number) => void;
  }

  let {
    stages,
    activeStage,
    shiftStage,
    min,
    max,
    step,
    allowed = [],
    maxStages = 5,
    disabled = false,
    onchange,
    oncommit,
  }: Props = $props();

  /** Above this the scale is compressed, as on G HUB's slider. */
  const KNEE = 4000;
  const KNEE_POS = 0.66;

  let track: HTMLDivElement | undefined = $state();
  let dragging = $state<number | null>(null);
  let dragValue = $state<number | null>(null);
  let dragOff = $state(false);
  let editing = $state<number | null>(null);
  let editText = $state("");

  const bent = $derived(max > KNEE * 1.5 && min < KNEE);

  /** Value → 0..1 along the track. */
  function pos(value: number): number {
    const v = Math.min(max, Math.max(min, value));
    if (!bent) return (v - min) / (max - min);
    if (v <= KNEE) return ((v - min) / (KNEE - min)) * KNEE_POS;
    return KNEE_POS + ((v - KNEE) / (max - KNEE)) * (1 - KNEE_POS);
  }

  function valueAt(fraction: number): number {
    const f = Math.min(1, Math.max(0, fraction));
    let raw: number;
    if (!bent) raw = min + f * (max - min);
    else if (f <= KNEE_POS) raw = min + (f / KNEE_POS) * (KNEE - min);
    else raw = KNEE + ((f - KNEE_POS) / (1 - KNEE_POS)) * (max - KNEE);
    return snap(raw);
  }

  function snap(raw: number): number {
    if (allowed.length) {
      return allowed.reduce((best, v) => (Math.abs(v - raw) < Math.abs(best - raw) ? v : best), allowed[0]);
    }
    const v = Math.round(raw / step) * step;
    return Math.min(max, Math.max(min, v));
  }

  /** Scale labels under the track, like G HUB's 100 … 4000 14800 25600. */
  const scale = $derived.by(() => {
    const labels: number[] = [min];
    if (bent) {
      for (let v = 1000; v <= KNEE; v += 1000) labels.push(v);
      const mid = snap((KNEE + max) / 2);
      labels.push(mid, max);
    } else {
      const span = max - min;
      const stepLabel = span <= 2000 ? 500 : span <= 8000 ? 1000 : 2000;
      for (let v = Math.ceil(min / stepLabel) * stepLabel; v < max; v += stepLabel) {
        if (v > min) labels.push(v);
      }
      labels.push(max);
    }
    return [...new Set(labels)];
  });

  const ticks = $derived.by(() => {
    const out: number[] = [];
    const n = 60;
    for (let i = 0; i <= n; i++) out.push(i / n);
    return out;
  });

  function fractionFromEvent(event: PointerEvent | MouseEvent): number {
    if (!track) return 0;
    const rect = track.getBoundingClientRect();
    return (event.clientX - rect.left) / rect.width;
  }

  function startDrag(event: PointerEvent, index: number) {
    if (disabled) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragging = index;
    dragValue = stages[index];
    dragOff = false;
  }

  function moveDrag(event: PointerEvent) {
    if (dragging === null || !track) return;
    const rect = track.getBoundingClientRect();
    dragOff = event.clientY < rect.top - 70 || event.clientY > rect.bottom + 70;
    dragValue = valueAt(fractionFromEvent(event));
  }

  function endDrag() {
    if (dragging === null) return;
    const index = dragging;
    const value = dragValue ?? stages[index];
    dragging = null;
    dragValue = null;

    if (dragOff && stages.length > 1) {
      dragOff = false;
      remove(index);
      return;
    }
    dragOff = false;
    setValue(index, value);
  }

  /** Writes a stage value, re-sorting and keeping the current/shift markers on their stages. */
  function setValue(index: number, value: number) {
    const next = stages.map((s, i) => (i === index ? value : s));
    const order = next.map((v, i) => ({ v, i })).sort((a, b) => a.v - b.v);
    const sorted = order.map((o) => o.v);
    const newActive = order.findIndex((o) => o.i === activeStage);
    const newShift = shiftStage === null ? null : order.findIndex((o) => o.i === shiftStage);
    const changed = sorted.some((v, i) => v !== stages[i]) || newActive !== activeStage;
    onchange(sorted, newActive, newShift);
    if (changed || index === activeStage) oncommit(value, order.findIndex((o) => o.i === index));
  }

  function remove(index: number) {
    const next = stages.filter((_, i) => i !== index);
    let newActive = activeStage;
    if (index < activeStage) newActive -= 1;
    if (newActive >= next.length) newActive = next.length - 1;
    let newShift = shiftStage;
    if (newShift !== null) {
      if (newShift === index) newShift = null;
      else if (index < newShift) newShift -= 1;
    }
    onchange(next, newActive, newShift);
    if (index === activeStage) oncommit(next[newActive], newActive);
  }

  function addAt(event: MouseEvent) {
    if (disabled || dragging !== null || stages.length >= maxStages) return;
    if ((event.target as HTMLElement).closest(".marker, .value")) return;
    const value = valueAt(fractionFromEvent(event));
    if (stages.includes(value)) return;
    const next = [...stages, value].sort((a, b) => a - b);
    const idx = next.indexOf(value);
    const newActive = activeStage + (idx <= activeStage ? 1 : 0);
    const newShift = shiftStage === null ? null : shiftStage + (idx <= shiftStage ? 1 : 0);
    onchange(next, newActive, newShift);
  }

  function select(index: number) {
    if (disabled || index === activeStage) return;
    onchange(stages, index, shiftStage);
    oncommit(stages[index], index);
  }

  function beginEdit(index: number) {
    editing = index;
    editText = String(stages[index]);
  }

  function finishEdit() {
    if (editing === null) return;
    const index = editing;
    editing = null;
    const n = Number(editText.replace(/[^\d]/g, ""));
    if (!Number.isFinite(n) || n <= 0) return;
    const value = snap(n);
    if (value !== stages[index]) setValue(index, value);
  }

  function shown(index: number): number {
    return dragging === index && dragValue !== null ? dragValue : stages[index];
  }
</script>

<div class="dpi-slider" class:disabled>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="track-wrap" onpointermove={moveDrag} onpointerup={endDrag} onpointercancel={endDrag}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="track" bind:this={track} onclick={addAt} title={stages.length >= maxStages ? "" : "Click to add a DPI speed"}>
      <div class="rail"></div>
      {#each ticks as t (t)}
        <span class="tick" style="left: {t * 100}%"></span>
      {/each}

      {#each stages as _, i (i)}
        {@const value = shown(i)}
        {@const p = pos(value)}
        <div
          class="marker"
          class:current={i === activeStage}
          class:shift={i === shiftStage}
          class:dragging={dragging === i}
          class:off={dragging === i && dragOff}
          style="left: {p * 100}%"
        >
          {#if editing === i}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="value edit"
              type="text"
              inputmode="numeric"
              bind:value={editText}
              autofocus
              onblur={finishEdit}
              onkeydown={(e) => {
                if (e.key === "Enter") finishEdit();
                if (e.key === "Escape") editing = null;
              }}
            />
          {:else}
            <button
              class="value"
              class:current={i === activeStage}
              class:shift={i === shiftStage}
              onclick={(e) => {
                e.stopPropagation();
                if (e.detail >= 2) beginEdit(i);
                else select(i);
              }}
              ondblclick={(e) => e.stopPropagation()}
              title="Click to select · double-click to type a value"
            >
              {value.toLocaleString()}
            </button>
          {/if}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span class="knob" onpointerdown={(e) => startDrag(e, i)}></span>
        </div>
      {/each}
    </div>

    <div class="scale">
      {#each scale as v (v)}
        <span style="left: {pos(v) * 100}%">{v.toLocaleString()}</span>
      {/each}
    </div>
  </div>
</div>

<style>
  .dpi-slider {
    --shift: #f5b400;
    width: 100%;
    user-select: none;
  }

  .disabled {
    opacity: 0.6;
    pointer-events: none;
  }

  .track-wrap {
    position: relative;
    padding: 46px 30px 34px;
    touch-action: none;
  }

  .track {
    position: relative;
    height: 22px;
    cursor: copy;
  }

  .rail {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 2px;
    transform: translateY(-50%);
    background: #4a4b4f;
  }

  .tick {
    position: absolute;
    top: 50%;
    width: 1px;
    height: 8px;
    background: #4a4b4f;
    transform: translate(-50%, -50%);
  }

  .marker {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    z-index: 1;
  }

  .marker.dragging {
    z-index: 3;
  }

  .knob {
    display: block;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    cursor: grab;
    transition: transform 100ms;
  }

  .marker.shift .knob {
    width: 13px;
    height: 13px;
    border-radius: 2px;
    background: var(--shift);
    transform: rotate(45deg);
  }

  .marker.dragging .knob {
    cursor: grabbing;
    transform: scale(1.3);
  }

  .marker.shift.dragging .knob {
    transform: rotate(45deg) scale(1.3);
  }

  .marker.off .knob {
    opacity: 0.35;
  }

  .value {
    position: absolute;
    bottom: 22px;
    padding: 2px 4px;
    font-family: var(--font);
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
    white-space: nowrap;
    cursor: pointer;
  }

  .value.current {
    text-decoration: underline;
    text-underline-offset: 4px;
    text-decoration-thickness: 2px;
  }

  .value.shift {
    color: var(--shift);
  }

  .marker.off .value {
    opacity: 0.35;
    text-decoration: line-through;
  }

  .value.edit {
    width: 72px;
    text-align: center;
    border: none;
    border-bottom: 2px solid var(--accent);
    background: var(--surface);
    user-select: text;
  }

  .value.edit:focus {
    outline: none;
  }

  .scale {
    position: relative;
    height: 16px;
    margin-top: 16px;
  }

  .scale span {
    position: absolute;
    transform: translateX(-50%);
    font-size: 11px;
    color: var(--text-dim);
    white-space: nowrap;
  }
</style>
