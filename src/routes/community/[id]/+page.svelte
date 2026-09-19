<script lang="ts">
  /**
   * A community profile, full page like G HUB's: back arrow, the game it is
   * for, the device tag, a big title, the author, description and tags on the
   * left; the device render with the profile's lighting / assignments / DPI
   * summary on the right; DOWNLOAD at the bottom.
   *
   * Every macro is listed step by step before anything can be imported — a
   * macro is a keystroke sequence and nobody should take one blind.
   */
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import DeviceArt, { type ZoneGlow } from "$lib/components/DeviceArt.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { avatarColor, initial, kindOf, loadIndex } from "$lib/community";
  import { kindIcon } from "$lib/device-ui";
  import { describeStep } from "$lib/macros";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { CommunityEntry, SharedProfile } from "$lib/types";

  let entry = $state<CommunityEntry | null>(null);
  let profile = $state<SharedProfile | null>(null);
  let error = $state<string | null>(null);
  let importing = $state(false);
  let showMacros = $state(true);

  onMount(async () => {
    const id = decodeURIComponent(page.params.id!);
    try {
      const index = await loadIndex(false);
      entry = index.find((e) => e.id === id) ?? null;
      if (!entry) {
        error = "This profile is not in the repository index.";
        return;
      }
      profile = await api.previewCommunityProfile(entry.path);
    } catch (e) {
      error = api.errorMessage(e);
    }
  });

  const dp = $derived(profile?.profile ?? null);
  const zoneNames = $derived(dp?.zoneNames ?? []);

  /** The primary zone's effect, for the summary. */
  const lighting = $derived.by(() => {
    if (!dp) return null;
    const zones = Object.entries(dp.lightingZones ?? {});
    if (zones.length) return zones[0][1];
    return dp.lighting ?? null;
  });

  const glows = $derived<ZoneGlow[]>(
    dp
      ? Object.entries(dp.lightingZones ?? {}).map(([idx, z]) => ({
          index: Number(idx),
          locationName: zoneNames[Number(idx)] ?? `Zone ${Number(idx) + 1}`,
          color: z.effect === "off" ? null : z.color,
          brightness: z.brightness,
        }))
      : [],
  );

  const EFFECT: Record<string, string> = { off: "Off", fixed: "Fixed", breathing: "Breathing", cycle: "Cycle" };

  async function importIt() {
    if (!profile) return;
    importing = true;
    try {
      configStore.apply(await api.importCommunityProfile(profile));
      ui.toast(`Imported “${profile.name}”. Find it under Profiles.`, "success", 4000);
      await goto("/profiles");
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 7000);
    } finally {
      importing = false;
    }
  }

  function tagify(s: string): string {
    return s.replace(/[^a-z0-9]+/gi, "").toUpperCase();
  }
</script>

<div class="page">
  <button class="back" onclick={() => goto("/community")} aria-label="Back to Community">
    <Icon name="arrowLeft" size={24} strokeWidth={1.8} />
  </button>

  {#if error}
    <p class="status">{error}</p>
  {:else if !entry || !profile || !dp}
    <p class="status">Loading…</p>
  {:else}
    <div class="columns">
      <div class="left">
        {#if entry.application}
          <span class="game">{entry.application.name}</span>
        {/if}
        <span class="device-tag">
          <Icon name={kindIcon(kindOf(entry))} size={10} strokeWidth={2} />
          {entry.device.displayName || entry.device.modelId}
        </span>
        <h1>{entry.name}</h1>

        <div class="author">
          <span class="avatar" style="background: {avatarColor(entry.author)}">{initial(entry.author)}</span>
          <span class="author-name">{entry.author}</span>
        </div>

        <p class="desc">{entry.description || "No description."}</p>

        {#if entry.application}
          <div class="game-row">
            <Icon name="library" size={13} strokeWidth={1.8} />
            <span>{entry.application.name}</span>
          </div>
        {/if}

        <div class="tags">
          {#if entry.application}<span>{tagify(entry.application.name)}</span>{/if}
          <span>{(entry.device.displayName || entry.device.modelId).toUpperCase()}</span>
        </div>

        <div class="meta">
          <span title="Licence"><Icon name="check" size={14} /> {profile.license}</span>
          <span title="Last updated"><Icon name="clock" size={14} /> {entry.updated || "—"}</span>
        </div>
      </div>

      <div class="right">
        <div class="render-wrap">
          <span class="render-name">{entry.device.displayName || entry.device.modelId}</span>
          <div class="render">
            <DeviceArt
              kind={kindOf(entry)}
              productIds={entry.device.productIds}
              zoneProductIds={entry.device.productIds}
              zones={glows}
              glow={lighting && lighting.effect !== "off" ? lighting.color : null}
              brightness={lighting?.brightness ?? 100}
              variant="thumb"
            />
          </div>
        </div>

        <dl class="stats">
          {#if lighting}
            <div class="stat">
              <span class="stat-icon"><Icon name="lightsync" size={18} strokeWidth={1.7} /></span>
              <div>
                <div><dt>Effect</dt><dd>{EFFECT[lighting.effect] ?? lighting.effect}</dd></div>
                {#if lighting.effect === "breathing" || lighting.effect === "cycle"}
                  <div><dt>Speed</dt><dd>{lighting.rateMs}ms</dd></div>
                {:else if lighting.effect !== "off"}
                  <div><dt>Colour</dt><dd><span class="swatch" style="background: {lighting.color}"></span>{lighting.color}</dd></div>
                {/if}
              </div>
            </div>
          {/if}
          <div class="stat">
            <span class="stat-icon"><Icon name="assignments" size={18} strokeWidth={1.7} /></span>
            <div>
              <div><dt>Assignments</dt><dd>{dp.assignments.length}</dd></div>
              {#if dp.macros.length}<div><dt>Macros</dt><dd>{dp.macros.length}</dd></div>{/if}
            </div>
          </div>
          {#if dp.wheel}
            <div class="stat">
              <span class="stat-icon"><Icon name="wheel" size={18} strokeWidth={1.7} /></span>
              <div>
                <div><dt>Operating range</dt><dd>{dp.wheel.rangeDeg}°</dd></div>
                <div><dt>Centering spring</dt><dd>{dp.wheel.centerSpring}%</dd></div>
                <div><dt>Sensitivity</dt><dd>{dp.wheel.sensitivity}</dd></div>
                {#if dp.wheel.gamepadMode}<div><dt>Gamepad mode</dt><dd>on — appears as an Xbox controller</dd></div>{/if}
                {#if dp.wheel.pedals?.combined}<div><dt>Pedals</dt><dd>combined</dd></div>{/if}
              </div>
            </div>
          {/if}
          {#if dp.dpiStages.length}
            <div class="stat">
              <span class="stat-icon"><Icon name="dpi" size={18} strokeWidth={1.7} /></span>
              <div>
                <div><dt>Default DPI</dt><dd>{dp.dpiStages[Math.min(dp.activeStage, dp.dpiStages.length - 1)]}</dd></div>
                <div><dt>DPI speeds</dt><dd>{dp.dpiStages.join(" / ")}</dd></div>
                {#if dp.reportRateHz}<div><dt>Report rate</dt><dd>{dp.reportRateHz}</dd></div>{/if}
              </div>
            </div>
          {/if}
        </dl>

        {#if dp.assignments.length || dp.macros.length}
          <section class="details">
            {#if dp.assignments.length}
              <h3>Bindings</h3>
              <ul>
                {#each dp.assignments as a (a.control)}
                  <li><code>{a.control}</code> → {a.label} <span class="dim">({a.category})</span></li>
                {/each}
              </ul>
            {/if}
            {#if dp.macros.length}
              <h3>
                <button class="toggle" onclick={() => (showMacros = !showMacros)}>
                  Macros <span class="dim">— shown in full; these are keystrokes your device will type</span>
                  <Icon name={showMacros ? "chevronUp" : "chevronDown"} size={13} />
                </button>
              </h3>
              {#if showMacros}
                {#each dp.macros as m (m.id)}
                  <div class="macro">
                    <strong>{m.name}</strong> <span class="dim">({m.steps.length} steps)</span>
                    <ol>
                      {#each m.steps as s, i (i)}<li>{describeStep(s)}</li>{/each}
                    </ol>
                  </div>
                {/each}
              {/if}
            {/if}
          </section>
        {/if}
      </div>
    </div>

    <footer>
      <button class="download" onclick={importIt} disabled={importing}>
        {importing ? "Importing…" : "Download"}
      </button>
    </footer>
  {/if}
</div>

<style>
  .page {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 24px 56px 36px;
    overflow-y: auto;
  }

  .back {
    position: absolute;
    top: 18px;
    left: 24px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    color: var(--text);
  }

  .back:hover {
    background: var(--surface);
  }

  .status {
    padding: 80px 0;
    text-align: center;
    color: var(--text-dim);
  }

  .columns {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
    gap: 40px;
    padding-top: 150px;
  }

  /* ---- left ------------------------------------------------------------ */
  .left {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .game {
    margin-bottom: 36px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .device-tag {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  h1 {
    margin-top: 2px;
    font-size: 40px;
    font-weight: 500;
    letter-spacing: -0.5px;
    line-height: 1.1;
    text-transform: uppercase;
  }

  .author {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 28px;
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    font-size: 16px;
    font-weight: 700;
  }

  .author-name {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .desc {
    max-width: 44ch;
    margin-top: 36px;
    font-size: 12px;
    line-height: 1.5;
  }

  .game-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 32px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tags {
    display: flex;
    gap: 14px;
    margin-top: 14px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .meta {
    display: flex;
    gap: 18px;
    margin-top: 24px;
    font-size: 11px;
    color: var(--text-dim);
  }

  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  /* ---- right ----------------------------------------------------------- */
  .right {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 20px 24px;
    align-content: start;
  }

  .render-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .render-name {
    align-self: flex-start;
    font-size: 17px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    color: #4a4b4f;
  }

  .render {
    width: 200px;
    height: 200px;
  }

  .stats {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding-top: 30px;
  }

  .stat {
    display: flex;
    gap: 14px;
  }

  .stat-icon {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    color: var(--text);
  }

  .stat > div {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .stat > div > div {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  dt {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  dd {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
  }

  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }

  .details {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 10px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--surface);
    font-size: 12px;
  }

  .details h3 {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .details ul,
  .details ol {
    padding-left: 18px;
    line-height: 1.6;
  }

  .details code {
    font-family: ui-monospace, monospace;
    font-size: 11px;
  }

  .macro {
    padding: 6px 0;
  }

  .dim {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-dim);
  }

  footer {
    display: flex;
    justify-content: center;
    padding-top: 40px;
  }

  .download {
    width: 174px;
    height: 30px;
    border-radius: 2px;
    background: var(--cyan);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #fff;
  }

  .download:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .download:disabled {
    opacity: 0.6;
  }
</style>
