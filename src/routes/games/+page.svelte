<script lang="ts">
  /**
   * Games — G HUB's library / launcher screen. A thin rail on the far left
   * (Library, Manage, Support), a filter column with the launchers and their
   * counts, and a poster grid of everything installed. Selecting a tile shows
   * its name; double-click (or Play) starts it through its own launcher.
   *
   * Sources are whatever is on this machine: Steam, Heroic (Epic / GOG),
   * Lutris, and executables the user added by hand.
   */
  import { goto } from "$app/navigation";
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import type { IconName } from "$lib/components/Icon.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Game, GameSource, ManualGame } from "$lib/types";

  type View = "library" | "manage";
  type SourceFilter = "all" | GameSource;
  type Sort = "recent" | "name" | "playtime";

  const SOURCES: { id: SourceFilter; label: string; icon: IconName }[] = [
    { id: "all", label: "All games", icon: "library" },
    { id: "manual", label: "Manually installed", icon: "manual" },
    { id: "steam", label: "Steam", icon: "steam" },
    { id: "epic", label: "Epic Games", icon: "epic" },
    { id: "gog", label: "GOG", icon: "gog" },
    { id: "lutris", label: "Lutris", icon: "lutris" },
  ];

  const SOURCE_LABEL: Record<GameSource, string> = {
    steam: "Steam",
    epic: "Epic Games",
    gog: "GOG",
    lutris: "Lutris",
    manual: "Manually installed",
  };

  let view = $state<View>("library");
  let games = $state<Game[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let source = $state<SourceFilter>("all");
  let search = $state("");
  let sort = $state<Sort>("recent");
  let layout = $state<"grid" | "list">("grid");
  let selected = $state<string | null>(null);
  let sourcesOpen = $state(true);
  let featuresOpen = $state(false);
  let onlyWithProfile = $state(false);
  let adding = $state(false);
  let launching = $state<string | null>(null);

  /** Local cover paths need the asset protocol under Tauri. */
  let toSrc = $state<(p: string) => string>((p) => p);

  const counts = $derived.by(() => {
    const c: Record<SourceFilter, number> = { all: games.length, manual: 0, steam: 0, epic: 0, gog: 0, lutris: 0 };
    for (const g of games) c[g.source] += 1;
    return c;
  });

  /** A profile bound to this game's Logitech application, if any. */
  function profileFor(g: Game) {
    if (!g.applicationId) return null;
    return configStore.profiles.find((p) => p.applicationId === g.applicationId) ?? null;
  }

  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const list = games.filter((g) => {
      if (source !== "all" && g.source !== source) return false;
      if (onlyWithProfile && !profileFor(g)) return false;
      return !q || g.name.toLowerCase().includes(q);
    });
    const byName = (a: Game, b: Game) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    switch (sort) {
      case "name":
        return list.sort(byName);
      case "playtime":
        return list.sort((a, b) => b.playtimeMinutes - a.playtimeMinutes || byName(a, b));
      default:
        return list.sort((a, b) => b.lastPlayed - a.lastPlayed || byName(a, b));
    }
  });

  const selectedGame = $derived(games.find((g) => g.id === selected) ?? null);
  const manual = $derived(configStore.manualGames);

  async function load(refresh = false) {
    loading = true;
    error = null;
    try {
      games = await api.getGames(refresh);
      if (refresh) ui.toast(`Found ${games.length} game${games.length === 1 ? "" : "s"}.`, "success", 2500);
    } catch (e) {
      error = api.errorMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (api.isTauri) {
      import("@tauri-apps/api/core").then(({ convertFileSrc }) => (toSrc = (p) => convertFileSrc(p)));
    }
    load();
  });

  function cover(g: Game): string | null {
    if (g.cover) return toSrc(g.cover);
    return g.coverUrl;
  }

  async function launch(g: Game) {
    if (launching) return;
    launching = g.id;
    try {
      await api.launchGame(g.id);
      ui.toast(`Starting ${g.name}…`, "success", 3000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 6000);
    } finally {
      launching = null;
    }
  }

  /** Jumps to the game's profile, creating (and binding) one if needed. */
  async function openProfile(g: Game) {
    try {
      let profile = profileFor(g);
      if (!profile) {
        // Bound games get G HUB's "Game: Default" name from the backend; games
        // outside Logitech's database are named here and can be bound later.
        await configStore.createProfile(g.applicationId ? "Default" : `${g.name}: Default`, "game");
        const created = configStore.profiles.find((p) => p.id === configStore.activeProfileId);
        if (created && g.applicationId) await configStore.bindApplication(created.id, g.applicationId);
        profile = created ?? null;
      } else {
        await configStore.selectProfile(profile.id);
      }
      await goto("/profiles");
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function removeManual(m: ManualGame) {
    try {
      await configStore.apply(await api.removeManualGame(m.id));
      await load(true);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  // -- add a game by hand ----------------------------------------------------
  let newName = $state("");
  let newExec = $state("");
  let newArgs = $state("");
  let newCover = $state<string | null>(null);
  let saving = $state(false);

  async function pickFile(kind: "exec" | "cover") {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        title: kind === "exec" ? "Choose the game's executable" : "Choose a cover image",
        multiple: false,
        directory: false,
        filters: kind === "cover" ? [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] : undefined,
      });
      if (typeof picked !== "string") return;
      if (kind === "exec") {
        newExec = picked;
        if (!newName.trim()) newName = picked.split("/").pop()?.replace(/\.[^.]+$/, "") ?? "";
      } else {
        newCover = picked;
      }
    } catch {
      ui.toast("File dialogs need the desktop app.", "error");
    }
  }

  async function addGame() {
    if (saving) return;
    saving = true;
    try {
      await configStore.apply(await api.addManualGame(newName, newExec, newArgs, newCover));
      adding = false;
      newName = newExec = newArgs = "";
      newCover = null;
      await load(true);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 6000);
    } finally {
      saving = false;
    }
  }

  function support() {
    import("@tauri-apps/plugin-opener")
      .then(({ openUrl }) => openUrl("https://github.com/Slyvan25/OpenGHub"))
      .catch(() => window.open("https://github.com/Slyvan25/OpenGHub", "_blank"));
  }

  function relative(secs: number): string {
    if (!secs) return "Never played";
    const d = (Date.now() / 1000 - secs) / 86400;
    if (d < 1) return "Played today";
    if (d < 2) return "Played yesterday";
    if (d < 30) return `Played ${Math.floor(d)} days ago`;
    if (d < 365) return `Played ${Math.floor(d / 30)} months ago`;
    return `Played ${Math.floor(d / 365)} years ago`;
  }

  function playtime(mins: number): string {
    if (!mins) return "";
    if (mins < 60) return `${mins} min`;
    return `${(mins / 60).toFixed(mins >= 600 ? 0 : 1)} h`;
  }

  function clearSelection(event: MouseEvent) {
    if (!(event.target as HTMLElement).closest(".tile, .row, .modal")) selected = null;
  }
</script>

<svelte:window onclick={clearSelection} />

<div class="page">
  <!-- far-left icon rail -->
  <aside class="mini">
    <button class="mini-btn" class:active={view === "library"} onclick={() => (view = "library")}>
      <Icon name="library" size={22} strokeWidth={1.6} />
      <span>Library</span>
    </button>
    <button class="mini-btn" class:active={view === "manage"} onclick={() => (view = "manage")}>
      <Icon name="manage" size={22} strokeWidth={1.6} />
      <span>Manage</span>
    </button>
    <button class="mini-btn bottom" onclick={support}>
      <Icon name="support" size={22} strokeWidth={1.6} />
      <span>Support</span>
    </button>
  </aside>

  {#if view === "library"}
    <aside class="rail">
      <section class="group">
        <button class="group-head" onclick={() => (sourcesOpen = !sourcesOpen)} aria-expanded={sourcesOpen}>
          <span>Games</span>
          <Icon name={sourcesOpen ? "chevronDown" : "chevronRight"} size={16} />
        </button>
        {#if sourcesOpen}
          <div class="group-body">
            {#each SOURCES.filter((s) => s.id === "all" || s.id === "manual" || counts[s.id] > 0) as s (s.id)}
              <button class="source" class:active={source === s.id} onclick={() => (source = s.id)}>
                <Icon name={s.icon} size={18} strokeWidth={1.6} />
                <span class="source-label">{s.label}</span>
                <span class="count">{counts[s.id]}</span>
              </button>
            {/each}
          </div>
        {/if}
      </section>

      <section class="group">
        <button class="group-head" onclick={() => (featuresOpen = !featuresOpen)} aria-expanded={featuresOpen}>
          <span>Features</span>
          <Icon name={featuresOpen ? "chevronDown" : "chevronRight"} size={16} />
        </button>
        {#if featuresOpen}
          <div class="group-body">
            <label class="check">
              <input type="checkbox" bind:checked={onlyWithProfile} />
              <span>Has an OpenGHub profile</span>
            </label>
          </div>
        {/if}
      </section>
    </aside>

    <section class="main">
      <header class="toolbar">
        <label class="search">
          <Icon name="search" size={18} />
          <input type="search" placeholder="Search" bind:value={search} />
        </label>

        <div class="tools">
          <div class="select">
            <select bind:value={sort} aria-label="Sort">
              <option value="recent">Recently played</option>
              <option value="name">Name</option>
              <option value="playtime">Most played</option>
            </select>
            <Icon name="chevronDown" size={15} />
          </div>
          <button class="tool" title="Add a game" aria-label="Add a game" onclick={() => (adding = true)}>
            <Icon name="plus" size={18} />
          </button>
          <button class="tool" title="Rescan launchers" aria-label="Rescan launchers" onclick={() => load(true)} disabled={loading}>
            <Icon name="refresh" size={18} />
          </button>
          <div class="pill" role="group" aria-label="Layout">
            <button class:active={layout === "grid"} aria-label="Grid" onclick={() => (layout = "grid")}>
              <Icon name="grid" size={17} />
            </button>
            <button class:active={layout === "list"} aria-label="List" onclick={() => (layout = "list")}>
              <Icon name="list" size={17} />
            </button>
          </div>
        </div>
      </header>

      {#if loading && games.length === 0}
        <p class="status">Scanning launchers…</p>
      {:else if error}
        <p class="status error">{error}</p>
      {:else if shown.length === 0}
        <div class="empty">
          <p>{games.length === 0 ? "No games found." : "Nothing matches."}</p>
          {#if games.length === 0}
            <p class="hint">Steam, Heroic (Epic / GOG) and Lutris libraries are scanned automatically. Add anything else with +.</p>
          {/if}
        </div>
      {:else if layout === "grid"}
        <div class="grid">
          {#each shown as g (g.id)}
            {@const src = cover(g)}
            {@const bound = profileFor(g)}
            <div
              class="tile"
              class:selected={selected === g.id}
              role="button"
              tabindex="0"
              title={g.name}
              onclick={() => (selected = g.id)}
              ondblclick={() => launch(g)}
              onkeydown={(e) => {
                if (e.key === "Enter") launch(g);
                if (e.key === " ") {
                  e.preventDefault();
                  selected = g.id;
                }
              }}
            >
              {#if src}
                <img class="cover" src={src} alt="" loading="lazy" draggable="false" />
              {:else}
                <div class="cover placeholder"><span>{g.name}</span></div>
              {/if}
              {#if bound}
                <span class="badge" title="Has a profile"><Icon name="profile" size={13} /></span>
              {/if}
              <span class="src-mark" title={SOURCE_LABEL[g.source]}><Icon name={g.source} size={13} strokeWidth={1.8} /></span>
              <div class="caption">{g.name}</div>
              <div class="actions">
                <button class="play" onclick={(e) => { e.stopPropagation(); launch(g); }} disabled={launching === g.id}>
                  <Icon name="play" size={14} strokeWidth={2} />
                  Play
                </button>
                <button class="ghost" onclick={(e) => { e.stopPropagation(); openProfile(g); }}>
                  {bound ? "Profile" : "Add profile"}
                </button>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="list">
          {#each shown as g (g.id)}
            {@const src = cover(g)}
            {@const bound = profileFor(g)}
            <div
              class="row"
              class:selected={selected === g.id}
              role="button"
              tabindex="0"
              onclick={() => (selected = g.id)}
              ondblclick={() => launch(g)}
              onkeydown={(e) => e.key === "Enter" && launch(g)}
            >
              {#if src}
                <img class="row-cover" src={src} alt="" loading="lazy" draggable="false" />
              {:else}
                <div class="row-cover placeholder"></div>
              {/if}
              <div class="row-text">
                <span class="row-name">{g.name}</span>
                <span class="row-meta">
                  <Icon name={g.source} size={13} strokeWidth={1.8} />
                  {SOURCE_LABEL[g.source]}
                  <span class="dot"></span>
                  {relative(g.lastPlayed)}
                  {#if g.playtimeMinutes}<span class="dot"></span>{playtime(g.playtimeMinutes)}{/if}
                </span>
              </div>
              <span class="row-profile">{bound ? bound.name.split(":")[1]?.trim() || bound.name : ""}</span>
              <button class="ghost" onclick={(e) => { e.stopPropagation(); openProfile(g); }}>
                {bound ? "Profile" : "Add profile"}
              </button>
              <button class="play" onclick={(e) => { e.stopPropagation(); launch(g); }} disabled={launching === g.id}>
                <Icon name="play" size={14} strokeWidth={2} />
                Play
              </button>
            </div>
          {/each}
        </div>
      {/if}

      {#if selectedGame}
        <footer class="detail">
          <div>
            <strong>{selectedGame.name}</strong>
            <span class="detail-meta">
              {SOURCE_LABEL[selectedGame.source]} · {relative(selectedGame.lastPlayed)}
              {#if selectedGame.playtimeMinutes}· {playtime(selectedGame.playtimeMinutes)} played{/if}
              {#if selectedGame.installDir}· {selectedGame.installDir}{/if}
            </span>
          </div>
        </footer>
      {/if}
    </section>
  {:else}
    <section class="main manage">
      <header class="head">
        <div>
          <h1>Manage games</h1>
          <p>
            Launchers are scanned read-only. Games added here are started directly from their
            executable; give them a cover so they look the part in the library.
          </p>
        </div>
        <button class="add" onclick={() => (adding = true)}><Icon name="plus" size={17} strokeWidth={2} /> Add game</button>
      </header>

      <div class="sources-table">
        {#each SOURCES.filter((s) => s.id !== "all") as s (s.id)}
          <div class="source-row">
            <Icon name={s.icon} size={20} strokeWidth={1.6} />
            <span class="source-label">{s.label}</span>
            <span class="count">{counts[s.id]} installed</span>
          </div>
        {/each}
      </div>

      <h2 class="section-title">MANUALLY INSTALLED</h2>
      {#if manual.length === 0}
        <p class="status">Nothing added yet.</p>
      {:else}
        <div class="list">
          {#each manual as m (m.id)}
            <div class="row static">
              {#if m.cover}
                <img class="row-cover" src={toSrc(m.cover)} alt="" draggable="false" />
              {:else}
                <div class="row-cover placeholder"></div>
              {/if}
              <div class="row-text">
                <span class="row-name">{m.name}</span>
                <span class="row-meta mono">{m.exec}{m.args ? ` ${m.args}` : ""}</span>
              </div>
              <button class="ghost danger" onclick={() => removeManual(m)}>
                <Icon name="trash" size={15} /> Remove
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>

{#if adding}
  <div class="scrim" role="presentation" onclick={(e) => e.target === e.currentTarget && (adding = false)}>
    <form class="modal" onsubmit={(e) => { e.preventDefault(); addGame(); }}>
      <h2>Add a game</h2>
      <label class="field">
        <span>Name</span>
        <input type="text" bind:value={newName} placeholder="Tempest Rising" required />
      </label>
      <label class="field">
        <span>Executable</span>
        <div class="with-btn">
          <input type="text" bind:value={newExec} placeholder="/path/to/game" required />
          <button type="button" class="tool" onclick={() => pickFile("exec")} aria-label="Browse"><Icon name="folder" size={17} /></button>
        </div>
      </label>
      <label class="field">
        <span>Arguments <em>(optional)</em></span>
        <input type="text" bind:value={newArgs} placeholder="--fullscreen" />
      </label>
      <label class="field">
        <span>Cover image <em>(optional)</em></span>
        <div class="with-btn">
          <input type="text" bind:value={newCover} placeholder="/path/to/cover.jpg" />
          <button type="button" class="tool" onclick={() => pickFile("cover")} aria-label="Browse"><Icon name="image" size={17} /></button>
        </div>
      </label>
      <div class="modal-actions">
        <button type="button" class="ghost" onclick={() => (adding = false)}>Cancel</button>
        <button type="submit" class="add" disabled={saving || !newName.trim() || !newExec.trim()}>Add</button>
      </div>
    </form>
  </div>
{/if}

<style>
  .page {
    display: grid;
    grid-template-columns: 84px 262px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }

  .page:has(.manage) {
    grid-template-columns: 84px minmax(0, 1fr);
  }

  /* ---- icon rail ------------------------------------------------------- */
  .mini {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 22px 0 20px;
    background: var(--surface);
  }

  .mini-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 64px;
    padding: 10px 0;
    border-radius: var(--radius);
    font-family: var(--font);
    font-size: 11px;
    color: var(--text-dim);
  }

  .mini-btn:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.04);
  }

  .mini-btn.active {
    color: var(--accent);
    background: rgba(17, 150, 255, 0.14);
  }

  .mini-btn.bottom {
    margin-top: auto;
  }

  /* ---- filter column --------------------------------------------------- */
  .rail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 26px 16px;
    overflow-y: auto;
  }

  .group {
    border-radius: var(--radius);
    background: var(--surface);
  }

  .group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 42px;
    padding: 0 14px;
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
  }

  .group-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 6px 8px;
  }

  .source {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 38px;
    padding: 0 10px;
    border-radius: 6px;
    font-family: var(--font);
    font-size: 13.5px;
    font-weight: 700;
    color: var(--text);
    text-align: left;
  }

  .source:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .source.active {
    color: var(--accent);
    background: rgba(17, 150, 255, 0.14);
    outline: 1px solid rgba(17, 150, 255, 0.4);
  }

  .source-label {
    flex: 1;
  }

  .count {
    min-width: 24px;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
    font-size: 11px;
    font-weight: 700;
    color: var(--text);
    text-align: center;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    font-size: 13px;
    cursor: pointer;
  }

  .check input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
  }

  /* ---- main ------------------------------------------------------------ */
  .main {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 26px 26px 20px 20px;
    overflow-y: auto;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 300px;
    height: 40px;
    padding: 0 14px;
    color: var(--text-dim);
  }

  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    font-size: 14px;
    letter-spacing: 0.02em;
    user-select: text;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .select {
    position: relative;
    display: flex;
    align-items: center;
    color: var(--text-dim);
  }

  .select select {
    appearance: none;
    height: 36px;
    padding: 0 32px 0 12px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    font-family: var(--font);
    font-size: 13.5px;
    font-weight: 700;
    color: var(--text);
    cursor: pointer;
  }

  .select :global(svg) {
    position: absolute;
    right: 10px;
    pointer-events: none;
  }

  .select option {
    background: #161616;
  }

  .tool {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--surface);
    color: var(--text);
  }

  .tool:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
  }

  .tool:disabled {
    opacity: 0.5;
  }

  .pill {
    display: flex;
    padding: 3px;
    border-radius: 999px;
    background: var(--surface);
  }

  .pill button {
    display: grid;
    place-items: center;
    width: 32px;
    height: 30px;
    border-radius: 999px;
    color: var(--text-dim);
  }

  .pill button.active {
    background: rgba(255, 255, 255, 0.12);
    color: var(--text);
  }

  .status {
    padding: 40px 0;
    color: var(--text-dim);
    text-align: center;
  }

  .status.error {
    color: var(--danger, #ff5c5c);
  }

  .empty {
    padding: 60px 0;
    text-align: center;
    color: var(--text-dim);
  }

  .hint {
    max-width: 48ch;
    margin: 8px auto 0;
    font-size: 13px;
  }

  /* ---- grid tiles ------------------------------------------------------ */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(146px, 1fr));
    gap: 14px;
  }

  .tile {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 8px;
    border: 2px solid transparent;
    background: var(--surface);
    overflow: hidden;
    cursor: pointer;
    transition: border-color 120ms, transform 120ms;
  }

  .tile:hover {
    transform: translateY(-2px);
  }

  .tile.selected {
    border-color: var(--accent);
  }

  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    display: grid;
    place-items: center;
    padding: 14px;
    background: linear-gradient(160deg, #2a2b30, #121316);
    font-size: 15px;
    font-weight: 700;
    text-align: center;
    color: var(--text);
  }

  .badge,
  .src-mark {
    position: absolute;
    top: 8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
  }

  .badge {
    left: 8px;
    color: var(--accent);
  }

  .src-mark {
    right: 8px;
  }

  .caption {
    position: absolute;
    inset: auto 0 0;
    padding: 28px 10px 10px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.9));
    font-size: 13px;
    font-weight: 700;
    color: #fff;
    opacity: 0;
    transition: opacity 120ms;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tile.selected .caption,
  .tile:hover .caption {
    opacity: 1;
  }

  .actions {
    position: absolute;
    inset: auto 8px 40px;
    display: flex;
    gap: 6px;
    opacity: 0;
    transition: opacity 120ms;
  }

  .tile.selected .actions {
    opacity: 1;
  }

  .play {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 6px;
    background: var(--accent);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
  }

  .play:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .play:disabled {
    opacity: 0.6;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.14);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    color: #fff;
    white-space: nowrap;
  }

  .ghost:hover {
    background: rgba(255, 255, 255, 0.24);
  }

  .ghost.danger:hover {
    background: rgba(255, 92, 92, 0.3);
  }

  /* ---- list ------------------------------------------------------------ */
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 12px 8px 8px;
    border: 2px solid transparent;
    border-radius: 8px;
    background: var(--surface);
    cursor: pointer;
  }

  .row.static {
    cursor: default;
  }

  .row.selected {
    border-color: var(--accent);
  }

  .row-cover {
    flex: none;
    width: 40px;
    height: 56px;
    border-radius: 4px;
    object-fit: cover;
  }

  .row-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .row-name {
    font-size: 14px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .row-meta.mono {
    font-family: ui-monospace, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: currentColor;
  }

  .row-profile {
    font-size: 12px;
    color: var(--text-dim);
  }

  .detail {
    position: sticky;
    bottom: 0;
    margin-top: auto;
    padding: 12px 16px;
    border-radius: var(--radius);
    background: rgba(33, 34, 37, 0.96);
    box-shadow: 0 -6px 20px rgba(0, 0, 0, 0.4);
  }

  .detail strong {
    display: block;
    font-size: 14px;
  }

  .detail-meta {
    font-size: 12px;
    color: var(--text-dim);
  }

  /* ---- manage view ----------------------------------------------------- */
  .manage {
    padding: 30px 40px 40px 36px;
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

  .head p {
    max-width: 70ch;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .add {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 42px;
    padding: 0 20px;
    border-radius: var(--radius);
    background: var(--accent);
    font-family: var(--font);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #fff;
  }

  .add:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .add:disabled {
    opacity: 0.5;
  }

  .sources-table {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
    margin-bottom: 30px;
  }

  .source-row {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 52px;
    padding: 0 16px;
    border-radius: var(--radius);
    background: var(--surface);
    font-size: 13.5px;
    font-weight: 700;
  }

  .source-row .count {
    font-weight: 400;
    background: none;
    color: var(--text-dim);
  }

  .section-title {
    margin-bottom: 12px;
  }

  /* ---- add-game modal -------------------------------------------------- */
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.6);
  }

  .modal {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 440px;
    padding: 24px;
    border-radius: 12px;
    background: var(--surface);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  }

  .modal h2 {
    font-size: 18px;
    font-weight: 700;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: var(--text-dim);
  }

  .field em {
    font-style: normal;
    font-weight: 400;
  }

  .field input {
    width: 100%;
    height: 38px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: #000;
    font-size: 13.5px;
    color: var(--text);
    user-select: text;
  }

  .field input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .with-btn {
    display: flex;
    gap: 8px;
  }

  .with-btn input {
    flex: 1;
    min-width: 0;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
