<script lang="ts">
  /**
   * Custom window chrome. The native decorations are off (`tauri.conf.json`), so
   * the drag region and the three window buttons live here.
   */
  import * as api from "$lib/api";
  import Icon from "./Icon.svelte";

  let maximized = $state(false);

  async function toggleMaximize() {
    maximized = await api.windowToggleMaximize();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <!-- <span class="mark">G</span> -->
    <!-- <span class="name">OpenGHub</span> -->
  </div>

  <div class="controls">
    <button class="ctrl" onclick={() => api.windowMinimize()} aria-label="Minimise">
      <Icon name="minimize" size={14} strokeWidth={5} />
    </button>
    <button class="ctrl" onclick={toggleMaximize} aria-label={maximized ? "Restore" : "Maximise"}>
      <Icon name={maximized ? "restore" : "maximize"} size={12} strokeWidth={5} />
    </button>
    <button class="ctrl close" onclick={() => api.windowClose()} aria-label="Close">
      <Icon name="close" size={14} strokeWidth={5} />
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: var(--titlebar-h);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 12px;
    background: var(--bg);
    z-index: 40;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 7px;
    pointer-events: none;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    background: var(--accent);
    color: #fff;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    line-height: 1;
  }

  .name {
    font-family: var(--font);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  .controls {
    display: flex;
    height: 100%;
  }

  .ctrl {
    display: grid;
    place-items: center;
    width: 44px;
    height: 100%;
    color: var(--text-dim);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }

  .ctrl:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .ctrl.close:hover {
    background: #c4291f;
    color: #fff;
  }
</style>
