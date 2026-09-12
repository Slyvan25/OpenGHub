<script lang="ts">
  /**
   * LIGHTSYNC — one tab per addressable zone, as G HUB does. Each zone carries
   * its own effect and colour; "sync zones" copies the active one to the rest.
   */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import ColorWheel from "$lib/components/ColorWheel.svelte";
  import DeviceArt, { type ZoneGlow } from "$lib/components/DeviceArt.svelte";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, LightEffectName, LightingSettings, ZoneInfo } from "$lib/types";
  import type { ZoneSpot } from "$lib/zones";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  /** HID++ effect id → the name our backend takes. */
  const EFFECT_BY_ID: Record<number, LightEffectName> = {
    0x00: "off",
    0x01: "fixed",
    0x03: "cycle",
    0x0a: "breathing",
  };

  const EFFECT_LABEL: Record<LightEffectName, string> = {
    off: "Off",
    fixed: "Fixed",
    breathing: "Breathing",
    cycle: "Colour cycle",
  };

  const DEFAULTS: LightingSettings = {
    effect: "fixed",
    color: "#00b5e2",
    brightness: 100,
    rateMs: 5000,
  };

  let zones = $state<ZoneInfo[]>([]);
  let activeZone = $state(0);
  /** zone index → settings */
  let byZone = $state<Record<string, LightingSettings>>({});
  let swatches = $state<string[]>(["#00b5e2", "#ff2d2d", "#ffffff", "#9002ff"]);
  let loading = $state(true);
  let applying = $state(false);
  let seededFor = $state<string | null>(null);
  let editZones = $state(false);
  let identifying = $state(false);
  /** Dragged glow positions for this device, keyed by zone index. */
  let positions = $state<Record<string, ZoneSpot>>({});

  const current = $derived(byZone[String(activeZone)] ?? DEFAULTS);
  const zoneInfo = $derived(zones.find((z) => z.index === activeZone));
  /** Only offer effects this particular zone advertises. */
  const available = $derived(
    (zoneInfo?.effects ?? [0x00, 0x01, 0x03, 0x0a])
      .map((id) => EFFECT_BY_ID[id])
      .filter((name): name is LightEffectName => !!name),
  );

  const glows = $derived<ZoneGlow[]>(
    zones.map((z) => {
      const s = byZone[String(z.index)] ?? DEFAULTS;
      return {
        index: z.index,
        locationName: z.locationName,
        color: s.effect === "off" ? null : s.color,
        brightness: s.brightness,
      };
    }),
  );

  $effect(() => {
    const id = device.id;
    if (seededFor === id) return;
    seededFor = id;
    untrack(() => load(id));
  });

  async function load(id: string) {
    loading = true;
    try {
      zones = await api.getLightingZones(id);
    } catch {
      // A device that will not describe its zones still gets one tab.
      zones = [{ index: 0, location: 1, locationName: "Primary", effects: [0, 1, 3, 10] }];
    }

    const profile = configStore.deviceProfile(id);
    const seeded: Record<string, LightingSettings> = {};
    for (const zone of zones) {
      const key = String(zone.index);
      seeded[key] = profile.lightingZones?.[key] ?? profile.lighting ?? { ...DEFAULTS };
    }
    byZone = seeded;
    positions = { ...(profile.zonePositions ?? {}) };
    activeZone = zones[0]?.index ?? 0;
    loading = false;
  }

  async function apply(zoneIndex: number) {
    const settings = byZone[String(zoneIndex)];
    if (!settings) return;
    applying = true;
    try {
      await api.setDeviceLighting({
        deviceId: device.id,
        zone: zoneIndex,
        color: settings.color,
        effect: settings.effect,
        brightness: settings.brightness,
        rateMs: settings.rateMs,
        persist: true,
      });
      await persist();
    } catch (e) {
      ui.toast(`Could not apply lighting: ${api.errorMessage(e)}`, "error", 6000);
    } finally {
      applying = false;
    }
  }

  async function persist() {
    const profile = configStore.deviceProfile(device.id);
    await configStore.saveDeviceProfile(device.id, {
      ...profile,
      lightingZones: { ...byZone },
      lighting: byZone[String(activeZone)] ?? profile.lighting,
      zonePositions: { ...positions },
    });
  }

  function update(patch: Partial<LightingSettings>) {
    byZone = {
      ...byZone,
      [String(activeZone)]: { ...current, ...patch },
    };
  }

  function moveZone(index: number, spot: ZoneSpot) {
    positions = { ...positions, [String(index)]: spot };
    persist();
  }

  function resetPositions() {
    positions = {};
    persist();
    ui.toast("Zone positions reset to the built-in defaults.", "success", 2500);
  }

  /**
   * Lights only the selected zone, briefly, so the user can see which physical
   * LED it is. The device cannot tell us where a zone is, so this is the only
   * reliable way to match a tab to a light.
   */
  async function identifyZone() {
    if (identifying) return;
    identifying = true;
    const restore = { ...byZone };
    try {
      for (const zone of zones) {
        await api.setDeviceLighting({
          deviceId: device.id,
          zone: zone.index,
          color: zone.index === activeZone ? "#ffffff" : "#000000",
          effect: zone.index === activeZone ? "fixed" : "off",
          brightness: 100,
          rateMs: 1000,
          persist: false,
        });
      }
      await new Promise((r) => setTimeout(r, 2500));
    } catch (e) {
      ui.toast(`Could not identify the zone: ${api.errorMessage(e)}`, "error");
    } finally {
      byZone = restore;
      for (const zone of zones) await apply(zone.index);
      identifying = false;
    }
  }

  /** Copies the active zone onto every other zone and applies them all. */
  async function syncZones() {
    const source = current;
    const next: Record<string, LightingSettings> = {};
    for (const zone of zones) next[String(zone.index)] = { ...source };
    byZone = next;
    for (const zone of zones) await apply(zone.index);
    ui.toast("All lighting zones synced.", "success", 2500);
  }
</script>

<DeviceWorkspace title="LIGHTSYNC">
  {#snippet panel()}
    {#if loading}
      <p class="hint">Reading lighting zones…</p>
    {:else}
      {#if zones.length > 1}
        <div class="zone-tabs" role="tablist">
          {#each zones as zone (zone.index)}
            <button
              class="zone-tab"
              class:active={zone.index === activeZone}
              role="tab"
              aria-selected={zone.index === activeZone}
              onclick={() => (activeZone = zone.index)}
            >
              {zone.locationName}
            </button>
          {/each}
        </div>
      {/if}

      <div class="field">
        <span class="label">Effect</span>
        <div class="select">
          <select
            value={current.effect}
            onchange={(e) => {
              update({ effect: e.currentTarget.value as LightEffectName });
              apply(activeZone);
            }}
          >
            {#each available as name (name)}
              <option value={name}>{EFFECT_LABEL[name]}</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </div>
      </div>

      {#if current.effect === "fixed" || current.effect === "breathing"}
        <div class="field">
          <span class="label">Colour</span>
          <ColorWheel
            value={current.color}
            {swatches}
            onchange={(hex) => {
              update({ color: hex });
              apply(activeZone);
            }}
            onswatches={(next) => (swatches = next)}
          />
        </div>
      {/if}

      {#if current.effect !== "off"}
        <Slider
          value={current.brightness}
          min={0}
          max={100}
          label="Brightness"
          suffix="%"
          oninput={(v) => update({ brightness: v })}
          onchange={() => apply(activeZone)}
        />
      {/if}

      {#if current.effect === "breathing" || current.effect === "cycle"}
        <Slider
          value={current.rateMs}
          min={500}
          max={20000}
          step={500}
          label="Cycle duration"
          suffix=" ms"
          oninput={(v) => update({ rateMs: v })}
          onchange={() => apply(activeZone)}
        />
      {/if}

      {#if zones.length > 1}
        <button class="wide" onclick={syncZones}>Sync lighting zones</button>
      {/if}

      <div class="placement">
        <button class="wide" class:on={editZones} onclick={() => (editZones = !editZones)}>
          {editZones ? "Done positioning" : "Position zones on artwork"}
        </button>
        {#if editZones}
          <p class="hint">
            Drag each label to where that light actually sits on your device. OpenGHub cannot
            read this from the hardware — it only reports that a zone is called
            “{zoneInfo?.locationName}”, not where it is.
          </p>
          <div class="row">
            <button class="wide small" onclick={identifyZone} disabled={identifying}>
              {identifying ? "Watch the device…" : `Identify ${zoneInfo?.locationName ?? "zone"}`}
            </button>
            <button class="wide small" onclick={resetPositions}>Reset</button>
          </div>
        {/if}
      </div>

      <p class="hint foot">
        {#if applying}Applying…{:else}Zone {activeZone + 1} of {zones.length}.{/if}
        Settings are stored in <strong>{configStore.active?.name}</strong>.
      </p>
    {/if}
  {/snippet}

  {#snippet stage()}
    <DeviceArt
      kind={device.kind}
      productIds={artworkIds(device)}
      zones={glows}
      zoneProductIds={artworkIds(device)}
      zoneOverrides={positions}
      {editZones}
      onzonemove={moveZone}
      class="render"
    />
  {/snippet}

  {#snippet stageFooter()}
    <button class="stage-button" onclick={() => apply(activeZone)}>Re-apply to device</button>
  {/snippet}
</DeviceWorkspace>

<style>
  .zone-tabs {
    display: flex;
    gap: 22px;
    border-bottom: 1px solid var(--line);
  }

  .zone-tab {
    padding: 0 0 9px;
    font-family: var(--font);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-dim);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }

  .zone-tab:hover {
    color: var(--text);
  }

  .zone-tab.active {
    color: var(--text);
    border-bottom-color: var(--cyan);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .label {
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-dim);
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
    padding: 7px 26px 7px 0;
    border: none;
    border-bottom: 1px solid transparent;
    background: transparent;
    font-family: var(--font);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
    cursor: pointer;
  }

  .select select:hover {
    border-bottom-color: var(--line-strong);
  }

  .select select:focus {
    outline: none;
    border-bottom-color: var(--cyan);
  }

  .select :global(svg) {
    position: absolute;
    right: 4px;
    pointer-events: none;
  }

  .select option {
    background: #161616;
  }

  .wide {
    width: 100%;
    padding: 11px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-size: 12px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text);
  }

  .wide:hover:not(:disabled) {
    background: var(--surface-3);
  }

  .wide:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .wide.on {
    border-color: var(--cyan);
    color: var(--cyan);
  }

  .wide.small {
    padding: 8px;
    font-size: 11px;
  }

  .placement {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .row .wide {
    flex: 1;
  }

  .hint {
    font-size: 11.5px;
    color: var(--text-dimmer);
    line-height: 1.5;
  }

  .hint strong {
    color: var(--text-dim);
    font-weight: 600;
  }

  .foot {
    margin-top: auto;
  }

  .stage-button {
    padding: 11px 30px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font-size: 12px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .stage-button:hover {
    background: var(--surface-3);
    color: var(--text);
  }
</style>
