<script lang="ts">
  /**
   * Device page chrome, laid out like G HUB: the top bar with its tabs is gone,
   * replaced by "← DEVICE NAME" with the profile picker on the right; a narrow
   * icon rail runs down the left with the gear parked at the bottom, and the
   * active section fills the rest.
   */
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Icon from "$lib/components/Icon.svelte";
  import ProfilePicker from "$lib/components/ProfilePicker.svelte";
  import { defaultTab, tabsFor } from "$lib/device-ui";
  import { deviceStore } from "$lib/stores/devices.svelte";

  let { children } = $props();

  const deviceId = $derived(page.params.id!);
  const device = $derived(deviceStore.get(deviceId));
  const currentTab = $derived(page.params.tab ?? (device ? defaultTab(device) : "settings"));
  const tabs = $derived(device ? tabsFor(device) : []);
  const railTabs = $derived(tabs.filter((t) => t.id !== "settings"));
</script>

{#if !device}
  <div class="missing">
    <Icon name="alert" size={32} strokeWidth={1.3} />
    <h2>Device not available</h2>
    <p>It may have been unplugged or gone to sleep.</p>
    <a href="/">Back to dashboard</a>
  </div>
{:else}
  <div class="device-page">
    <header class="head">
      <button class="back" onclick={() => goto("/")} aria-label="Back to devices">
        <Icon name="arrowLeft" size={22} strokeWidth={1.8} />
      </button>
      <h1>{device.name}</h1>

      <div class="right">
        <ProfilePicker />
        <button class="icon-btn" aria-label="Account">
          <Icon name="user" size={20} strokeWidth={1.7} />
        </button>
      </div>
    </header>

    <div class="body">
      <nav class="rail" aria-label="Device sections">
        {#each railTabs as tab (tab.id)}
          <a
            class="tab"
            class:active={tab.id === currentTab}
            href="/device/{device.id}/{tab.id}"
            title={tab.label}
            aria-label={tab.label}
          >
            <Icon name={tab.icon} size={22} strokeWidth={1.7} />
          </a>
        {/each}
        <a
          class="tab gear"
          class:active={currentTab === "settings"}
          href="/device/{device.id}/settings"
          title="Settings"
          aria-label="Device settings"
        >
          <Icon name="gear" size={22} strokeWidth={1.7} />
        </a>
      </nav>

      <div class="content">
        {@render children?.()}
      </div>
    </div>
  </div>
{/if}

<style>
  .device-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 0 0 20px;
  }

  /* Same height as the top bar it replaces, so the page doesn't jump. */
  .head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 18px;
    height: var(--topbar-h);
    padding: 0 24px 0 24px;
  }

  .back {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    color: var(--text);
  }

  .back:hover {
    background: var(--surface);
  }

  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.6px;
    line-height: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 22px;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    color: var(--text);
  }

  .icon-btn:hover {
    background: var(--surface);
  }

  .body {
    flex: 1;
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    min-height: 0;
    padding-top: 12px;
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding-top: 22px;
  }

  .tab {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: var(--radius);
    color: var(--text-dim);
    transition: background 130ms var(--ease), color 130ms var(--ease);
  }

  .tab:hover {
    background: var(--surface);
    color: var(--text);
  }

  .tab.active {
    background: var(--accent);
    color: #fff;
  }

  .tab.gear {
    margin-top: auto;
    background: var(--surface);
    color: var(--text);
  }

  .tab.gear.active {
    background: var(--accent);
  }

  .content {
    display: flex;
    min-width: 0;
    min-height: 0;
    padding-right: 24px;
    overflow: hidden;
  }

  .missing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 100px 0;
    color: var(--text-dimmer);
  }

  .missing h2 {
    font-size: 18px;
    color: var(--text);
  }
</style>
