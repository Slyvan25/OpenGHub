<script lang="ts" module>
  /**
   * The icon set, as raw SVG inner markup on a 24×24 grid.
   *
   * Everything is stroke-based so a single `currentColor` + `stroke-width` pair
   * keeps the whole interface visually consistent, the way G HUB's line icons do.
   */
  export const icons = {
    // -- chrome -------------------------------------------------------------
    menu: '<path d="M3 6h18"/><path d="M3 12h12"/><path d="M3 18h7"/>',
    close: '<path d="M5 5l14 14"/><path d="M19 5L5 19"/>',
    minimize: '<path d="M5 12h14"/>',
    maximize: '<rect x="5" y="5" width="14" height="14" rx="1.5"/>',
    restore:
      '<rect x="8" y="3.5" width="12.5" height="12.5" rx="1.5"/><path d="M16 19.5a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5V10A1.5 1.5 0 0 1 5 8.5"/>',
    chevronDown: '<path d="M6 9l6 6 6-6"/>',
    chevronUp: '<path d="M6 15l6-6 6 6"/>',
    chevronRight: '<path d="M9 6l6 6-6 6"/>',
    chevronLeft: '<path d="M15 6l-6 6 6 6"/>',
    arrowLeft: '<path d="M19 12H5"/><path d="M11 6l-6 6 6 6"/>',
    grid:
      '<rect x="3.5" y="3.5" width="7" height="7" rx="1.5"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.5"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.5"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.5"/>',
    list:
      '<rect x="3.5" y="4.5" width="17" height="6" rx="1.5"/><rect x="3.5" y="13.5" width="17" height="6" rx="1.5"/>',
    search: '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
    refresh:
      '<path d="M20 11a8 8 0 1 0-.6 4"/><path d="M20 4.5V11h-6.2"/>',
    plus: '<path d="M12 5v14"/><path d="M5 12h14"/>',
    trash:
      '<path d="M4 7h16"/><path d="M9.5 7V5.2A1.2 1.2 0 0 1 10.7 4h2.6a1.2 1.2 0 0 1 1.2 1.2V7"/><path d="M6.5 7l.9 12a1.5 1.5 0 0 0 1.5 1.4h6.2a1.5 1.5 0 0 0 1.5-1.4L17.5 7"/>',
    pencil:
      '<path d="M4 20h4.2L19.4 8.8a2 2 0 0 0 0-2.8l-1.4-1.4a2 2 0 0 0-2.8 0L4 15.8z"/><path d="M14.5 6.5l3 3"/>',
    check: '<path d="M4.5 12.5l5 5 10-11"/>',
    gear:
      '<circle cx="12" cy="12" r="3.2"/><path d="M12 2.8l1.1 2.4 2.6-.6 1 2.5 2.5 1-.6 2.6 2.4 1.1-2.4 1.1.6 2.6-2.5 1-1 2.5-2.6-.6L12 21.2l-1.1-2.4-2.6.6-1-2.5-2.5-1 .6-2.6L2.8 12l2.4-1.1-.6-2.6 2.5-1 1-2.5 2.6.6z"/>',
    info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><path d="M12 7.8v.6"/>',
    alert:
      '<path d="M12 3.5l9 15.5H3z"/><path d="M12 9.5v4.2"/><path d="M12 16.6v.5"/>',
    home: '<path d="M4 10.5L12 4l8 6.5V19a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 19z"/>',
    profile:
      '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M3.5 9.5h17"/><path d="M8 4.5v5"/>',
    user: '<circle cx="12" cy="8.5" r="3.8"/><path d="M4.5 20a7.5 7.5 0 0 1 15 0"/>',
    lock:
      '<rect x="4.5" y="10.5" width="15" height="10" rx="2"/><path d="M8 10.5V7.8a4 4 0 0 1 8 0v2.7"/>',
    power: '<path d="M12 3.5v8"/><path d="M7 6.6a7.5 7.5 0 1 0 10 0"/>',

    // -- device categories --------------------------------------------------
    mouse:
      '<rect x="7" y="2.8" width="10" height="18.4" rx="5"/><path d="M12 6.5v4"/>',
    keyboard:
      '<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 9.5h.01M9 9.5h.01M12 9.5h.01M15 9.5h.01M18 9.5h.01M6 12.5h.01M9 12.5h.01M12 12.5h.01M15 12.5h.01M18 12.5h.01"/><path d="M8 15.5h8"/>',
    headset:
      '<path d="M4.5 14.5v-2.3a7.5 7.5 0 0 1 15 0v2.3"/><rect x="2.8" y="13.5" width="4" height="6.5" rx="1.6"/><rect x="17.2" y="13.5" width="4" height="6.5" rx="1.6"/>',
    microphone:
      '<rect x="9" y="2.8" width="6" height="11" rx="3"/><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0"/><path d="M12 18v3"/>',
    light:
      '<rect x="3" y="4" width="18" height="4" rx="1.6"/><path d="M12 8v10"/><path d="M7.5 20h9"/>',
    speaker:
      '<rect x="6" y="2.5" width="12" height="19" rx="2"/><circle cx="12" cy="15" r="3.2"/><circle cx="12" cy="7" r="1.4"/>',
    webcam: '<circle cx="12" cy="10.5" r="6.5"/><circle cx="12" cy="10.5" r="2.4"/><path d="M6 20.5h12"/>',
    wheel: '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="2.6"/><path d="M12 3.5v6M5 16.5l4.8-2.8M19 16.5l-4.8-2.8"/>',
    receiver: '<rect x="3.5" y="9" width="13" height="6" rx="1.5"/><path d="M16.5 12h4"/><path d="M19 8.5a5 5 0 0 1 0 7"/>',
    device: '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M9 9h6v6H9z"/>',

    // -- status -------------------------------------------------------------
    battery:
      '<rect x="2.5" y="7.5" width="16" height="9" rx="2"/><path d="M21 11v2"/>',
    batteryCharging:
      '<path d="M13.5 7.5H4.5a2 2 0 0 0-2 2v5a2 2 0 0 0 2 2h4"/><path d="M21 11v2"/><path d="M13 6.5L9.5 12.8h3.6L12.2 18l4.3-6.6h-3.6z"/>',
    chip:
      '<rect x="7" y="7" width="10" height="10" rx="1.6"/><path d="M10 4v3M14 4v3M10 17v3M14 17v3M4 10h3M4 14h3M17 10h3M17 14h3"/>',
    eye: '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>',
    volume:
      '<path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4z"/><path d="M15.8 9.2a4 4 0 0 1 0 5.6"/><path d="M18.4 6.6a7.6 7.6 0 0 1 0 10.8"/>',
    usb: '<path d="M12 21V6"/><path d="M12 6l-2.4 3h4.8z"/><circle cx="12" cy="21" r="1"/><path d="M8 13.5L12 16l4-3.5"/><rect x="14.5" y="10.5" width="3" height="3" rx="0.6"/><circle cx="8" cy="13.5" r="1.4"/>',
    wireless:
      '<path d="M4.6 9.2a10.5 10.5 0 0 1 14.8 0"/><path d="M7.6 12.4a6.3 6.3 0 0 1 8.8 0"/><path d="M10.4 15.5a2.3 2.3 0 0 1 3.2 0"/><circle cx="12" cy="18.6" r="0.9"/>',
    bluetooth: '<path d="M7.5 7.5L16 16.5 12 20V4l4 3.5L7.5 16.5"/>',

    // -- device page tabs ---------------------------------------------------
    // G HUB's sensitivity glyph: a four-way move arrow around a centre dot.
    dpi: '<path d="M12 3v18M3 12h18"/><path d="M9.5 5.5L12 3l2.5 2.5M9.5 18.5L12 21l2.5-2.5M5.5 9.5L3 12l2.5 2.5M18.5 9.5L21 12l-2.5 2.5"/><circle cx="12" cy="12" r="1.8" fill="currentColor" stroke="none"/>',
    // G HUB's assignments glyph: a rounded square with a plus.
    assignments:
      '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M12 8.5v7M8.5 12h7"/>',
    // G HUB's LIGHTSYNC glyph: a small sun with eight short rays.
    lightsync:
      '<circle cx="12" cy="12" r="3.2"/><path d="M12 4v2.2M12 17.8V20M4 12h2.2M17.8 12H20M6.3 6.3l1.6 1.6M16.1 16.1l1.6 1.6M6.3 17.7l1.6-1.6M16.1 7.9l1.6-1.6"/>',
    lightOff:
      '<path d="M9.2 13.8A5.5 5.5 0 0 1 12 3.5a5.4 5.4 0 0 1 3.6 1.4"/><path d="M15 17h-6v-3.4"/><path d="M10 20h4"/><path d="M3.5 3.5l17 17"/>',
    sun: '<circle cx="12" cy="12" r="4.2"/><path d="M12 2.8v2.4M12 18.8v2.4M4.4 4.4l1.7 1.7M17.9 17.9l1.7 1.7M2.8 12h2.4M18.8 12h2.4M4.4 19.6l1.7-1.7M17.9 6.1l1.7-1.7"/>',
    sliders:
      '<path d="M4 7h9"/><path d="M17 7h3"/><path d="M4 17h3"/><path d="M11 17h9"/><circle cx="15" cy="7" r="2.2"/><circle cx="8" cy="17" r="2.2"/>',
    onboard:
      '<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M8.5 9.5h7"/><path d="M8.5 13h7"/><path d="M8.5 16.5h4"/>',
    // G HUB's onboard-memory-mode badge: a chip with a slash through it.
    onboardOff:
      '<rect x="7" y="7" width="10" height="10" rx="1.6"/><path d="M10 4v3M14 4v3M10 17v3M14 17v3M4 10h3M4 14h3M17 10h3M17 14h3"/><path d="M4.5 4.5l15 15"/>',
    macro: '<path d="M8 5l-5 7 5 7"/><path d="M16 5l5 7-5 7"/><path d="M13.5 4.5l-3 15"/>',
    colorCycle:
      '<path d="M20.5 12a8.5 8.5 0 1 1-2.5-6"/><path d="M18.5 3v3.5H15"/><circle cx="12" cy="12" r="3.2"/>',
    equalizer: '<path d="M6 20V9"/><path d="M12 20V4"/><path d="M18 20v-7"/><path d="M3.5 9h5M9.5 4h5M15.5 13h5"/>',
    image:
      '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="8.5" cy="9.5" r="1.8"/><path d="M4 16.5l4.5-4.2 3.4 3 3-2.6 5.1 4.3"/>',
    keycap:
      '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M8 12h8"/><path d="M13 9l3 3-3 3"/>',

    // -- games library ------------------------------------------------------
    library:
      '<rect x="3.5" y="6.5" width="14" height="14" rx="2"/><path d="M7.5 3.5h11a2 2 0 0 1 2 2v11"/><path d="M8.5 11l5 2.5-5 2.5z"/>',
    manage:
      '<rect x="3.5" y="4.5" width="17" height="7" rx="2"/><rect x="3.5" y="14.5" width="17" height="5" rx="2"/><path d="M7 8h.01"/><path d="M7 17h.01"/>',
    support:
      '<circle cx="12" cy="12" r="8.5"/><circle cx="12" cy="12" r="3.5"/><path d="M6 6l3.5 3.5M14.5 14.5L18 18M18 6l-3.5 3.5M9.5 14.5L6 18"/>',
    play: '<path d="M8 5.5v13l10-6.5z"/>',
    folder:
      '<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/>',
    steam:
      '<circle cx="12" cy="12" r="8.5"/><circle cx="15" cy="9" r="2.2"/><circle cx="9.5" cy="14.5" r="2.2"/><path d="M3.8 13.5l5.5 2.3"/><path d="M11.2 13.7l2.7-3"/>',
    epic:
      '<path d="M6 3.5h12v12.5L12 20.5 6 16z"/><path d="M9.5 8h5M9.5 11.5h4M9.5 15h5"/>',
    gog:
      '<rect x="3.5" y="5.5" width="17" height="13" rx="2"/><rect x="6.5" y="8.5" width="4" height="4" rx="1"/><rect x="13.5" y="8.5" width="4" height="4" rx="1"/><path d="M6.5 15.5h11"/>',
    lutris:
      '<path d="M12 3.5l7.5 4.5v8L12 20.5 4.5 16V8z"/><path d="M12 8v8"/><path d="M8.5 10l7 4M15.5 10l-7 4"/>',
    manual:
      '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M7.5 9l3 3-3 3"/><path d="M12.5 15h4"/>',
    clock: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>',

    // -- macro types --------------------------------------------------------
    arrowRight: '<path d="M4 12h15"/><path d="M13 6l6 6-6 6"/>',
    repeat: '<path d="M18.5 8.5A7.5 7.5 0 1 0 19.5 14"/><path d="M19.5 4v5h-5"/>',
    toggle:
      '<rect x="2.5" y="7" width="19" height="10" rx="5"/><circle cx="16.5" cy="12" r="3" fill="currentColor" stroke="none"/>',
    sequence:
      '<rect x="3" y="6.5" width="11" height="9" rx="1.5"/><rect x="10" y="10.5" width="11" height="9" rx="1.5" fill="var(--bg, #000)"/>',
  } as const;

  export type IconName = keyof typeof icons;
</script>

<script lang="ts">
  interface Props {
    name: IconName;
    size?: number;
    strokeWidth?: number;
    class?: string;
  }
  let { name, size = 18, strokeWidth = 1.7, class: className = "" }: Props = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {@html icons[name]}
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
