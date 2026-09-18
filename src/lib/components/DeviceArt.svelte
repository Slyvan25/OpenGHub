<script lang="ts" module>
  /** Unique gradient ids per instance — duplicate ids would cross-link fills. */
  let seq = 0;
</script>

<script lang="ts">
  import { artwork } from "$lib/stores/artwork.svelte";
  import { spotFor, type ZoneSpot } from "$lib/zones";
  import type { DeviceKind } from "$lib/types";

  /** One lit zone, positioned on the art by its reported location. */
  export interface ZoneGlow {
    /** Zone index, used to key any user-dragged position. */
    index: number;
    locationName: string;
    /** `#rrggbb`, or `null` when the zone is off. */
    color: string | null;
    /** 0–100. */
    brightness: number;
  }

  interface Props {
    kind: DeviceKind;
    /** Single-colour glow; the simple case used by the dashboard cards. */
    glow?: string | null;
    /** 0–100, scales the glow opacity. */
    brightness?: number;
    /**
     * Per-zone glows. Takes precedence over `glow`, and is what makes a photo
     * show the same zones lit as the real device.
     */
    zones?: ZoneGlow[];
    /** Product ids, so per-model zone positions can be looked up. */
    zoneProductIds?: number[];
    /** User-dragged glow positions, keyed by zone index. */
    zoneOverrides?: Record<string, ZoneSpot>;
    /**
     * Turns on drag handles so the user can place each glow on their own
     * artwork — the only way to be right, since the device reports no position.
     */
    editZones?: boolean;
    onzonemove?: (index: number, spot: ZoneSpot) => void;
    /**
     * Product ids to look for user-supplied artwork under, best first. Falls
     * back to the SVG when there is no file, or when one fails to decode.
     */
    productIds?: number[];
    /**
     * `thumb` for dashboard cards (G HUB ships a separate small render);
     * `shifter` / `pedals` are a wheel's accessories.
     */
    variant?: "front" | "thumb" | "side" | "shifter" | "pedals";
    class?: string;
  }

  let {
    kind,
    glow = null,
    brightness = 100,
    zones = [],
    zoneProductIds = [],
    zoneOverrides = {},
    editZones = false,
    onzonemove,
    productIds = [],
    variant = "front",
    class: className = "",
  }: Props = $props();

  let imageFailed = $state(false);
  const photo = $derived.by(() => {
    if (productIds.length === 0 || imageFailed) return null;
    if (variant === "thumb") return artwork.thumbFor(productIds);
    if (variant === "side") return artwork.sideFor(productIds) ?? artwork.forProductIds(productIds);
    if (variant === "shifter") return artwork.shifterFor(productIds);
    if (variant === "pedals") return artwork.pedalsFor(productIds);
    return artwork.forProductIds(productIds);
  });

  /**
   * Zone rectangles from an imported G HUB depot, in the image's own
   * coordinate space. When present these are authoritative: they are what
   * G HUB itself draws, so no positioning or masks are needed.
   */
  const layoutView = $derived.by(() => {
    const layout = artwork.layoutFor(zoneProductIds);
    if (!layout) return null;
    const want = variant === "thumb" ? "front" : variant;
    return layout.views.find((v) => v.view === want) ?? layout.views[0] ?? null;
  });

  const uid = `art${seq++}`;

  const placed = $derived(
    zones.map((z) => ({
      ...z,
      /** Exact rectangle from the depot layout, matched on HID++ location name. */
      rect: layoutView?.zones.find((r) => r.locationName === z.locationName) ?? null,
      /** A user-supplied mask beats any guessed position. */
      mask: artwork.maskFor(zoneProductIds, z.index),
      spot: spotFor(kind, z.locationName, z.index, {
        productIds: zoneProductIds,
        overrides: zoneOverrides,
      }),
    })),
  );

  /** Zones that are actually on. In edit mode every zone stays visible. */
  const litZones = $derived(
    editZones ? placed : placed.filter((z) => z.color && z.brightness > 0),
  );

  let artBox: HTMLDivElement | undefined = $state();
  let dragging = $state<number | null>(null);
  let boxW = $state(1);
  let boxH = $state(1);

  // The photo is `object-fit: contain` inside a 4% padding, so the image
  // occupies a centred sub-rectangle of the box. Layout coordinates are
  // fractions of the *image*, so they are mapped into that sub-rectangle.
  // Note CSS percentage padding is relative to the box *width* on every side.
  const PAD = $derived(className.includes("tight") ? 0 : 0.04);
  const imgAspect = $derived(layoutView ? layoutView.width / layoutView.height : 1);
  const geom = $derived.by(() => {
    const pad = PAD * boxW;
    const innerW = Math.max(1, boxW - 2 * pad);
    const innerH = Math.max(1, boxH - 2 * pad);
    let w: number, h: number;
    if (imgAspect > innerW / innerH) {
      w = innerW;
      h = w / imgAspect;
    } else {
      h = innerH;
      w = h * imgAspect;
    }
    const left = pad + (innerW - w) / 2;
    const top = pad + (innerH - h) / 2;
    const bw = Math.max(1, boxW);
    const bh = Math.max(1, boxH);
    return { w: (w / bw) * 100, h: (h / bh) * 100, left: (left / bw) * 100, top: (top / bh) * 100 };
  });
  const imgW = $derived(geom.w);
  const imgH = $derived(geom.h);
  const imgLeft = $derived(geom.left);
  const imgTop = $derived(geom.top);

  function moveZone(event: PointerEvent, index: number, spot: ZoneSpot) {
    if (!artBox) return;
    const rect = artBox.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const y = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height));
    onzonemove?.(index, { ...spot, x, y });
  }

  // The SVG drawings take a single colour. With per-zone data, use the first
  // lit zone so the drawing still reflects something sensible.
  const effectiveGlow = $derived(zones.length > 0 ? (litZones[0]?.color ?? null) : glow);
  const effectiveBrightness = $derived(
    zones.length > 0 ? (litZones[0]?.brightness ?? 0) : brightness,
  );

  const lit = $derived(!!effectiveGlow && effectiveBrightness > 0);
  const glowColor = $derived(effectiveGlow ?? "#3a3a3a");
  const glowAlpha = $derived(lit ? 0.25 + (effectiveBrightness / 100) * 0.75 : 0);

  // 60 % keyboard: key counts per row, then the modifier-heavy bottom row.
  const keyRows = [14, 14, 13, 12];
  // Bottom row widths in key units (the space bar is 6.25u), laid out proportionally.
  const bottomRow = [1.25, 1.25, 1.25, 6.25, 1.25, 1.25, 1.25, 1.25];
  const bottomUnits = bottomRow.reduce((a, b) => a + b, 0);
  const unit = (430 - 8 - (bottomRow.length + 1) * 3) / bottomUnits;
  /** Left edge of each bottom-row key, in SVG units. */
  const bottomOffsets = bottomRow.map(
    (_, i) => 24 + 3 + bottomRow.slice(0, i).reduce((a, b) => a + b, 0) * unit + i * 3,
  );
</script>

<div class="art {className}">
  {#if photo}
    <!--
      A photo is a fixed image, so the lighting is composited on top: one
      screen-blended blob per lit zone, positioned by the zone's reported
      location. That reads much closer to the real device than tinting the
      whole picture.
    -->
    <div
      class="photo-wrap"
      bind:this={artBox}
      bind:clientWidth={boxW}
      bind:clientHeight={boxH}
    >
      <img src={photo} alt="" class="photo" onerror={() => (imageFailed = true)} />
      {#each litZones as zone (zone.index)}
        {#if zone.rect && layoutView}
          <!--
            Rect route: the zone's own rectangle from G HUB's metadata, placed
            over the letterboxed image. A soft-edged fill reads like the LED
            bleeding through the shell.
          -->
          <div
            class="zone-rect"
            style="
              left: {imgLeft + zone.rect.x * imgW}%;
              top: {imgTop + zone.rect.y * imgH}%;
              width: {zone.rect.width * imgW}%;
              height: {zone.rect.height * imgH}%;
              background: radial-gradient(ellipse at center, {zone.color ?? '#888'} 0%, {zone.color ?? '#888'}aa 45%, transparent 75%);
              opacity: {editZones && !zone.color ? 0.3 : 0.45 + (zone.brightness / 100) * 0.55};
            "
          ></div>
        {:else if zone.mask}
          <!--
            Mask route: the zone's own image defines exactly which pixels light
            up, filled with the live colour. Same approach as G HUB.
          -->
          <div
            class="zone-mask"
            style="
              background: {zone.color ?? '#888'};
              mask-image: url({zone.mask});
              -webkit-mask-image: url({zone.mask});
              opacity: {editZones && !zone.color ? 0.3 : 0.4 + (zone.brightness / 100) * 0.6};
            "
          ></div>
        {:else}
          <div
            class="zone-glow"
            style="
              left: {zone.spot.x * 100}%;
              top: {zone.spot.y * 100}%;
              width: {zone.spot.r * 200}%;
              padding-bottom: {zone.spot.r * 200}%;
              background: radial-gradient(closest-side, {zone.color ?? '#888'} 0%, {zone.color ?? '#888'}80 45%, transparent 72%);
              opacity: {editZones && !zone.color ? 0.3 : 0.35 + (zone.brightness / 100) * 0.65};
            "
          ></div>
        {/if}
        {#if editZones && !zone.mask && !zone.rect}
          <button
            class="zone-handle"
            class:dragging={dragging === zone.index}
            style="left: {zone.spot.x * 100}%; top: {zone.spot.y * 100}%"
            aria-label="Move the {zone.locationName} zone"
            onpointerdown={(e) => {
              (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
              dragging = zone.index;
              moveZone(e, zone.index, zone.spot);
            }}
            onpointermove={(e) => dragging === zone.index && moveZone(e, zone.index, zone.spot)}
            onpointerup={() => (dragging = null)}
          >
            {zone.locationName}
          </button>
        {/if}
      {/each}
      {#if litZones.length === 1}
        <!-- A single zone also throws light onto the surroundings. -->
        <div
          class="ambient"
          style="background: radial-gradient(closest-side, {litZones[0].color}55, transparent 70%);
                 opacity: {litZones[0].brightness / 100}"
        ></div>
      {/if}
    </div>
  {:else if kind === "mouse"}
    <svg viewBox="0 0 130 210" role="img" aria-label="Mouse">
      <defs>
        <linearGradient id="{uid}-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="#4a4a52" />
          <stop offset="45%" stop-color="#2d2d34" />
          <stop offset="100%" stop-color="#17171b" />
        </linearGradient>
        <linearGradient id="{uid}-sheen" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#ffffff" stop-opacity="0.16" />
          <stop offset="60%" stop-color="#ffffff" stop-opacity="0" />
        </linearGradient>
        <radialGradient id="{uid}-glow">
          <stop offset="0%" stop-color={glowColor} stop-opacity={glowAlpha} />
          <stop offset="100%" stop-color={glowColor} stop-opacity="0" />
        </radialGradient>
      </defs>

      <ellipse cx="65" cy="198" rx="44" ry="8" fill="#000" opacity="0.55" />
      <path
        d="M65 6c30 0 47 22 50 52 3 30 2 66-6 86-7 18-23 28-44 28s-37-10-44-28c-8-20-9-56-6-86C18 28 35 6 65 6z"
        fill="url(#{uid}-body)"
        stroke="#0c0c0f"
        stroke-width="1.5"
      />
      <path
        d="M65 6c30 0 47 22 50 52 1 10 1 21 1 32H14c0-11 0-22 1-32C18 28 35 6 65 6z"
        fill="url(#{uid}-sheen)"
      />
      <!-- button split -->
      <path d="M65 8v82" stroke="#101013" stroke-width="2.5" fill="none" />
      <path d="M16 90h98" stroke="#101013" stroke-width="1.6" fill="none" opacity="0.7" />
      <!-- scroll wheel -->
      <rect x="58" y="30" width="14" height="30" rx="7" fill="#141418" />
      <rect x="60" y="32" width="10" height="26" rx="5" fill={lit ? glowColor : "#2b2b31"} opacity={lit ? glowAlpha : 1} />
      <!-- dpi button -->
      <rect x="59.5" y="68" width="11" height="7" rx="3.5" fill="#1b1b20" />
      <!-- thumb buttons -->
      <rect x="12" y="52" width="8" height="17" rx="4" fill="#202026" />
      <rect x="12" y="73" width="8" height="17" rx="4" fill="#202026" />
      <!-- logo -->
      <circle cx="65" cy="140" r="22" fill="url(#{uid}-glow)" />
      <circle cx="65" cy="140" r="9" fill="none" stroke={lit ? glowColor : "#33333a"} stroke-width="2.4" opacity={lit ? glowAlpha : 1} />
      <path d="M65 133v14M65 147h6" stroke={lit ? glowColor : "#33333a"} stroke-width="2.2" fill="none" stroke-linecap="round" opacity={lit ? glowAlpha : 1} />
    </svg>
  {:else if kind === "keyboard"}
    <svg viewBox="0 0 470 170" role="img" aria-label="Keyboard">
      <defs>
        <linearGradient id="{uid}-case" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#3a3a42" />
          <stop offset="100%" stop-color="#1a1a1e" />
        </linearGradient>
        <linearGradient id="{uid}-key" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#33333b" />
          <stop offset="100%" stop-color="#22222a" />
        </linearGradient>
        <filter id="{uid}-bloom" x="-40%" y="-40%" width="180%" height="180%">
          <feGaussianBlur stdDeviation="5" />
        </filter>
      </defs>

      <ellipse cx="235" cy="160" rx="200" ry="9" fill="#000" opacity="0.5" />

      {#if lit}
        <rect x="16" y="18" width="438" height="130" rx="12" fill={glowColor} opacity={glowAlpha * 0.5} filter="url(#{uid}-bloom)" />
      {/if}

      <rect x="12" y="12" width="446" height="136" rx="10" fill="url(#{uid}-case)" stroke="#0d0d10" stroke-width="1.5" />
      <rect x="20" y="20" width="430" height="120" rx="6" fill="#141418" />

      {#each keyRows as count, row}
        {#each Array(count) as _, col}
          {@const w = (430 - 8 - (count + 1) * 3) / count}
          <rect
            x={24 + 3 + col * (w + 3)}
            y={24 + row * 24}
            width={w}
            height={20}
            rx="3"
            fill="url(#{uid}-key)"
            stroke={lit ? glowColor : "#101014"}
            stroke-width={lit ? 1.2 : 0.8}
            stroke-opacity={lit ? glowAlpha : 1}
          />
        {/each}
      {/each}

      <!-- bottom modifier row -->
      {#each bottomRow as u, i}
        <rect
          x={bottomOffsets[i]}
          y={24 + 4 * 24}
          width={u * unit}
          height={20}
          rx="3"
          fill="url(#{uid}-key)"
          stroke={lit ? glowColor : "#101014"}
          stroke-width={lit ? 1.2 : 0.8}
          stroke-opacity={lit ? glowAlpha : 1}
        />
      {/each}

      <!-- escape key badge, like the G logo cap -->
      <rect x="27" y="24" width="26" height="20" rx="3" fill="#f0f0f2" />
      <path d="M40 29v10M40 39h4.5" stroke="#1b1b1f" stroke-width="1.8" fill="none" stroke-linecap="round" />
    </svg>
  {:else if kind === "headset"}
    <svg viewBox="0 0 200 200" role="img" aria-label="Headset">
      <defs>
        <linearGradient id="{uid}-cup" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="#43434c" />
          <stop offset="100%" stop-color="#1c1c21" />
        </linearGradient>
      </defs>

      <ellipse cx="100" cy="190" rx="62" ry="8" fill="#000" opacity="0.5" />
      <!-- headband -->
      <path d="M32 116V88a68 68 0 0 1 136 0v28" fill="none" stroke="#2c2c34" stroke-width="15" stroke-linecap="round" />
      <path d="M38 112V88a62 62 0 0 1 124 0v24" fill="none" stroke="#3e3e48" stroke-width="6" stroke-linecap="round" />
      <!-- ear cups -->
      <rect x="12" y="96" width="46" height="70" rx="22" fill="url(#{uid}-cup)" stroke="#101014" stroke-width="1.5" />
      <rect x="142" y="96" width="46" height="70" rx="22" fill="url(#{uid}-cup)" stroke="#101014" stroke-width="1.5" />
      <ellipse cx="35" cy="131" rx="15" ry="23" fill="#15151a" />
      <ellipse cx="165" cy="131" rx="15" ry="23" fill="#15151a" />
      <circle cx="35" cy="131" r="9" fill="none" stroke={lit ? glowColor : "#34343c"} stroke-width="2" opacity={lit ? glowAlpha : 1} />
      <circle cx="165" cy="131" r="9" fill="none" stroke={lit ? glowColor : "#34343c"} stroke-width="2" opacity={lit ? glowAlpha : 1} />
      <!-- boom mic -->
      <path d="M58 150c22 6 30 18 30 30" fill="none" stroke="#2c2c34" stroke-width="7" stroke-linecap="round" />
      <circle cx="88" cy="182" r="7" fill="#1c1c21" stroke="#0f0f13" stroke-width="1.5" />
    </svg>
  {:else if kind === "light"}
    <svg viewBox="0 0 200 210" role="img" aria-label="Light">
      <defs>
        <linearGradient id="{uid}-panel" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#ffffff" />
          <stop offset="100%" stop-color="#d8d8dc" />
        </linearGradient>
        <filter id="{uid}-lampglow" x="-60%" y="-200%" width="220%" height="500%">
          <feGaussianBlur stdDeviation="10" />
        </filter>
      </defs>

      <ellipse cx="100" cy="198" rx="52" ry="7" fill="#000" opacity="0.5" />
      {#if lit}
        <rect x="24" y="36" width="152" height="16" rx="8" fill={glowColor} opacity={glowAlpha * 0.6} filter="url(#{uid}-lampglow)" />
      {/if}
      <!-- light bar -->
      <rect x="24" y="36" width="152" height="15" rx="7.5" fill="url(#{uid}-panel)" />
      <rect x="28" y="39" width="144" height="5" rx="2.5" fill={lit ? glowColor : "#ffffff"} opacity={lit ? glowAlpha : 0.85} />
      <!-- stem -->
      <rect x="93" y="51" width="14" height="128" rx="3" fill="#25252b" />
      <rect x="96" y="51" width="3" height="128" fill="#3a3a44" />
      <!-- base -->
      <ellipse cx="100" cy="181" rx="44" ry="10" fill="#2a2a31" stroke="#101014" stroke-width="1.5" />
      <ellipse cx="100" cy="178" rx="44" ry="10" fill="#33333b" />
    </svg>
  {:else if kind === "microphone"}
    <svg viewBox="0 0 170 210" role="img" aria-label="Microphone">
      <defs>
        <linearGradient id="{uid}-mic" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stop-color="#3c3c45" />
          <stop offset="50%" stop-color="#22222a" />
          <stop offset="100%" stop-color="#16161a" />
        </linearGradient>
      </defs>

      <ellipse cx="85" cy="198" rx="46" ry="7" fill="#000" opacity="0.5" />
      <!-- capsule -->
      <rect x="60" y="18" width="50" height="72" rx="25" fill="url(#{uid}-mic)" stroke="#0e0e12" stroke-width="1.5" />
      <rect x="66" y="24" width="38" height="46" rx="19" fill="#101014" />
      {#each Array(7) as _, i}
        <path d="M68 {30 + i * 6}h34" stroke="#2a2a32" stroke-width="1.4" />
      {/each}
      <!-- rgb ring -->
      <rect x="62" y="78" width="46" height="7" rx="3.5" fill={lit ? glowColor : "#2e2e36"} opacity={lit ? glowAlpha : 1} />
      <!-- arm -->
      <path d="M85 90c0 26-32 26-32 52v22" fill="none" stroke="#2a2a32" stroke-width="11" stroke-linecap="round" />
      <path d="M85 90c0 26-32 26-32 52v22" fill="none" stroke="#3a3a44" stroke-width="4" stroke-linecap="round" />
      <!-- base -->
      <ellipse cx="53" cy="182" rx="40" ry="10" fill="#2a2a31" stroke="#101014" stroke-width="1.5" />
      <ellipse cx="53" cy="179" rx="40" ry="10" fill="#33333b" />
    </svg>
  {:else if kind === "speaker"}
    <svg viewBox="0 0 140 210" role="img" aria-label="Speaker">
      <ellipse cx="70" cy="198" rx="44" ry="7" fill="#000" opacity="0.5" />
      <rect x="22" y="16" width="96" height="170" rx="14" fill="#25252c" stroke="#0e0e12" stroke-width="1.5" />
      <circle cx="70" cy="128" r="34" fill="#141418" stroke="#33333b" stroke-width="2" />
      <circle cx="70" cy="128" r="14" fill="#1e1e24" />
      <circle cx="70" cy="58" r="15" fill="#141418" stroke="#33333b" stroke-width="2" />
      <rect x="30" y="176" width="80" height="5" rx="2.5" fill={lit ? glowColor : "#2e2e36"} opacity={lit ? glowAlpha : 1} />
    </svg>
  {:else if kind === "webcam"}
    <svg viewBox="0 0 200 180" role="img" aria-label="Webcam">
      <ellipse cx="100" cy="170" rx="46" ry="7" fill="#000" opacity="0.5" />
      <rect x="40" y="30" width="120" height="76" rx="16" fill="#25252c" stroke="#0e0e12" stroke-width="1.5" />
      <circle cx="100" cy="68" r="26" fill="#101014" stroke="#3a3a44" stroke-width="2.5" />
      <circle cx="100" cy="68" r="11" fill="#1b1b22" />
      <circle cx="93" cy="61" r="4" fill="#4a4a55" />
      <path d="M60 106v18h80v-18" fill="none" stroke="#2a2a32" stroke-width="8" />
      <rect x="70" y="140" width="60" height="22" rx="6" fill="#2a2a32" />
    </svg>
  {:else if kind === "wheel"}
    <svg viewBox="0 0 200 200" role="img" aria-label="Racing wheel">
      <ellipse cx="100" cy="190" rx="56" ry="8" fill="#000" opacity="0.5" />
      <circle cx="100" cy="96" r="76" fill="none" stroke="#26262d" stroke-width="20" />
      <circle cx="100" cy="96" r="76" fill="none" stroke="#3a3a44" stroke-width="5" />
      <circle cx="100" cy="96" r="26" fill="#22222a" stroke="#0e0e12" stroke-width="1.5" />
      <path d="M100 70V34M54 132l26-16M146 132l-26-16" stroke="#26262d" stroke-width="14" stroke-linecap="round" />
      <rect x="84" y="88" width="32" height="8" rx="4" fill={lit ? glowColor : "#2e2e36"} opacity={lit ? glowAlpha : 1} />
    </svg>
  {:else if kind === "receiver"}
    <svg viewBox="0 0 200 120" role="img" aria-label="Receiver">
      <rect x="40" y="36" width="90" height="48" rx="8" fill="#25252c" stroke="#0e0e12" stroke-width="1.5" />
      <rect x="128" y="50" width="36" height="20" rx="3" fill="#9a9aa4" />
      <rect x="132" y="54" width="28" height="12" rx="2" fill="#6d6d78" />
      <path d="M62 50v20M76 46v28" stroke="#3a3a44" stroke-width="4" stroke-linecap="round" />
    </svg>
  {:else}
    <svg viewBox="0 0 180 180" role="img" aria-label="Device">
      <rect x="30" y="30" width="120" height="120" rx="22" fill="#25252c" stroke="#0e0e12" stroke-width="1.5" />
      <rect x="62" y="62" width="56" height="56" rx="10" fill="#16161b" stroke="#3a3a44" stroke-width="2" />
    </svg>
  {/if}
</div>

<style>
  .art {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .art :global(svg) {
    /* `meet` letterboxing needs a bounded box in both axes, or the intrinsic
       aspect ratio expands the element instead. */
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
    min-width: 0;
    min-height: 0;
  }

  .photo-wrap {
    /* Absolutely pinned: a percentage height on an <img> only works when its
       parent has a definite height, and a flex/grid item's often is not. */
    position: absolute;
    inset: 0;
  }

  .photo {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 4%;
    box-sizing: border-box;
    z-index: 25;
  }

  /* Callers that size the box to the image exactly opt out of the padding. */
  .art.tight .photo {
    padding: 0;
  }

  .zone-glow {
    position: absolute;
    transform: translate(-50%, -50%);
    height: 0;
    border-radius: 50%;
    pointer-events: none;
    /* `screen` adds light rather than painting over the render, so the product
       underneath stays visible the way a real LED behaves. */
    mix-blend-mode: screen;
    transition: opacity 200ms var(--ease), background 200ms var(--ease);
  }

  .zone-mask {
    position: absolute;
    inset: 4%;
    pointer-events: none;
    /* `contain` + centre matches how the base render is laid out, so the mask
       lines up with the photo at any size. */
    mask-repeat: no-repeat;
    mask-position: center;
    mask-size: contain;
    -webkit-mask-repeat: no-repeat;
    -webkit-mask-position: center;
    -webkit-mask-size: contain;
    mix-blend-mode: screen;
    transition: opacity 200ms var(--ease), background 200ms var(--ease);
  }

  .zone-rect {
    position: absolute;
    pointer-events: none;
    border-radius: 18%;
    filter: blur(6px);
    mix-blend-mode: screen;
    transition: opacity 200ms var(--ease), background 200ms var(--ease);
  }

  .zone-handle {
    position: absolute;
    transform: translate(-50%, -50%);
    padding: 3px 9px;
    border: 1px solid var(--cyan);
    border-radius: var(--radius-pill);
    background: rgba(0, 0, 0, 0.72);
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text);
    cursor: grab;
    touch-action: none;
    white-space: nowrap;
  }

  .zone-handle.dragging {
    cursor: grabbing;
    background: var(--cyan);
    color: #04222b;
  }

  .ambient {
    position: absolute;
    inset: -10%;
    border-radius: 50%;
    pointer-events: none;
    mix-blend-mode: screen;
    filter: blur(18px);
    transition: opacity 200ms var(--ease);
  }
</style>
