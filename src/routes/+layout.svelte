<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import PermissionsSetup from "$lib/components/PermissionsSetup.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import type { Config } from "$lib/types";
  import TopBar from "$lib/components/TopBar.svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let { children } = $props();

  /**
   * Device screens and community profile pages carry their own back arrow
   * instead of the tab bar, as in G HUB.
   */
  const showTopBar = $derived(
    !page.url.pathname.startsWith("/device/") && !/^\/community\/.+/.test(page.url.pathname),
  );

  /**
   * Fades out the splash from `app.html` — the markup that paints before
   * SvelteKit has booted — once the stores hold real data.
   */
  function dismissSplash() {
    const splash = document.getElementById("splash");
    if (!splash) return;
    splash.classList.add("done");
    setTimeout(() => splash.remove(), 400);
  }

  // No native frame, so the edges have to start the resize themselves.
  const edges = ["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"] as const;

  api.installErrorForwarding();

  onMount(() => {
    ui.restore();

    let unsubscribe: (() => void) | undefined;
    (async () => {
      unsubscribe = await deviceStore.subscribe();
      // The backend switches profiles itself when a bound game starts or
      // stops; mirror that here so the picker and every editor follow along.
      const unConfig = await api.on<Config>(api.events.configChanged, (cfg) =>
        configStore.apply(cfg),
      );
      const unApp = await api.on<string | null>(api.events.activeApplication, (id) => {
        if (id) ui.toast("Game detected — profile switched.", "info", 3000);
      });
      const unArt = await api.on(api.events.artworkChanged, () => {
        artwork.load();
        ui.toast("Device artwork downloaded.", "success", 3000);
      });
      // The tray menu's Devices / Games / Community / Profiles entries.
      const unNav = await api.on<string>(api.events.navigate, (route) => goto(route));
      const prev = unsubscribe;
      unsubscribe = () => {
        prev?.();
        unConfig();
        unApp();
        unArt();
        unNav();
      };
      await Promise.all([configStore.load(), deviceStore.load(), artwork.load()]);
      dismissSplash();
      if (!api.isTauri) {
        ui.toast("Running outside Tauri — showing mock devices.", "info", 6000);
      }
    })().catch((e) => {
      dismissSplash();
      ui.toast(api.errorMessage(e), "error");
    });

    return () => unsubscribe?.();
  });
</script>

<div class="shell">
  <TitleBar />
  {#if showTopBar}
    <TopBar />
  {/if}
  <main>
    {@render children?.()}
  </main>
</div>

{#if api.isTauri}
  {#each edges as dir (dir)}
    <div
      class="edge {dir}"
      aria-hidden="true"
      onpointerdown={(e) => e.button === 0 && getCurrentWindow().startResizeDragging(dir)}
    ></div>
  {/each}
{/if}

<PermissionsSetup />
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
    display: flex;
    flex-direction: column;
  }

  .edge {
    position: fixed;
    z-index: 1000;
  }

  .North,
  .South {
    left: 10px;
    right: 10px;
    height: 6px;
    cursor: ns-resize;
  }

  .North {
    top: 0;
  }

  .South {
    bottom: 0;
  }

  .East,
  .West {
    top: 10px;
    bottom: 10px;
    width: 6px;
    cursor: ew-resize;
  }

  .East {
    right: 0;
  }

  .West {
    left: 0;
  }

  .NorthEast,
  .NorthWest,
  .SouthEast,
  .SouthWest {
    width: 12px;
    height: 12px;
  }

  .NorthWest {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }

  .SouthEast {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }

  .NorthEast {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }

  .SouthWest {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }

  main > :global(*) {
    flex: 1;
    min-height: 0;
  }
</style>
