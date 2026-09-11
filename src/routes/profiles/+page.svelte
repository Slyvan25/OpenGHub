<script lang="ts">
  /** Profile manager: create, switch and delete application profiles. */
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let newName = $state("");
  let busy = $state(false);

  async function create(event: SubmitEvent) {
    event.preventDefault();
    const name = newName.trim();
    if (!name) return;
    busy = true;
    try {
      await configStore.createProfile(name);
      newName = "";
      ui.toast(`Created “${name}”.`, "success");
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      busy = false;
    }
  }

  async function remove(id: string, name: string) {
    try {
      await configStore.removeProfile(id);
      ui.toast(`Deleted “${name}”.`, "success");
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  function configuredCount(profileId: string): number {
    const profile = configStore.profiles.find((p) => p.id === profileId);
    return profile ? Object.keys(profile.devices).length : 0;
  }
</script>

<div class="page">
  <header>
    <h1>Profiles</h1>
    <p>
      Every DPI stage, lighting effect and binding belongs to a profile. Switch profiles from
      the picker in the top bar; the active one is applied to all
      {deviceStore.devices.length} detected device(s).
    </p>
  </header>

  <form class="create card" onsubmit={create}>
    <Icon name="plus" size={16} />
    <input
      type="text"
      placeholder="New profile name, e.g. Counter-Strike"
      bind:value={newName}
      maxlength="48"
      spellcheck="false"
    />
    <button type="submit" class="primary" disabled={busy || !newName.trim()}>Create</button>
  </form>

  <div class="list">
    {#each configStore.profiles as profile (profile.id)}
      <div class="profile card" class:active={profile.id === configStore.activeProfileId}>
        <span class="icon">
          <Icon name={profile.kind === "desktop" ? "profile" : "macro"} size={17} />
        </span>

        <div class="info">
          <div class="name">{profile.name}</div>
          <div class="sub">
            {configuredCount(profile.id)} device(s) configured
            {#if profile.id === configStore.activeProfileId}· <em>active</em>{/if}
          </div>
        </div>

        {#if profile.id !== configStore.activeProfileId}
          <button class="ghost" onclick={() => configStore.selectProfile(profile.id)}>
            Activate
          </button>
        {/if}

        {#if profile.id !== "default"}
          <button
            class="ghost danger"
            aria-label="Delete profile"
            onclick={() => remove(profile.id, profile.name)}
          >
            <Icon name="trash" size={15} />
          </button>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .page {
    max-width: 880px;
    margin: 0 auto;
    padding: 0 var(--content-pad) 50px;
  }

  header {
    margin-bottom: 22px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
  }

  header p {
    margin-top: 6px;
    max-width: 68ch;
    font-size: 13px;
    color: var(--text-dim);
    line-height: 1.6;
  }

  .create {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    margin-bottom: 14px;
    color: var(--text-dimmer);
  }

  .create input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 13.5px;
    color: var(--text);
    user-select: text;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .profile {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border: 1px solid transparent;
  }

  .profile.active {
    border-color: var(--accent);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text-dim);
  }

  .profile.active .icon {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .info {
    flex: 1;
    min-width: 0;
  }

  .name {
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
  }

  .sub {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-dimmer);
  }

  .sub em {
    color: var(--accent);
    font-style: normal;
  }

  .ghost {
    padding: 7px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    color: var(--text-dim);
  }

  .ghost:hover {
    border-color: var(--line-strong);
    color: var(--text);
  }

  .ghost.danger {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    padding: 0;
  }

  .ghost.danger:hover {
    border-color: var(--danger);
    color: var(--danger);
  }

  .primary {
    padding: 8px 18px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: #fff;
    font-size: 13px;
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .primary:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
