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
    <!-- The OpenGHub logo (static/logo.svg): disc in the text colour, glyph cut out. -->
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 132.49316 132.49316" width="30" height="30" aria-hidden="true">
      <g transform="translate(-39.986188,-66.472211)">
        <circle cx="106.23277" cy="132.7188" r="57.700085" fill="currentColor" stroke="currentColor" stroke-width="17.093"/>
        <path d="m 110.97539,190.55127 c 0,-39.42404 27.47736,-71.68008 66.0054,-71.68008 38.52804,0 66.00541,32.25604 66.00541,71.68008 0,39.42404 -27.47737,71.68008 -66.00541,71.68008 -38.52804,0 -66.0054,-32.25604 -66.0054,-71.68008 z m 172.33085,0 c 0,-62.12273 -46.29338,-107.520118 -106.32545,-107.520118 -60.03206,0 -106.325448,45.397388 -106.325448,107.520118 0,62.12273 46.293388,107.52012 106.325448,107.52012 60.03207,0 106.32545,-45.39739 106.32545,-107.52012 z" transform="matrix(0.3114053,0,0,0.3114053,50.284534,73.978262)" fill="var(--bg)" stroke="var(--bg)" stroke-width="50.2677"/>
        <rect x="102.20329" y="136.95477" width="7.9145093" height="49.537155" fill="currentColor" stroke="currentColor" stroke-width="9.8888"/>
        <rect x="101.92165" y="167.54222" width="36.668324" height="9.5465488" fill="currentColor" stroke="currentColor" stroke-width="9.344"/>
        <circle cx="105.94232" cy="133.34341" r="2.2346151" fill="var(--bg)" stroke="var(--bg)" stroke-width="5.56782"/>
      </g>
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
