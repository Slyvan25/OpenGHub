<script lang="ts">
  /**
   * Searchable list of Logitech's application database, for binding a profile
   * to a game. Posters load from Logitech's public channel at runtime; nothing
   * is bundled.
   */
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import type { Application } from "$lib/types";

  interface Props {
    /** Currently bound application id, if any. */
    value: string | null;
    onselect: (app: Application | null) => void;
    onclose: () => void;
  }

  let { value, onselect, onclose }: Props = $props();

  let apps = $state<Application[]>([]);
  let search = $state("");
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      // Keyed lists must have unique ids; the backend dedupes, but a stale
      // cache from an older build should not be able to break the picker.
      const seen = new Set<string>();
      apps = (await api.getApplications()).filter((a) => !seen.has(a.id) && seen.add(a.id));
    } catch (e) {
      error = api.errorMessage(e);
    } finally {
      loading = false;
    }
  });

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const list = q ? apps.filter((a) => a.name.toLowerCase().includes(q)) : apps;
    return list.slice(0, 60);
  });

  async function refresh() {
    loading = true;
    try {
      await api.refreshApplicationDatabase();
      const seen = new Set<string>();
      apps = (await api.getApplications()).filter((a) => !seen.has(a.id) && seen.add(a.id));
    } catch (e) {
      error = api.errorMessage(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="picker">
  <div class="head">
    <label class="search">
      <Icon name="search" size={14} />
      <input type="text" placeholder="Search games" bind:value={search} spellcheck="false" />
    </label>
    <button class="icon" onclick={refresh} title="Refresh database" aria-label="Refresh">
      <Icon name="refresh" size={14} />
    </button>
    <button class="icon" onclick={onclose} aria-label="Close">
      <Icon name="close" size={14} />
    </button>
  </div>

  {#if error}
    <p class="hint err"><Icon name="alert" size={13} /> {error}</p>
  {:else if loading}
    <p class="hint">Loading application database…</p>
  {:else}
    <p class="hint">{apps.length} games known. Detection uses Steam app ids on Linux.</p>
    <div class="grid">
      <button class="card none" class:active={value === null} onclick={() => onselect(null)}>
        <span class="poster blank"><Icon name="profile" size={22} /></span>
        <span class="name">Desktop</span>
      </button>
      {#each filtered as app (app.id)}
        <button class="card" class:active={value === app.id} onclick={() => onselect(app)}>
          {#if app.posterUrl}
            <img class="poster" src={app.posterUrl} alt="" loading="lazy" />
          {:else}
            <span class="poster blank"><Icon name="macro" size={22} /></span>
          {/if}
          <span class="name">{app.name}</span>
          {#if app.steamAppIds.length}
            <span class="tag">Steam</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }

  .head {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    height: 32px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text-dimmer);
  }

  .search input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 13px;
    user-select: text;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
  }

  .icon:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 8px;
    max-height: 340px;
    overflow-y: auto;
    padding-right: 2px;
  }

  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: var(--bg);
    text-align: left;
  }

  .card:hover {
    border-color: var(--line-strong);
  }

  .card.active {
    border-color: var(--cyan);
  }

  .poster {
    width: 100%;
    aspect-ratio: 3 / 4;
    object-fit: cover;
    border-radius: 3px;
    background: var(--surface-3);
  }

  .poster.blank {
    display: grid;
    place-items: center;
    color: var(--text-dimmer);
  }

  .name {
    font-size: 11.5px;
    line-height: 1.3;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tag {
    position: absolute;
    top: 10px;
    right: 10px;
    padding: 1px 6px;
    border-radius: var(--radius-pill);
    background: rgba(0, 0, 0, 0.7);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-dimmer);
  }

  .hint.err {
    color: var(--danger);
  }
</style>
