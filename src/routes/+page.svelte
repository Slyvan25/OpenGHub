<script lang="ts">
  /** Dashboard — the device grid that G HUB opens on. */
  import * as api from "$lib/api";
  import DeviceCard from "$lib/components/DeviceCard.svelte";
  import DeviceNotice from "$lib/components/DeviceNotice.svelte";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { artworkIds, batteryIcon, connectionIcon, defaultTab, zoneGlowsFor } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { goto } from "$app/navigation";

  let refreshing = $state(false);

  // The cards need each zone's HID++ location name to place its glow. That
  // comes from the device, so learn it once per device and cache it in the
  // profile — after that the dashboard never touches the hardware for it.
  const learned = new Set<string>();
  $effect(() => {
    for (const d of deviceStore.devices) {
      if (!d.capabilities.lighting || !d.online || learned.has(d.id)) continue;
      const profile = configStore.deviceProfile(d.id);
      if ((profile.zoneNames ?? []).length > 0) {
        learned.add(d.id);
        continue;
      }
      learned.add(d.id);
      api
        .getLightingZones(d.id)
        .then((zones) =>
          configStore.saveDeviceProfile(d.id, {
            ...configStore.deviceProfile(d.id),
            zoneNames: zones.map((z) => z.locationName),
          }),
        )
        .catch(() => learned.delete(d.id));
    }
  });

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
    <!-- G HUB's view switch: a pill with grid on the left, list on the right. -->
    <div class="views" role="group" aria-label="Layout">
      <button
        class="view"
        class:active={ui.view === "grid"}
        onclick={() => ui.setView("grid")}
        aria-label="Grid view"
      >
        <Icon name="grid" size={17} />
      </button>
      <span class="divider"></span>
      <button
        class="view"
        class:active={ui.view === "list"}
        onclick={() => ui.setView("list")}
        aria-label="List view"
      >
        <Icon name="list" size={17} />
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
          zones={zoneGlowsFor(configStore.deviceProfile(device.id))}
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
              zoneProductIds={artworkIds(device)}
              variant="thumb"
              zones={zoneGlowsFor(configStore.deviceProfile(device.id))}
              glow={glowFor(device.id)}
              brightness={brightnessFor(device.id)}
            />
          </div>
          <div class="row-main">
            <div class="row-name">{device.name}</div>
            <div class="row-sub">
              {#if device.battery}
                <span class="row-pct">{device.battery.percentage}%</span>
                <Icon name={batteryIcon(device.battery)} size={16} strokeWidth={1.5} />
              {/if}
              <Icon name={connectionIcon(device.connection)} size={16} strokeWidth={1.5} />
            </div>
          </div>
          <button
            class="row-action"
            title={device.capabilities.onboardMemory ? "Onboard memory mode" : "Device settings"}
            aria-label={device.capabilities.onboardMemory ? "Onboard memory mode" : "Device settings"}
            onclick={(e) => {
              e.stopPropagation();
              goto(`/device/${device.id}/settings`);
            }}
          >
            <Icon name={device.capabilities.onboardMemory ? "onboardOff" : "gear"} size={20} strokeWidth={1.6} />
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    --card-w: 328px;
    max-width: var(--content-max);
    margin: 0 auto;
    padding: 0 var(--content-pad) 40px;
    padding-top: 30px;
    width: 100%;
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
    align-items: center;
    margin-left: 8px;
    padding: 3px;
    border-radius: var(--radius-lg);
    background: var(--surface);
  }

  .view {
    display: grid;
    place-items: center;
    width: 34px;
    height: 32px;
    border-radius: var(--radius);
    color: var(--text-dim);
    transition: color 120ms var(--ease), background 120ms var(--ease);
  }

  .view:hover {
    color: var(--text);
  }

  .view.active {
    color: var(--accent);
    background: var(--surface-2);
  }

  .divider {
    width: 1px;
    height: 18px;
    margin: 0 2px;
    background: var(--line-strong);
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

  .tool.spinning :global(svg) {
    animation: spin 900ms linear infinite;
  }

  .grid {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 14px;
  }

  .grid > :global(.card) {
    width: var(--card-w);
    flex: none;
  }

  .grid > :global(.card.wide) {
    width: calc(var(--card-w) * 2 + 14px);
  }

  .skeleton {
    width: var(--card-w);
    height: 395px;
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

  /* G HUB's list row: a wide card with the thumb, a big name and the status. */
  .row {
    display: flex;
    align-items: center;
    gap: 40px;
    min-height: 130px;
    padding: 18px 20px 18px 40px;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: var(--surface);
    cursor: pointer;
    transition: background 130ms var(--ease), border-color 130ms var(--ease);
  }

  .row:hover {
    background: var(--surface-hover);
    border-color: var(--line-strong);
  }

  .thumb {
    width: 96px;
    height: 96px;
    flex: none;
  }

  .row-main {
    flex: 1;
    min-width: 0;
  }

  .row-name {
    font-family: var(--font);
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .row-sub {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 10px;
    color: var(--text-dimmer);
  }

  .row-pct {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-dim);
  }

  .row-action {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: var(--radius);
    background: var(--surface-2);
    color: var(--text-dim);
    flex: none;
  }

  .row-action:hover {
    background: var(--surface-3);
    color: var(--text);
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
    .page {
      --card-w: 300px;
    }
  }

  @media (max-width: 880px) {
    .page {
      --card-w: 260px;
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
