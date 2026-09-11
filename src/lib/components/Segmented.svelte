<script lang="ts" generics="T extends string | number">
  interface Option {
    value: T;
    label: string;
  }

  interface Props {
    value: T;
    options: Option[];
    disabled?: boolean;
    onselect?: (value: T) => void;
  }

  let { value = $bindable(), options, disabled = false, onselect }: Props = $props();

  function select(next: T) {
    if (disabled || next === value) return;
    value = next;
    onselect?.(next);
  }
</script>

<div class="segmented" class:disabled role="group">
  {#each options as option (option.value)}
    <button
      class="segment"
      class:active={option.value === value}
      aria-pressed={option.value === value}
      {disabled}
      onclick={() => select(option.value)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    padding: 3px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    gap: 3px;
  }

  .segmented.disabled {
    opacity: 0.45;
    pointer-events: none;
  }

  .segment {
    padding: 7px 16px;
    border-radius: 3px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-dim);
    white-space: nowrap;
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }

  .segment:hover {
    color: var(--text);
  }

  .segment.active {
    background: var(--accent);
    color: #fff;
  }
</style>
