<script lang="ts">
  /**
   * Profiles — G HUB's "All Games & Apps" screen: a filter rail on the left
   * with the profile-switching controls beneath it, and a grid of poster
   * cards on the right. Each card is a game (or the Desktop) with its profile.
   */
  import * as api from "$lib/api";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import GamePicker from "$lib/components/GamePicker.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Application, Profile } from "$lib/types";

  type Filter = "all" | "active" | "disabled";
  let filter = $state<Filter>("all");
  let search = $state("");
  let adding = $state(false);
  /** Card whose ⋮ menu is open. */
  let menuFor = $state<string | null>(null);

  const profiles = $derived(configStore.profiles);
  const counts = $derived({
    all: profiles.length,
    active: profiles.filter((p) => !p.disabled).length,
    disabled: profiles.filter((p) => p.disabled).length,
  });

  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return profiles.filter((p) => {
      if (filter === "active" && p.disabled) return false;
      if (filter === "disabled" && !p.disabled) return false;
      return !q || displayName(p).toLowerCase().includes(q);
    });
  });

  /** "Counter-Strike 2: Default" → "Counter-Strike 2"; the Desktop keeps its name. */
  function displayName(p: Profile): string {
    if (p.id === "default") return "Desktop";
    return p.name.includes(":") ? p.name.split(":")[0].trim() : p.name;
  }

  /** A device to stand in for the Desktop card's artwork. */
  const desktopDevice = $derived(deviceStore.devices.find((d) => d.kind !== "receiver") ?? null);

  async function select(p: Profile) {
    menuFor = null;
    if (p.disabled) return;
    try {
      await configStore.selectProfile(p.id);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  /** "Add games & apps": pick from Logitech's database, creating a bound profile. */
  async function addGame(app: Application | null) {
    adding = false;
    if (!app) return;
    if (profiles.some((p) => p.applicationId === app.id)) {
      ui.toast(`${app.name} is already added.`, "info");
      return;
    }
    try {
      await configStore.createProfile("Default", "game");
      const created = configStore.profiles.find((p) => p.id === configStore.activeProfileId);
      if (created) await configStore.bindApplication(created.id, app.id);
      ui.toast(`Added ${app.name}.`, "success", 2500);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  /** Card whose name is being edited, and the draft. */
  let renaming = $state<string | null>(null);
  let renameDraft = $state("");

  function startRename(p: Profile) {
    menuFor = null;
    renaming = p.id;
    renameDraft = p.id === "default" ? "Desktop" : (p.name.split(":")[1]?.trim() ?? p.name);
  }

  async function finishRename() {
    const id = renaming;
    renaming = null;
    if (!id) return;
    const name = renameDraft.trim();
    if (!name) return;
    try {
      await configStore.renameProfile(id, id === "default" ? `Desktop: ${name}` : name);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function duplicate(p: Profile) {
    menuFor = null;
    try {
      await configStore.duplicateProfile(p.id);
      ui.toast(`Duplicated ${displayName(p)}.`, "success", 2500);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function toggleDisabled(p: Profile) {
    menuFor = null;
    try {
      await configStore.setDisabled(p.id, !p.disabled);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function remove(p: Profile) {
    menuFor = null;
    try {
      await configStore.removeProfile(p.id);
      ui.toast(`Removed ${displayName(p)}.`, "success", 2500);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  /**
   * Exports a profile's settings for one connected device as a shareable
   * JSON file, ready for a pull request to the community repository.
   */
  async function share(p: Profile) {
    menuFor = null;
    const configured = deviceStore.devices.filter((d) => p.devices[d.id]);
    const device = configured[0];
    if (!device) {
      ui.toast("This profile has no device settings yet — configure a device first.", "error", 6000);
      return;
    }
    let path: string | null = null;
    try {
      const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
      const picked = await saveDialog({
        title: "Share profile",
        defaultPath: `${displayName(p).toLowerCase().replace(/[^a-z0-9]+/g, "-")}-${device.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.json`,
        filters: [{ name: "OpenGHub profile", extensions: ["json"] }],
      });
      path = typeof picked === "string" ? picked : null;
    } catch {
      ui.toast("Sharing needs the desktop app.", "error");
      return;
    }
    if (!path) return;
    try {
      const json = await api.exportProfile(p.id, device.id, "");
      await api.writeTextFile(path, json);
      ui.toast(`Saved. Add it to the community repository with a pull request.`, "success", 6000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 7000);
    }
  }

  async function save(patch: Partial<typeof configStore.settings>) {
    try {
      await configStore.saveSettings({ ...configStore.settings, ...patch });
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  function closeMenus(event: MouseEvent) {
    if (!(event.target as HTMLElement).closest(".menu, .kebab")) menuFor = null;
  }
</script>

<svelte:window onclick={closeMenus} />

<div class="page">
  <aside class="rail">
    <nav class="filters" aria-label="Filter">
      {#each [["all", "All"], ["active", "Active"], ["disabled", "Disabled"]] as [id, label] (id)}
        <button class="filter" class:active={filter === id} onclick={() => (filter = id as Filter)}>
          <span>{label}</span>
          {#if filter === id}<span class="count">{counts[id as Filter]}</span>{/if}
        </button>
      {/each}
    </nav>

    <div class="switching">
      <h2>
        Profile Switching
        <span class="info" title="Switch to a game's profile automatically when it is detected running.">
          <Icon name="info" size={15} />
        </span>
      </h2>

      <label class="check">
        <input
          type="checkbox"
          checked={configStore.settings.autoSwitchProfiles}
          onchange={(e) => save({ autoSwitchProfiles: e.currentTarget.checked })}
        />
        <span>Enable profile switching</span>
      </label>

      <div class="field">
        <span class="field-label">Select persistent profile</span>
        <div class="select">
          <select
            value={configStore.settings.persistentProfile}
            onchange={(e) => save({ persistentProfile: e.currentTarget.value })}
          >
            {#each profiles.filter((p) => !p.disabled) as p (p.id)}
              <option value={p.id}>{displayName(p)} – {p.id === "default" ? "Default" : (p.name.split(":")[1]?.trim() ?? "Default")}</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={15} />
        </div>
      </div>
    </div>
  </aside>

  <section class="main">
    <header class="head">
      <div class="head-text">
        <h1>All Games & Apps</h1>
        <p>
          Select a game or app to manage its profiles, macros and integrations. Disabling a game
          or app will prevent its profiles from becoming active. Deleting a game or app will
          remove it and its associated profiles and macros from OpenGHub.
        </p>
      </div>
      <div class="head-actions">
        <label class="search">
          <Icon name="search" size={16} />
          <input type="text" placeholder="Search" bind:value={search} spellcheck="false" />
        </label>
        <button class="add" onclick={() => (adding = true)}>
          <Icon name="plus" size={16} strokeWidth={2.2} />
          Add games & apps
        </button>
      </div>
    </header>

    {#if adding}
      <div class="picker-wrap">
        <GamePicker value={null} onselect={addGame} onclose={() => (adding = false)} />
      </div>
    {/if}

    <div class="grid">
      {#each shown as p (p.id)}
        {@const isActive = p.id === configStore.activeProfileId}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="card"
          class:selected={isActive}
          class:disabled={p.disabled}
          onclick={() => select(p)}
          role="button"
          tabindex="0"
        >
          <div class="poster">
            {#if p.posterUrl}
              <img src={p.posterUrl} alt="" loading="lazy" />
            {:else if p.id === "default" && desktopDevice}
              <div class="desktop-art">
                <DeviceArt
                  kind={desktopDevice.kind}
                  productIds={artworkIds(desktopDevice)}
                  variant="thumb"
                  glow="#00a9e0"
                  brightness={70}
                />
              </div>
            {:else}
              <div class="desktop-art blank"><Icon name="macro" size={34} strokeWidth={1.2} /></div>
            {/if}
            {#if p.disabled}
              <div class="ban" aria-hidden="true">
                <svg viewBox="0 0 48 48" width="56" height="56">
                  <circle cx="24" cy="24" r="19" fill="rgba(0,0,0,.55)" stroke="#fff" stroke-width="3.5" />
                  <path d="M11 11l26 26" stroke="#fff" stroke-width="3.5" stroke-linecap="round" />
                </svg>
              </div>
            {/if}
          </div>

          {#if renaming === p.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="name-edit"
              type="text"
              bind:value={renameDraft}
              autofocus
              onclick={(e) => e.stopPropagation()}
              onblur={finishRename}
              onkeydown={(e) => {
                if (e.key === "Enter") finishRename();
                if (e.key === "Escape") renaming = null;
              }}
            />
          {:else}
            <div class="name">{displayName(p)}</div>
          {/if}

          <div class="foot">
            {#if p.id === "default"}
              <span class="badge" title="Persistent profile"><Icon name="profile" size={18} /></span>
              <button
                class="kebab"
                aria-label="More"
                onclick={(e) => {
                  e.stopPropagation();
                  menuFor = menuFor === p.id ? null : p.id;
                }}
              >
                <svg viewBox="0 0 4 16" width="4" height="16" aria-hidden="true">
                  <circle cx="2" cy="2" r="1.8" fill="currentColor" /><circle cx="2" cy="8" r="1.8" fill="currentColor" /><circle cx="2" cy="14" r="1.8" fill="currentColor" />
                </svg>
              </button>
              {#if menuFor === p.id}
                <div class="menu" role="menu">
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); startRename(p); }}>Rename…</button>
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); duplicate(p); }}>Duplicate</button>
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); share(p); }}>Share…</button>
                </div>
              {/if}
            {:else}
              <span class="state">{p.disabled ? "Disabled" : ""}</span>
              <button
                class="kebab"
                aria-label="More"
                onclick={(e) => {
                  e.stopPropagation();
                  menuFor = menuFor === p.id ? null : p.id;
                }}
              >
                <svg viewBox="0 0 4 16" width="4" height="16" aria-hidden="true">
                  <circle cx="2" cy="2" r="1.8" fill="currentColor" /><circle cx="2" cy="8" r="1.8" fill="currentColor" /><circle cx="2" cy="14" r="1.8" fill="currentColor" />
                </svg>
              </button>
              {#if menuFor === p.id}
                <div class="menu" role="menu">
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); startRename(p); }}>Rename…</button>
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); duplicate(p); }}>Duplicate</button>
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); share(p); }}>
                    Share…
                  </button>
                  <button role="menuitem" onclick={(e) => { e.stopPropagation(); toggleDisabled(p); }}>
                    {p.disabled ? "Enable" : "Disable"}
                  </button>
                  <button role="menuitem" class="danger" onclick={(e) => { e.stopPropagation(); remove(p); }}>
                    Delete
                  </button>
                </div>
              {/if}
            {/if}
          </div>
        </div>
      {:else}
        <p class="empty">
          {#if search}No games match “{search}”.{:else}Nothing here yet.{/if}
        </p>
      {/each}
    </div>
  </section>
</div>

<style>
  .name-edit {
    width: 100%;
    padding: 4px 6px;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: #000;
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    user-select: text;
  }

  .page {
    display: grid;
    grid-template-columns: 296px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }

  /* ---- left rail ------------------------------------------------------- */
  .rail {
    display: flex;
    flex-direction: column;
    padding: 34px 18px 28px 44px;
    border-right: 1px solid var(--line);
  }

  .filters {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .filter {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 9px 12px;
    border-radius: var(--radius);
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    text-align: left;
  }

  .filter:hover {
    background: var(--surface);
  }

  .filter.active {
    color: var(--accent);
    background: rgba(17, 150, 255, 0.14);
  }

  .count {
    font-size: 13px;
    font-weight: 400;
  }

  .switching {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 0 12px;
  }

  .switching h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 16px;
    font-weight: 700;
  }

  .info {
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13.5px;
    cursor: pointer;
  }

  .check input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field-label {
    font-size: 13px;
    font-weight: 700;
  }

  .select {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-dim);
  }

  .select select {
    appearance: none;
    width: 100%;
    height: 40px;
    padding: 0 34px 0 14px;
    border: none;
    border-radius: var(--radius);
    background: var(--surface);
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    cursor: pointer;
  }

  .select :global(svg) {
    position: absolute;
    right: 12px;
    pointer-events: none;
  }

  .select option {
    background: #161616;
  }

  /* ---- main ------------------------------------------------------------ */
  .main {
    min-width: 0;
    padding: 30px 40px 40px 56px;
    overflow-y: auto;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 26px;
  }

  h1 {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.96px;
    line-height: 28px;
    margin-bottom: 8px;
  }

  .head-text p {
    max-width: 78ch;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .head-actions {
    flex: none;
    display: flex;
    gap: 14px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 230px;
    height: 46px;
    padding: 0 18px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-dim);
  }

  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    font-size: 14px;
    letter-spacing: 0.04em;
    user-select: text;
  }

  .add {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    height: 46px;
    padding: 0 22px;
    border-radius: var(--radius);
    background: var(--accent);
    font-family: var(--font);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #fff;
  }

  .add:hover {
    background: var(--accent-hover);
  }

  .picker-wrap {
    margin-bottom: 20px;
  }

  .grid {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  /* ---- cards ----------------------------------------------------------- */
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 180px;
    height: 335px;
    padding: 12px;
    border: 2px solid transparent;
    border-radius: 6px;
    background: var(--surface);
    cursor: pointer;
    transition: border-color 120ms var(--ease), background 120ms var(--ease);
  }

  .card:hover {
    border-color: var(--line-strong);
  }

  .card.selected {
    border-color: var(--accent);
    background: #0f2236;
  }

  .card.disabled {
    cursor: default;
  }

  .poster {
    position: relative;
    width: 100%;
    aspect-ratio: 4 / 5;
    border-radius: 4px;
    overflow: hidden;
    background: #0b0b0c;
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .card.disabled .poster img {
    filter: grayscale(0.4) brightness(0.55);
  }

  .desktop-art {
    width: 100%;
    height: 100%;
    padding: 14px 18px;
    background: radial-gradient(ellipse at 50% 60%, #1c1c1f, #0b0b0c 70%);
  }

  .desktop-art.blank {
    display: grid;
    place-items: center;
    color: var(--text-dimmer);
  }

  .ban {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  .name {
    margin-top: 16px;
    font-family: var(--font);
    font-size: 15px;
    font-weight: 700;
    line-height: 1.3;
  }

  .foot {
    position: relative;
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 40px;
  }

  .badge {
    color: var(--accent);
  }

  .state {
    font-size: 12px;
    color: var(--text-dim);
  }

  .kebab {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-left: auto;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    color: var(--text);
  }

  .kebab:hover {
    background: var(--surface-2);
  }

  .menu {
    position: absolute;
    right: 0;
    bottom: 46px;
    min-width: 140px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: #161616;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.6);
    z-index: 5;
  }

  .menu button {
    display: block;
    width: 100%;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text);
    text-align: left;
  }

  .menu button:hover {
    background: var(--surface-2);
  }

  .menu .danger {
    color: var(--danger);
  }

  .empty {
    padding: 40px 0;
    font-size: 13px;
    color: var(--text-dimmer);
  }
</style>
