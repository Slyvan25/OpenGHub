<script lang="ts">
  /**
   * G HUB's header: the G mark, the Devices / Games / Community / Profiles tabs with a
   * blue underline on the active one, and on the right the profile picker, the
   * settings gear and the account icon.
   */
  import { page } from "$app/state";
  import Icon from "./Icon.svelte";
  import ProfilePicker from "./ProfilePicker.svelte";

  const tabs = [
    { href: "/", label: "Devices" },
    { href: "/games", label: "Games", beta: true },
    { href: "/community", label: "Community" },
    { href: "/profiles", label: "Profiles" },
  ];

  /** Device pages count as the Devices tab. */
  function isActive(href: string): boolean {
    const path = page.url.pathname;
    if (href === "/") return path === "/" || path.startsWith("/device/");
    return path.startsWith(href);
  }
</script>

<header class="topbar" data-tauri-drag-region>
  <a class="mark" href="/" aria-label="OpenGHub home">
    <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
      <!-- The G: a ring with a gap on the right and an inward bar, drawn as a stroke. -->
      <path
        d="M24 9.5A10.5 10.5 0 1 0 26.5 16h-9"
        fill="none"
        stroke="currentColor"
        stroke-width="4.2"
        stroke-linecap="round"
      />
    </svg>
  </a>

  <nav class="tabs" aria-label="Sections">
    {#each tabs as tab (tab.href)}
      <a class="tab" class:active={isActive(tab.href)} href={tab.href}>
        {tab.label}
        {#if "beta" in tab && tab.beta}<span class="beta">Beta</span>{/if}
      </a>
    {/each}
  </nav>

  <div class="right">
    <ProfilePicker />
    <a
      class="icon-btn"
      class:active={page.url.pathname.startsWith("/settings")}
      href="/settings"
      aria-label="Settings"
    >
      <Icon name="gear" size={20} strokeWidth={1.7} />
    </a>
    <!-- <button class="icon-btn" aria-label="Account">
      <Icon name="user" size={20} strokeWidth={1.7} />
    </button> -->
  </div>
</header>

<style>
  .topbar {
    flex: none;
    height: var(--topbar-h);
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 0 26px 0 30px;
    border-bottom: 1px solid var(--line);
  }

  .mark {
    display: grid;
    place-items: center;
    color: var(--text);
    margin-right: 6px;
  }

  .mark:hover {
    color: var(--cyan);
  }

  .tabs {
    display: flex;
    align-items: stretch;
    gap: 6px;
    height: 100%;
  }

  .tab {
    position: relative;
    display: flex;
    align-items: center;
    padding: 0 13px;
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: var(--text);
    transition: color 120ms var(--ease);
  }

  .tab:hover {
    color: var(--cyan);
  }

  /* G HUB's green "Beta" superscript on the Games tab. */
  .beta {
    position: absolute;
    top: 8px;
    right: -6px;
    font-size: 9px;
    font-weight: 700;
    color: #7ed321;
  }

  /* Active tab: G HUB's blue text with a 3px underline flush to the bar. */
  .tab.active {
    color: var(--accent);
  }

  .tab.active::after {
    content: "";
    position: absolute;
    left: 13px;
    right: 13px;
    bottom: 0;
    height: 3px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    color: var(--text);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }

  .icon-btn:hover,
  .icon-btn.active {
    background: var(--surface-2);
  }
</style>
