<script lang="ts">
  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    suffix?: string;
    disabled?: boolean;
    /** Fires continuously while dragging. */
    oninput?: (value: number) => void;
    /** Fires once the handle is released — use this for device writes. */
    onchange?: (value: number) => void;
  }

  let {
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    label,
    suffix = "",
    disabled = false,
    oninput,
    onchange,
  }: Props = $props();

  const percent = $derived(max > min ? ((value - min) / (max - min)) * 100 : 0);

  function handleInput(event: Event) {
    const next = Number((event.currentTarget as HTMLInputElement).value);
    value = next;
    oninput?.(next);
  }

  function handleChange(event: Event) {
    onchange?.(Number((event.currentTarget as HTMLInputElement).value));
  }
</script>

<div class="slider" class:disabled>
  {#if label}
    <div class="head">
      <span class="label">{label}</span>
      <span class="value">{value}{suffix}</span>
    </div>
  {/if}
  <input
    type="range"
    {min}
    {max}
    {step}
    {disabled}
    {value}
    style="--percent: {percent}%"
    oninput={handleInput}
    onchange={handleChange}
    aria-label={label}
  />
</div>

<style>
  .slider {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .slider.disabled {
    opacity: 0.45;
    pointer-events: none;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    font-size: 12px;
  }

  .label {
    color: var(--text-dim);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .value {
    font-family: var(--font);
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }

  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 18px;
    background: transparent;
    cursor: pointer;
  }

  input[type="range"]::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--percent),
      var(--surface-3) var(--percent),
      var(--surface-3) 100%
    );
  }

  input[type="range"]::-moz-range-track {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
  }

  input[type="range"]::-moz-range-progress {
    height: 4px;
    border-radius: 2px;
    background: var(--accent);
  }

  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -5px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.6);
    transition: transform 110ms var(--ease);
  }

  input[type="range"]::-moz-range-thumb {
    width: 14px;
    height: 14px;
    border: none;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.6);
  }

  input[type="range"]:hover::-webkit-slider-thumb,
  input[type="range"]:active::-webkit-slider-thumb {
    transform: scale(1.15);
  }
</style>
