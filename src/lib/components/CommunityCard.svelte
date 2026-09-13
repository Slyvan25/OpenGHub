<script lang="ts">
  /**
   * One community tile, G HUB style: image on top, the author's avatar and
   * name, the device it targets, the title, a two-line description and a
   * stats row. Clicking opens the profile's detail page.
   */
  import { goto } from "$app/navigation";
  import { avatarColor, initial, kindOf } from "$lib/community";
  import { kindIcon } from "$lib/device-ui";
  import { artwork } from "$lib/stores/artwork.svelte";
  import type { CommunityEntry } from "$lib/types";
  import DeviceArt from "./DeviceArt.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    entry: CommunityEntry;
    posters: Record<string, string>;
  }
  let { entry, posters }: Props = $props();

  const poster = $derived(entry.application ? posters[entry.application.id] : undefined);
  const deviceImg = $derived(artwork.thumbFor(entry.device.productIds));

  function open() {
    goto(`/community/${encodeURIComponent(entry.id)}`);
  }
</script>

<div
  class="card"
  role="button"
  tabindex="0"
  onclick={open}
  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && open()}
>
  <div class="image" class:device={!poster}>
    {#if poster}
      <img src={poster} alt="" loading="lazy" draggable="false" />
    {:else if deviceImg}
      <img class="device-img" src={deviceImg} alt="" loading="lazy" draggable="false" />
    {:else}
      <div class="device-svg"><DeviceArt kind={kindOf(entry)} /></div>
    {/if}
  </div>

  <div class="body">
    <div class="author">
      <span class="avatar" style="background: {avatarColor(entry.author)}">{initial(entry.author)}</span>
      <span class="author-name">{entry.author}</span>
    </div>
    <div class="device">
      <Icon name={kindIcon(kindOf(entry))} size={11} strokeWidth={1.8} />
      <span>{entry.device.displayName || entry.device.modelId}</span>
    </div>
    <h3>{entry.name}</h3>
    <p>{entry.description || "No description."}</p>
    <div class="stats">
      {#if entry.dpiStages.length}<span title="DPI stages"><Icon name="dpi" size={13} /> {entry.dpiStages.length}</span>{/if}
      {#if entry.hasLighting}<span title="Lighting"><Icon name="lightsync" size={13} /></span>{/if}
      {#if entry.assignmentCount}<span title="Bindings"><Icon name="assignments" size={13} /> {entry.assignmentCount}</span>{/if}
      {#if entry.macroCount}<span title="Macros"><Icon name="macro" size={13} /> {entry.macroCount}</span>{/if}
    </div>
  </div>
</div>

<style>
  .card {
    flex: none;
    display: flex;
    flex-direction: column;
    width: 218px;
    height: 295px;
    border-radius: 8px;
    background: var(--surface);
    overflow: hidden;
    cursor: pointer;
    transition: transform 120ms var(--ease), background 120ms var(--ease);
  }

  .card:hover {
    background: var(--surface-hover);
    transform: translateY(-2px);
  }

  .card:focus-visible {
    outline: 2px solid var(--accent);
  }

  .image {
    position: relative;
    height: 122px;
    background: #111214;
    overflow: hidden;
  }

  .image img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .image.device {
    background: linear-gradient(180deg, #2b2c30, #17181b);
  }

  .device-img {
    object-fit: contain !important;
    padding: 10px;
  }

  .device-svg {
    height: 100%;
    padding: 8px 60px;
  }

  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 12px 14px 12px;
    min-height: 0;
    position: relative;
  }

  .author {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    font-size: 15px;
    font-weight: 700;
    color: #fff;
  }

  .author-name {
    font-size: 12px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 10px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  h3 {
    margin-top: 6px;
    font-size: 15px;
    font-weight: 700;
    letter-spacing: -0.2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  p {
    margin-top: 4px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-dim);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .stats {
    display: flex;
    gap: 14px;
    margin-top: auto;
    font-size: 11px;
    color: var(--text-dim);
  }

  .stats span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
</style>
