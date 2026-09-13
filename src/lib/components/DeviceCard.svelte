<script lang="ts">
  /** A dashboard tile: name, status row, artwork and the quick-action buttons. */
  import { goto } from "$app/navigation";
  import type { Device } from "$lib/types";
  import { artworkIds, batteryIcon, connectionIcon, defaultTab, kindIcon } from "$lib/device-ui";
  import DeviceArt, { type ZoneGlow } from "./DeviceArt.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    device: Device;
    /** Per-device lighting colour from the active profile, for the render's glow. */
    glow?: string | null;
    brightness?: number;
    /** Per-zone lighting; takes precedence over `glow` when present. */
    zones?: ZoneGlow[];
  }

  let { device, glow = null, brightness = 100, zones = [] }: Props = $props();

  /**
   * G HUB's cards carry one button, bottom right: onboard memory mode. Its
   * settings live on the device's settings tab here.
   */
  const settingsHref = $derived(`/device/${device.id}/settings`);

  function open() {
    goto(`/device/${device.id}/${defaultTab(device)}`);
  }

  function openAction(event: MouseEvent, href: string) {
    event.stopPropagation();
    goto(href);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="card"
  class:wide={device.kind === "keyboard"}
  class:offline={!device.online}
  onclick={open}
  role="button"
  tabindex="0"
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && open()}
>
  <header>
    <h3>{device.name}</h3>
    <div class="status">
      {#if device.battery}
        <span class="pct" class:low={device.battery.percentage <= 20}>
          {device.battery.percentage}%
        </span>
        <Icon name={batteryIcon(device.battery)} size={16} strokeWidth={1.5} />
      {/if}
      <!-- {#if device.capabilities.onboardMemory}
        <Icon name="chip" size={13} strokeWidth={1.5} />
      {/if} -->
      <Icon name={connectionIcon(device.connection)} size={16} strokeWidth={1.5} />
      <!-- {#if device.capabilities.lighting}
        <Icon name="eye" size={13} strokeWidth={1.5} />
      {/if} -->
    </div>
  </header>

  <div class="art">
    <DeviceArt
      kind={device.kind}
      productIds={artworkIds(device)}
      zoneProductIds={artworkIds(device)}
      variant="thumb"
      {zones}
      {glow}
      {brightness}
    />
  </div>

  {#if device.lastError}
    <p class="err" title={device.lastError}>
      <Icon name="alert" size={12} />
      {device.lastError}
    </p>
  {/if}

  <footer>
    <button
      class="action"
      title={device.capabilities.onboardMemory ? "Onboard memory mode" : "Device settings"}
      aria-label={device.capabilities.onboardMemory ? "Onboard memory mode" : "Device settings"}
      onclick={(e) => openAction(e, settingsHref)}
    >
      <Icon name={device.capabilities.onboardMemory ? "onboardOff" : "gear"} size={20} strokeWidth={1.6} />
    </button>
  </footer>

  {#if !device.online}
    <div class="badge"><Icon name={kindIcon(device.kind)} size={12} /> Offline</div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 395px;
    padding: 22px 22px 18px;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: var(--surface);
    cursor: pointer;
    transition:
      background 140ms var(--ease),
      border-color 140ms var(--ease),
      transform 140ms var(--ease);
  }

  .card:hover {
    background: var(--surface-hover);
    border-color: var(--line-strong);
  }

  .card:active {
    transform: scale(0.995);
  }

  .card.offline {
    opacity: 0.62;
  }

  header {
    flex: none;
  }

  h3 {
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin-bottom: 8px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 18px;
    color: var(--text-dimmer);
  }

  .pct {
    font-size: 1rem;
    letter-spacing: 0.02em;
    color: var(--text-dim);
    font-weight: bold;
  }

  .pct.low {
    color: var(--warning);
  }

  .art {
    flex: 1;
    min-height: 0;
    padding: 12px 24px 4px;
    overflow: hidden;
  }

  .err {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
    font-size: 11px;
    color: var(--warning);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  footer {
    flex: none;
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    min-height: 28px;
  }

  .action {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    background: var(--surface-2);
    color: var(--text-dim);
    transition:
      background 120ms var(--ease),
      color 120ms var(--ease);
  }

  .action:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .badge {
    position: absolute;
    top: 14px;
    right: 16px;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
</style>
