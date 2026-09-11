<script lang="ts">
  interface Props {
    checked: boolean;
    label?: string;
    description?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(), label, description, disabled = false, onchange }: Props = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<div class="row" class:disabled>
  {#if label}
    <div class="text">
      <div class="label">{label}</div>
      {#if description}<div class="desc">{description}</div>{/if}
    </div>
  {/if}
  <button
    class="track"
    class:on={checked}
    role="switch"
    aria-checked={checked}
    aria-label={label}
    {disabled}
    onclick={toggle}
  >
    <span class="knob"></span>
  </button>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .row.disabled {
    opacity: 0.45;
  }

  .label {
    font-size: 13.5px;
  }

  .desc {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-dim);
    max-width: 52ch;
  }

  .track {
    flex: none;
    position: relative;
    width: 40px;
    height: 22px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    transition: background 150ms var(--ease);
  }

  .track.on {
    background: var(--accent);
  }

  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform 150ms var(--ease);
  }

  .track.on .knob {
    transform: translateX(18px);
  }
</style>
