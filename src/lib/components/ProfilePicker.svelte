<script lang="ts">
  /**
   * The profile pill in the top bar. G HUB keys every setting on the active
   * application profile, so this is the most prominent control in the chrome.
   */
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import Icon from "./Icon.svelte";

  let root: HTMLDivElement | undefined = $state();

  const active = $derived(configStore.active);

  function toggle() {
    ui.profilePickerOpen = !ui.profilePickerOpen;
  }

  async function select(id: string) {
    ui.profilePickerOpen = false;
    await configStore.selectProfile(id);
  }

  function onWindowClick(event: MouseEvent) {
    if (ui.profilePickerOpen && root && !root.contains(event.target as Node)) {
      ui.profilePickerOpen = false;
    }
  }
</script>

<svelte:window onclick={onWindowClick} />

<div class="picker" bind:this={root}>
  <button class="pill" class:open={ui.profilePickerOpen} onclick={toggle}>
    <span class="badge"><Icon name="profile" size={13} /></span>
    <span class="label">{active?.name ?? "Desktop: Default"}</span>
    <span class="chev" class:flipped={ui.profilePickerOpen}>
      <Icon name="chevronDown" size={15} />
    </span>
  </button>

  {#if ui.profilePickerOpen}
    <div class="menu">
      <div class="menu-head">Profiles</div>
      {#each configStore.profiles as profile (profile.id)}
        <button
          class="item"
          class:selected={profile.id === configStore.activeProfileId}
          onclick={() => select(profile.id)}
        >
          <Icon name={profile.kind === "desktop" ? "profile" : "macro"} size={14} />
          <span>{profile.name}</span>
          {#if profile.id === configStore.activeProfileId}
            <Icon name="check" size={14} class="tick" />
          {/if}
        </button>
      {/each}
      <a class="item manage" href="/profiles" onclick={() => (ui.profilePickerOpen = false)}>
        <Icon name="gear" size={14} />
        <span>Manage profiles</span>
      </a>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    font-weight: bold;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 256px;
    height: 40px;
    padding: 0 12px 0 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    transition: background 120ms var(--ease), border-color 120ms var(--ease);
  }

  .pill:hover,
  .pill.open {
    background: var(--surface-hover);
    border-color: var(--line-strong);
  }

  .badge {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 3px;
    background: var(--surface-3);
    color: var(--text-dim);
  }

  .label {
    flex: 1;
    text-align: left;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chev {
    color: var(--text-dim);
    display: flex;
    transition: transform 160ms var(--ease);
  }

  .chev.flipped {
    transform: rotate(180deg);
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    min-width: 260px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: #141414;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
    z-index: 50;
  }

  .menu-head {
    padding: 6px 10px 8px;
    font-family: var(--font);
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text);
    text-align: left;
    font-weight: bold;
  }

  .item:hover {
    background: var(--surface-2);
  }

  .item.selected {
    color: var(--accent);
  }

  .item span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .manage {
    margin-top: 4px;
    border-top: 1px solid var(--line);
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    padding-top: 12px;
    color: var(--text-dim);
  }
</style>
