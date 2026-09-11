<script lang="ts">
  /** Dashboard — the device grid that G HUB opens on. */
  import * as api from "$lib/api";
  import DeviceCard from "$lib/components/DeviceCard.svelte";
  import DeviceNotice from "$lib/components/DeviceNotice.svelte";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { artworkIds, batteryLabel, connectionLabel, defaultTab, kindIcon } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { goto } from "$app/navigation";

  let refreshing = $state(false);

  /** Devices with a `receiver` kind are dongles, not something to configure. */
  const visible = $derived(deviceStore.devices.filter((d) => d.kind !== "receiver"));

  function glowFor(deviceId: string) {
    const lighting = configStore.deviceProfile(deviceId).lighting;
    if (!lighting || lighting.effect === "off") return null;
    return lighting.color;
  }

  function brightnessFor(deviceId: string) {
    return configStore.deviceProfile(deviceId).lighting?.brightness ?? 100;
  }

  async function refresh() {
    refreshing = true;
    try {
      await deviceStore.load(true);
      ui.toast(`Found ${deviceStore.devices.length} device(s).`, "success");
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      refreshing = false;
    }
  }
</script>

<div class="page">
  <DeviceNotice reason={deviceStore.emptyReason} demo={deviceStore.demo} onrescan={refresh} />

  <div class="toolbar">
    <button class="tool" class:spinning={refreshing} onclick={refresh} aria-label="Rescan devices">
      <Icon name="refresh" size={17} />
    </button>
    <div class="views">
      <button
        class="tool"
        class:active={ui.view === "list"}
        onclick={() => ui.setView("list")}
        aria-label="List view"
      >
        <Icon name="list" size={17} />
      </button>
      <button
        class="tool"
        class:active={ui.view === "grid"}
        onclick={() => ui.setView("grid")}
        aria-label="Grid view"
      >
        <Icon name="grid" size={17} />
      </button>
    </div>
  </div>

  {#if deviceStore.loading && visible.length === 0}
    <div class="grid">
      {#each Array(4) as _}
        <div class="skeleton"></div>
      {/each}
    </div>
  {:else if visible.length === 0}
    <div class="empty">
      <Icon name="device" size={38} strokeWidth={1.2} />
      <h2>No devices detected</h2>
      <p>Connect a Logitech device, then rescan.</p>
      <button class="primary" onclick={refresh}>Rescan</button>
    </div>
  {:else if ui.view === "grid"}
    <div class="grid">
      {#each visible as device (device.id)}
        <DeviceCard
          {device}
          glow={glowFor(device.id)}
          brightness={brightnessFor(device.id)}
        />
      {/each}
    </div>
  {:else}
    <div class="rows">
      {#each visible as device (device.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          role="button"
          tabindex="0"
          onclick={() => goto(`/device/${device.id}/${defaultTab(device)}`)}
          onkeydown={(e) => e.key === "Enter" && goto(`/device/${device.id}/${defaultTab(device)}`)}
        >
          <div class="thumb">
            <DeviceArt
              kind={device.kind}
              productIds={artworkIds(device)}
              glow={glowFor(device.id)}
              brightness={brightnessFor(device.id)}
            />
          </div>
          <div class="row-main">
            <div class="row-name">
              <Icon name={kindIcon(device.kind)} size={15} />
              {device.name}
            </div>
            <div class="row-sub">
              {connectionLabel(device.connection)}
              · HID++ {device.protocolVersion || "—"}
              {#if device.dpi}· {device.dpi.current.toLocaleString()} DPI{/if}
              {#if device.reportRate}· {device.reportRate.currentHz} Hz{/if}
            </div>
          </div>
          {#if device.battery}
            <div class="row-battery">
              <span>{device.battery.percentage}%</span>
              <small>{batteryLabel(device.battery)}</small>
            </div>
          {/if}
          <Icon name="chevronRight" size={17} class="row-chev" />
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    max-width: var(--content-max);
    margin: 0 auto;
    padding: 0 var(--content-pad) 40px;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    height: 40px;
    margin-bottom: 8px;
  }

  .views {
    display: flex;
    gap: 2px;
    margin-left: 8px;
  }

  .tool {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    color: var(--text-dimmer);
    transition: color 120ms var(--ease), background 120ms var(--ease);
  }

  .tool:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  .tool.active {
    color: var(--text);
  }

  .tool.spinning :global(svg) {
    animation: spin 900ms linear infinite;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 14px;
  }

  .skeleton {
    min-height: 296px;
    border-radius: var(--radius);
    background: linear-gradient(100deg, var(--surface) 30%, #232323 50%, var(--surface) 70%);
    background-size: 300% 100%;
    animation: shimmer 1.4s linear infinite;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: var(--surface);
    cursor: pointer;
    transition: background 130ms var(--ease), border-color 130ms var(--ease);
  }

  .row:hover {
    background: var(--surface-hover);
    border-color: var(--line-strong);
  }

  .thumb {
    width: 72px;
    height: 56px;
    flex: none;
  }

  .row-main {
    flex: 1;
    min-width: 0;
  }

  .row-name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
  }

  .row-sub {
    margin-top: 3px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .row-battery {
    text-align: right;
    flex: none;
  }

  .row-battery span {
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
  }

  .row-battery small {
    display: block;
    font-size: 11px;
    color: var(--text-dimmer);
  }

  .row :global(.row-chev) {
    color: var(--text-dimmer);
    flex: none;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 90px 0;
    color: var(--text-dimmer);
    text-align: center;
  }

  .empty h2 {
    font-size: 18px;
    color: var(--text);
  }

  .empty p {
    font-size: 13px;
  }

  .primary {
    margin-top: 8px;
    padding: 9px 20px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: #fff;
    font-size: 13px;
  }

  .primary:hover {
    background: var(--accent-hover);
  }

  @media (max-width: 1150px) {
    .grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 880px) {
    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @keyframes shimmer {
    to {
      background-position: -300% 0;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
