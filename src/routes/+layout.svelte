<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import NavDrawer from "$lib/components/NavDrawer.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import TopBar from "$lib/components/TopBar.svelte";

  let { children } = $props();

  onMount(() => {
    ui.restore();

    let unsubscribe: (() => void) | undefined;
    (async () => {
      unsubscribe = await deviceStore.subscribe();
      await Promise.all([configStore.load(), deviceStore.load(), artwork.load()]);
      if (!api.isTauri) {
        ui.toast("Running outside Tauri — showing mock devices.", "info", 6000);
      }
    })().catch((e) => ui.toast(api.errorMessage(e), "error"));

    return () => unsubscribe?.();
  });
</script>

<div class="shell">
  <TitleBar />
  <TopBar />
  <main>
    {@render children?.()}
  </main>
</div>

<NavDrawer />
<Toasts />

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
  }

  main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
</style>
