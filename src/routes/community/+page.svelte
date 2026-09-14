<script lang="ts">
  /**
   * Community — G HUB's landing: a centred search bar and rows of profile
   * cards you scroll sideways, each with VIEW ALL and ‹ › arrows. The profiles
   * come from an open Git repository (settable in Settings); nothing is
   * written to a device from here.
   */
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import CommunityCard from "$lib/components/CommunityCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { loadIndex, loadPosters } from "$lib/community";
  import { artworkIds } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import type { CommunityEntry } from "$lib/types";

  let entries = $state<CommunityEntry[]>([]);
  let posters = $state<Record<string, string>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let search = $state("");
  /** Sections expanded with VIEW ALL. */
  let expanded = $state<Record<string, boolean>>({});
  let rows = $state<Record<string, HTMLDivElement>>({});

  const myIds = $derived(new Set(deviceStore.devices.flatMap((d) => artworkIds(d))));

  const matches = (e: CommunityEntry, q: string) =>
    !q ||
    [e.name, e.author, e.description, e.device.displayName, e.application?.name ?? ""]
      .join(" ")
      .toLowerCase()
      .includes(q);

  interface Section {
    id: string;
    title: string;
    items: CommunityEntry[];
  }

  const sections = $derived.by<Section[]>(() => {
    const q = search.trim().toLowerCase();
    if (q) return [{ id: "results", title: "Search results", items: entries.filter((e) => matches(e, q)) }];

    const mine = entries.filter((e) => e.device.productIds.some((p) => myIds.has(p)));
    const games = entries.filter((e) => e.application);
    const recent = [...entries].sort((a, b) => (b.updated ?? "").localeCompare(a.updated ?? ""));
    const out: Section[] = [];
    if (mine.length) out.push({ id: "mine", title: "For your devices", items: mine });
    if (games.length) out.push({ id: "games", title: "Game profiles", items: games });
    out.push({ id: "recent", title: mine.length || games.length ? "May be of interest" : "All profiles", items: recent });
    return out;
  });

  async function load(refresh = false) {
    loading = true;
    error = null;
    try {
      entries = await loadIndex(refresh);
    } catch (e) {
      error = api.errorMessage(e);
    } finally {
      loading = false;
    }
    posters = await loadPosters();
  }

  onMount(() => load(false));

  function scroll(id: string, dir: -1 | 1) {
    const row = rows[id];
    if (!row) return;
    row.scrollBy({ left: dir * (row.clientWidth - 60), behavior: "smooth" });
  }
</script>

<div class="page">
  <label class="search">
    <Icon name="search" size={17} />
    <input type="text" placeholder="Search" bind:value={search} spellcheck="false" />
  </label>

  {#if error}
    <div class="notice">
      <Icon name="alert" size={16} />
      <div>
        <strong>Could not reach the community repository.</strong>
        {error}
        <span class="dim">Repository: {configStore.settings.communityRepo || "(default)"} — change it in Settings.</span>
      </div>
      <button class="ghost" onclick={() => load(true)}>Retry</button>
    </div>
  {:else if loading}
    <p class="dim center">Loading…</p>
  {:else if entries.length === 0}
    <p class="dim center">The repository has no profiles yet. Share one from Profiles → ⋮ → Share.</p>
  {:else}
    {#each sections as section (section.id)}
      <section class="section">
        <header>
          <h2>{section.title}</h2>
          {#if section.items.length > 0}
            <div class="section-tools">
              <button class="view-all" onclick={() => (expanded = { ...expanded, [section.id]: !expanded[section.id] })}>
                {expanded[section.id] ? "Show less" : "View all"}
              </button>
              {#if !expanded[section.id]}
                <button class="arrow" onclick={() => scroll(section.id, -1)} aria-label="Scroll left">
                  <Icon name="chevronLeft" size={18} />
                </button>
                <button class="arrow" onclick={() => scroll(section.id, 1)} aria-label="Scroll right">
                  <Icon name="chevronRight" size={18} />
                </button>
              {/if}
            </div>
          {/if}
        </header>

        {#if section.items.length === 0}
          <p class="dim">No profiles match “{search}”.</p>
        {:else}
          <div class="row" class:wrap={expanded[section.id]} bind:this={rows[section.id]}>
            {#each section.items as e (e.id)}
              <CommunityCard entry={e} {posters} />
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</div>

<style>
  .page {
    max-width: var(--content-max);
    margin: 0 auto;
    padding: 30px var(--content-pad) 40px;
    width: 100%;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 440px;
    height: 38px;
    margin: 0 auto 26px;
    padding: 0 18px;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    color: var(--text);
  }

  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    font-size: 13px;
    user-select: text;
  }

  .section {
    margin: 0 auto 30px;
    max-width: 1010px;
  }

  .section header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 20px;
  }

  h2 {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.4px;
  }

  .section-tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .view-all {
    margin-right: 10px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text);
  }

  .view-all:hover {
    color: var(--accent);
  }

  .arrow {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: var(--surface);
    color: var(--text);
  }

  .arrow:hover {
    background: var(--surface-3);
  }

  .row {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
    padding-bottom: 4px;
  }

  .row::-webkit-scrollbar {
    display: none;
  }

  .row > :global(*) {
    scroll-snap-align: start;
  }

  .row.wrap {
    flex-wrap: wrap;
    overflow: visible;
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    max-width: 760px;
    margin: 0 auto;
    padding: 14px 16px;
    border-left: 3px solid var(--warning);
    border-radius: var(--radius);
    background: var(--surface);
    font-size: 13px;
    line-height: 1.5;
  }

  .notice strong {
    display: block;
  }

  .ghost {
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text);
  }

  .dim {
    display: block;
    font-size: 12px;
    color: var(--text-dim);
  }

  .center {
    text-align: center;
    padding: 40px 0;
  }
</style>
