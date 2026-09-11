<script lang="ts">
  import { ui } from "$lib/stores/ui.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="stack" role="status" aria-live="polite">
  {#each ui.toasts as toast (toast.id)}
    <div class="toast {toast.tone}">
      <Icon name={toast.tone === "error" ? "alert" : toast.tone === "success" ? "check" : "info"} size={15} />
      <span>{toast.message}</span>
      <button onclick={() => ui.dismiss(toast.id)} aria-label="Dismiss">
        <Icon name="close" size={13} />
      </button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 20px;
    bottom: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 80;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 420px;
    padding: 11px 12px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--text-dim);
    border-radius: var(--radius-sm);
    background: #161616;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.55);
    font-size: 13px;
    pointer-events: auto;
    animation: rise 180ms var(--ease);
  }

  .toast.error {
    border-left-color: var(--danger);
  }
  .toast.success {
    border-left-color: var(--success);
  }
  .toast.info {
    border-left-color: var(--accent);
  }

  .toast span {
    flex: 1;
  }

  .toast button {
    color: var(--text-dimmer);
  }

  .toast button:hover {
    color: var(--text);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
