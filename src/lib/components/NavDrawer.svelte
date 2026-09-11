<script lang="ts">
  /** Slide-out navigation behind the hamburger, matching G HUB's menu panel. */
  import { page } from "$app/state";
  import { ui } from "$lib/stores/ui.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  const links: { href: string; label: string; icon: IconName }[] = [
    { href: "/", label: "Home", icon: "home" },
    { href: "/profiles", label: "Profiles", icon: "profile" },
    { href: "/settings", label: "Settings", icon: "gear" },
  ];

  function close() {
    ui.navOpen = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if ui.navOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={close}></div>

  <nav class="drawer" aria-label="Main navigation">
    <div class="head">
      <span class="mark">G</span>
      <div>
        <div class="title">OpenGHub</div>
        <div class="sub">Logitech G control for Linux</div>
      </div>
    </div>

    <div class="group">
      {#each links as link}
        <a
          class="link"
          class:active={page.url.pathname === link.href}
          href={link.href}
          onclick={close}
        >
          <Icon name={link.icon} size={17} />
          {link.label}
        </a>
      {/each}
    </div>

    <div class="group">
      <div class="group-title">Devices</div>
      {#each deviceStore.devices as device (device.id)}
        <a class="link device" href="/device/{device.id}" onclick={close}>
          <Icon name={device.kind === "other" ? "device" : device.kind} size={17} />
          <span class="dev-name">{device.name}</span>
          {#if !device.online}<span class="offline">offline</span>{/if}
        </a>
      {/each}
      {#if deviceStore.devices.length === 0}
        <p class="empty">No devices detected</p>
      {/if}
    </div>
  </nav>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 60;
    animation: fade 140ms var(--ease);
  }

  .drawer {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    width: 268px;
    padding: 18px 12px;
    background: #101010;
    border-right: 1px solid var(--line);
    z-index: 61;
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow-y: auto;
    animation: slide 180ms var(--ease);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 8px 0;
  }

  .mark {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    font-family: var(--font);
    font-weight: 700;
    font-size: 17px;
  }

  .title {
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .sub {
    font-size: 11px;
    color: var(--text-dimmer);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .group-title {
    padding: 4px 10px 8px;
    font-family: var(--font);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  .link {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    border-radius: var(--radius-sm);
    font-size: 13.5px;
    color: var(--text-dim);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }

  .link:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .link.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .dev-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .offline {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  .empty {
    padding: 4px 10px;
    font-size: 12px;
    color: var(--text-dimmer);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
  }
</style>
