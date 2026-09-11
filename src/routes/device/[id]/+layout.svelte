<script lang="ts">
  /**
   * Device page chrome, laid out like G HUB: profile header across the top, a
   * narrow icon rail down the left, and the active section filling the rest.
   */
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Icon from "$lib/components/Icon.svelte";
  import { batteryIcon, batteryLabel, connectionLabel, defaultTab, tabsFor } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let { children } = $props();

  const deviceId = $derived(page.params.id!);
  const device = $derived(deviceStore.get(deviceId));
  const currentTab = $derived(page.params.tab ?? (device ? defaultTab(device) : "settings"));
  const tabs = $derived(device ? tabsFor(device) : []);
  /** The gear sits apart from the rail, as in G HUB. */
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
      <button class="back" onclick={() => goto("/")} aria-label="Back to dashboard">
        <Icon name="arrowLeft" size={20} />
      </button>

      <div class="profile">
        <div class="profile-kind">Persistent profile</div>
        <button class="profile-name" onclick={() => (ui.profilePickerOpen = true)}>
          <Icon name="lock" size={13} />
          <strong>{configStore.active?.name ?? "Desktop: Default"}</strong>
          <Icon name="chevronDown" size={14} />
        </button>
      </div>

      <div class="status">
        {#if device.battery}
          <span class:low={device.battery.percentage <= 20}>
            <Icon name={batteryIcon(device.battery)} size={13} />
            {device.battery.percentage}% · {batteryLabel(device.battery)}
          </span>
        {/if}
        <span>{connectionLabel(device.connection)}</span>
      </div>

      <a
        class="gear"
        class:active={currentTab === "settings"}
        href="/device/{device.id}/settings"
        aria-label="Device settings"
      >
        <Icon name="gear" size={19} />
      </a>
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
            <Icon name={tab.icon} size={21} strokeWidth={1.6} />
          </a>
        {/each}
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
    max-width: var(--content-max);
    margin: 0 auto;
    padding: 0 var(--content-pad) 18px;
  }

  .head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 0 18px;
  }

  .back {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
  }

  .back:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .profile-kind {
    font-size: 10.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .profile-name {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 1px;
    color: var(--cyan);
    font-size: 15px;
  }

  .profile-name strong {
    font-family: var(--font);
    font-weight: 600;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .profile-name:hover {
    filter: brightness(1.15);
  }

  .status {
    margin-left: auto;
    display: flex;
    gap: 16px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .status span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .status .low {
    color: var(--warning);
  }

  .gear {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
  }

  .gear:hover,
  .gear.active {
    background: var(--surface-2);
    color: var(--text);
  }

  .body {
    flex: 1;
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    gap: 10px;
    min-height: 0;
  }

  .rail {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tab {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text-dim);
    transition: background 130ms var(--ease), color 130ms var(--ease);
  }

  .tab:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tab.active {
    background: var(--accent);
    color: #fff;
  }

  .content {
    display: flex;
    min-width: 0;
    min-height: 0;
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
