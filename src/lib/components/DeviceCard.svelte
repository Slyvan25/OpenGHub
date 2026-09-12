<script lang="ts">
  /** A dashboard tile: name, status row, artwork and the quick-action buttons. */
  import { goto } from "$app/navigation";
  import type { Device } from "$lib/types";
  import { artworkIds, batteryIcon, connectionIcon, defaultTab, kindIcon } from "$lib/device-ui";
  import DeviceArt from "./DeviceArt.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    device: Device;
    /** Per-device lighting colour from the active profile, for the render's glow. */
    glow?: string | null;
    brightness?: number;
  }

  let { device, glow = null, brightness = 100 }: Props = $props();

  interface Action {
    icon: IconName;
    label: string;
    href: string;
    primary?: boolean;
  }

  const actions = $derived.by<Action[]>(() => {
    const list: Action[] = [];
    if (device.capabilities.dpi) {
      list.push({ icon: "dpi", label: "Sensitivity", href: `/device/${device.id}/sensitivity` });
    }
    if (device.kind === "keyboard") {
      list.push({
        icon: "keycap",
        label: "Assignments",
        href: `/device/${device.id}/assignments`,
        primary: true,
      });
    }
    if (device.capabilities.lighting) {
      list.push({ icon: "sun", label: "Brightness", href: `/device/${device.id}/lighting` });
      list.push({ icon: "lightOff", label: "Lighting", href: `/device/${device.id}/lighting` });
    }
    if (list.length === 0) {
      list.push({ icon: "pencil", label: "Configure", href: `/device/${device.id}/settings` });
    }
    return list;
  });

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
        <Icon name={batteryIcon(device.battery)} size={13} strokeWidth={1.5} />
      {/if}
      {#if device.capabilities.onboardMemory}
        <Icon name="chip" size={13} strokeWidth={1.5} />
      {/if}
      <Icon name={connectionIcon(device.connection)} size={13} strokeWidth={1.5} />
      {#if device.capabilities.lighting}
        <Icon name="eye" size={13} strokeWidth={1.5} />
      {/if}
    </div>
  </header>

  <div class="art">
    <DeviceArt kind={device.kind} productIds={artworkIds(device)} variant="thumb" {glow} {brightness} />
  </div>

  {#if device.lastError}
    <p class="err" title={device.lastError}>
      <Icon name="alert" size={12} />
      {device.lastError}
    </p>
  {/if}

  <footer>
    {#each actions as action}
      <button
        class="action"
        class:primary={action.primary}
        title={action.label}
        aria-label={action.label}
        onclick={(e) => openAction(e, action.href)}
      >
        <Icon name={action.icon} size={15} strokeWidth={1.6} />
      </button>
    {/each}
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
    min-height: 296px;
    padding: 16px 18px 14px;
    border: 1px solid transparent;
    border-radius: var(--radius);
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

  .card.wide {
    grid-column: span 2;
  }

  .card.offline {
    opacity: 0.62;
  }

  header {
    flex: none;
  }

  h3 {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.01em;
    margin-bottom: 4px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 18px;
    color: var(--text-dimmer);
  }

  .pct {
    font-size: 11.5px;
    letter-spacing: 0.02em;
    color: var(--text-dim);
  }

  .pct.low {
    color: var(--warning);
  }

  .art {
    flex: 1;
    min-height: 0;
    padding: 12px 4px 4px;
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
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
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

  .action.primary {
    background: var(--accent);
    color: #fff;
  }

  .action.primary:hover {
    background: var(--accent-hover);
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
