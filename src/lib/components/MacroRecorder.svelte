<script lang="ts">
  /**
   * Records a key sequence by capturing real key events, the way G HUB's macro
   * editor does. Timing between events is captured too, so a macro plays back
   * at the speed it was typed.
   */
  import Icon from "$lib/components/Icon.svelte";
  import {
    describeStep,
    encodedSize,
    macroDuration,
    modifierFor,
    usageFor,
    MAX_DELAY_MS,
    type MacroDef,
    type MacroStep,
  } from "$lib/macros";

  interface Props {
    macro: MacroDef;
    /** Bytes available in the device's macro sector, to warn before overflow. */
    budget?: number;
    onchange: (macro: MacroDef) => void;
    onclose: () => void;
  }

  let { macro, budget = 253, onchange, onclose }: Props = $props();

  let recording = $state(false);
  let recordDelays = $state(true);
  let lastEventAt = 0;

  const size = $derived(encodedSize(macro.steps));
  const tooBig = $derived(size > budget);

  function push(...steps: MacroStep[]) {
    const next = [...macro.steps];
    if (recordDelays && next.length > 0) {
      const gap = Math.min(MAX_DELAY_MS, Math.round(performance.now() - lastEventAt));
      if (gap > 0) next.push({ step: "delay", ms: gap });
    }
    lastEventAt = performance.now();
    next.push(...steps);
    onchange({ ...macro, steps: next });
  }

  function onKeyDown(event: KeyboardEvent) {
    if (!recording) return;
    // Let the user stop without it landing in the macro.
    if (event.key === "Escape") {
      recording = false;
      event.preventDefault();
      return;
    }
    event.preventDefault();
    if (event.repeat) return;

    const modifier = modifierFor(event.code);
    if (modifier !== null) return push({ step: "modifiersDown", mask: modifier });
    const usage = usageFor(event.code);
    if (usage !== null) push({ step: "keyDown", usage });
  }

  function onKeyUp(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    const modifier = modifierFor(event.code);
    if (modifier !== null) return push({ step: "modifiersUp", mask: modifier });
    const usage = usageFor(event.code);
    if (usage !== null) push({ step: "keyUp", usage });
  }

  function toggleRecording() {
    recording = !recording;
    lastEventAt = performance.now();
  }

  function removeStep(index: number) {
    onchange({ ...macro, steps: macro.steps.filter((_, i) => i !== index) });
  }

  function setDelay(index: number, ms: number) {
    const steps = macro.steps.map((s, i) =>
      i === index && s.step === "delay"
        ? { ...s, ms: Math.max(0, Math.min(MAX_DELAY_MS, ms)) }
        : s,
    );
    onchange({ ...macro, steps });
  }
</script>

<svelte:window onkeydown={onKeyDown} onkeyup={onKeyUp} />

<div class="recorder">
  <div class="head">
    <input
      class="name"
      type="text"
      value={macro.name}
      placeholder="Macro name"
      onchange={(e) => onchange({ ...macro, name: e.currentTarget.value })}
    />
    <button class="icon" onclick={onclose} aria-label="Close editor">
      <Icon name="close" size={14} />
    </button>
  </div>

  <div class="controls">
    <button class="record" class:on={recording} onclick={toggleRecording}>
      <span class="dot"></span>
      {recording ? "Stop recording" : "Record keys"}
    </button>
    <label class="toggle">
      <input type="checkbox" bind:checked={recordDelays} />
      Record delays
    </label>
  </div>

  {#if recording}
    <p class="hint live">Press keys now — they are captured, not sent. Escape stops.</p>
  {/if}

  <ol class="steps">
    {#each macro.steps as step, index (index)}
      <li>
        <span class="num">{index + 1}</span>
        {#if step.step === "delay"}
          <input
            class="delay"
            type="number"
            min="0"
            max={MAX_DELAY_MS}
            value={step.ms}
            onchange={(e) => setDelay(index, Number(e.currentTarget.value))}
          />
          <span class="unit">ms</span>
        {:else}
          <span class="desc">{describeStep(step)}</span>
        {/if}
        <button class="icon" onclick={() => removeStep(index)} aria-label="Remove step">
          <Icon name="close" size={11} />
        </button>
      </li>
    {:else}
      <p class="hint">No steps yet. Hit record and type.</p>
    {/each}
  </ol>

  <p class="hint" class:warn={tooBig}>
    {macro.steps.length} steps · {macroDuration(macro.steps)} ms · {size}/{budget} bytes
    {#if tooBig}— too large for the device's macro sector{/if}
  </p>
</div>

<style>
  .recorder {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .name {
    flex: 1;
    padding: 5px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: 13px;
    user-select: text;
  }

  .name:focus {
    outline: none;
    border-color: var(--cyan);
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .record {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text);
  }

  .record .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-dimmer);
  }

  .record.on {
    border-color: var(--danger);
  }

  .record.on .dot {
    background: var(--danger);
    animation: pulse 1s ease-in-out infinite;
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .steps {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 190px;
    overflow-y: auto;
  }

  .steps li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border-radius: 3px;
    background: var(--bg);
    font-size: 12px;
  }

  .num {
    width: 18px;
    font-size: 10px;
    color: var(--text-dimmer);
  }

  .desc {
    flex: 1;
    font-family: var(--font);
    font-weight: 600;
  }

  .delay {
    width: 58px;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: var(--surface-2);
    font-size: 12px;
    user-select: text;
  }

  .unit {
    flex: 1;
    font-size: 11px;
    color: var(--text-dimmer);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    color: var(--text-dimmer);
  }

  .icon:hover {
    background: var(--danger);
    color: #fff;
  }

  .hint {
    font-size: 11.5px;
    color: var(--text-dimmer);
  }

  .hint.live {
    color: var(--danger);
  }

  .hint.warn {
    color: var(--warning);
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
