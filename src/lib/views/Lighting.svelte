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
  import RegionPicker from "$lib/components/RegionPicker.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import { artworkIds, batteryIcon, batteryLabel } from "$lib/device-ui";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, LightEffectName, LightingSettings, SoftwareEffect, ZoneInfo } from "$lib/types";
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
    cycle: "Cycle",
    screen: "Screen sampler",
    audio: "Audio visualizer",
  };

  /** Effects OpenGHub runs itself, streaming colours to the zone. */
  const SOFTWARE: LightEffectName[] = ["screen", "audio"];
  const SCREEN_DEFAULT: SoftwareEffect = { kind: "screen", region: { x: 0, y: 0, w: 1, h: 1 }, brightness: 100 };
  const AUDIO_DEFAULT: SoftwareEffect = { kind: "audio", low: "#ff2d2d", mid: "#00b8fc", high: "#ffffff", sensitivity: 60, brightness: 100 };
  let syncStatus = $state<{ activeZones: number; error: string | null; screenAuthorised: boolean } | null>(null);

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
  /** With a depot layout the zones already sit where the LEDs are. */
  const hasLayout = $derived(artwork.layoutFor(artworkIds(device)) !== null);
  let syncingOptions = $state(false);
  const zoneInfo = $derived(zones.find((z) => z.index === activeZone));
  /** Only offer effects this particular zone advertises. */
  const available = $derived([
    ...(zoneInfo?.effects ?? [0x00, 0x01, 0x03, 0x0a])
      .map((id) => EFFECT_BY_ID[id])
      .filter((name): name is LightEffectName => !!name),
    ...SOFTWARE,
  ]);

  /** The active zone's software parameters, with defaults for its kind. */
  const software = $derived.by<SoftwareEffect | null>(() => {
    if (current.effect === "screen") return current.software?.kind === "screen" ? current.software : SCREEN_DEFAULT;
    if (current.effect === "audio") return current.software?.kind === "audio" ? current.software : AUDIO_DEFAULT;
    return null;
  });

  // Poll the engine status while a software effect is shown, so a refused
  // screen share or a missing helper is visible where the effect was chosen.
  $effect(() => {
    if (!software) {
      syncStatus = null;
      return;
    }
    let alive = true;
    const tick = () => api.getLightSyncStatus().then((s) => alive && (syncStatus = s)).catch(() => {});
    tick();
    const t = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  });

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

    // Cache the zone names so the dashboard can render per-zone lighting
    // without a HID++ round trip per card.
    const names = zones.map((z) => z.locationName);
    if (JSON.stringify(names) !== JSON.stringify(profile.zoneNames ?? [])) {
      await configStore.saveDeviceProfile(id, { ...profile, zoneNames: names });
    }
  }

  async function apply(zoneIndex: number) {
    const settings = byZone[String(zoneIndex)];
    if (!settings) return;
    applying = true;
    try {
      if (SOFTWARE.includes(settings.effect)) {
        const fx = settings.software && settings.software.kind === settings.effect
          ? settings.software
          : settings.effect === "screen" ? SCREEN_DEFAULT : AUDIO_DEFAULT;
        // Keep the stored parameters in step with what runs.
        if (settings.software !== fx) {
          byZone = { ...byZone, [String(zoneIndex)]: { ...settings, software: fx } };
        }
        await api.setZoneSoftwareEffect(device.id, zoneIndex, fx);
        await persist();
        return;
      }
      // A firmware effect replaces any software one on this zone.
      await api.setZoneSoftwareEffect(device.id, zoneIndex, null);
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
      zoneNames: zones.map((z) => z.locationName),
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

  /**
   * G HUB's "Sync lighting options": pushes this zone's effect to every other
   * lighting-capable device, all their zones, and saves it into their profiles.
   */
  async function syncOptions() {
    if (syncingOptions) return;
    syncingOptions = true;
    const source = { ...current };
    let touched = 0;
    try {
      for (const other of deviceStore.devices) {
        if (other.id === device.id || !other.capabilities.lighting) continue;
        let otherZones: ZoneInfo[];
        try {
          otherZones = await api.getLightingZones(other.id);
        } catch {
          otherZones = [{ index: 0, location: 1, locationName: "Primary", effects: [0, 1, 3, 10] }];
        }
        const settings: Record<string, LightingSettings> = {};
        for (const z of otherZones) {
          settings[String(z.index)] = { ...source };
          await api.setDeviceLighting({
            deviceId: other.id,
            zone: z.index,
            color: source.color,
            effect: source.effect,
            brightness: source.brightness,
            rateMs: source.rateMs,
            persist: true,
          });
        }
        const profile = configStore.deviceProfile(other.id);
        await configStore.saveDeviceProfile(other.id, {
          ...profile,
          lighting: { ...source },
          lightingZones: settings,
        });
        touched += 1;
      }
      // This device's other zones follow too, as in G HUB.
      await syncZones(false);
      ui.toast(
        touched === 0 ? "No other lighting devices connected." : `Lighting synced to ${touched} other device${touched === 1 ? "" : "s"}.`,
        touched === 0 ? "info" : "success",
        3000,
      );
    } catch (e) {
      ui.toast(`Could not sync lighting: ${api.errorMessage(e)}`, "error", 6000);
    } finally {
      syncingOptions = false;
    }
  }

  /** Copies the active zone onto every other zone and applies them all. */
  async function syncZones(notify = true) {
    const source = current;
    const next: Record<string, LightingSettings> = {};
    for (const zone of zones) next[String(zone.index)] = { ...source };
    byZone = next;
    for (const zone of zones) await apply(zone.index);
    if (notify) ui.toast("All lighting zones synced.", "success", 2500);
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

      {#if current.effect === "breathing" || current.effect === "cycle"}
        <Slider
          value={current.rateMs}
          min={500}
          max={20000}
          step={500}
          label="Effect rate"
          suffix="ms"
          oninput={(v) => update({ rateMs: v })}
          onchange={() => apply(activeZone)}
        />
      {/if}

      {#if software?.kind === "screen"}
        <div class="field">
          <span class="label">Screen region</span>
          <RegionPicker
            value={software.region}
            onchange={(region) => {
              update({ software: { ...software, region } });
              apply(activeZone);
            }}
          />
        </div>
        <Slider
          value={software.brightness}
          min={0}
          max={100}
          label="Effect brightness"
          suffix="%"
          oninput={(v) => update({ software: { ...software, brightness: v }, brightness: v })}
          onchange={() => apply(activeZone)}
        />
      {:else if software?.kind === "audio"}
        <div class="field">
          <span class="label">Colours</span>
          <div class="bands">
            {#each [["low", "Bass"], ["mid", "Mids"], ["high", "Treble"]] as [key, name] (key)}
              <label class="band">
                <input
                  type="color"
                  value={software[key as "low" | "mid" | "high"]}
                  onchange={(e) => {
                    update({ software: { ...software, [key]: e.currentTarget.value } });
                    apply(activeZone);
                  }}
                />
                <span>{name}</span>
              </label>
            {/each}
          </div>
        </div>
        <Slider
          value={software.sensitivity}
          min={0}
          max={100}
          label="Sensitivity"
          suffix="%"
          oninput={(v) => update({ software: { ...software, sensitivity: v } })}
          onchange={() => apply(activeZone)}
        />
        <Slider
          value={software.brightness}
          min={0}
          max={100}
          label="Effect brightness"
          suffix="%"
          oninput={(v) => update({ software: { ...software, brightness: v }, brightness: v })}
          onchange={() => apply(activeZone)}
        />
      {/if}

      {#if software}
        <p class="hint" class:error={!!syncStatus?.error}>
          {#if syncStatus?.error}
            {syncStatus.error}
          {:else if software.kind === "screen"}
            {syncStatus?.screenAuthorised
              ? "Sampling your screen through the desktop portal."
              : "Your desktop will ask once which screen to share."}
          {:else}
            Listening to what is playing on the default output.
          {/if}
        </p>
      {/if}

      {#if current.effect !== "off" && !software}
        <Slider
          value={current.brightness}
          min={0}
          max={100}
          label="Effect brightness"
          suffix="%"
          oninput={(v) => update({ brightness: v })}
          onchange={() => apply(activeZone)}
        />
      {/if}

      {#if zones.length > 1}
        <button class="wide" onclick={() => syncZones()}>Sync lighting zones</button>
      {/if}

      <div class="placement" class:hidden={hasLayout && !editZones}>
        <button class="link" class:on={editZones} onclick={() => (editZones = !editZones)}>
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

    {/if}
  {/snippet}

  {#snippet stageAside()}
    {#if device.battery}
      <div class="battery">
        <span class="battery-label">Battery level</span>
        <span class="battery-value">
          {device.battery.percentage}%
          <Icon name={batteryIcon(device.battery)} size={16} />
        </span>
        <span class="battery-label">{batteryLabel(device.battery)}</span>
      </div>
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
    <button class="stage-button" onclick={syncOptions} disabled={syncingOptions}>
      {syncingOptions ? "Syncing…" : "Sync lighting options"}
    </button>
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
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-label);
  }

  .select {
    position: relative;
    align-self: flex-start;
    display: flex;
    align-items: center;
    color: var(--text-dim);
  }

  .select select {
    appearance: none;
    width: auto;
    padding: 4px 22px 4px 0;
    border: none;
    border-bottom: 1px solid transparent;
    background: transparent;
    font-family: var(--font);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.04em;
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

  .select::after {
    content: "";
    position: absolute;
    right: 8px;
    width: 7px;
    height: 7px;
    border-right: 1.5px solid var(--text);
    border-bottom: 1.5px solid var(--text);
    transform: translateY(-2px) rotate(45deg);
    pointer-events: none;
  }

  .select option {
    background: #161616;
  }

  .wide {
    width: 100%;
    padding: 9px;
    border-radius: 4px;
    background: var(--surface-3);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .link {
    align-self: flex-start;
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-decoration: underline;
    color: var(--text-dim);
  }

  .link:hover,
  .link.on {
    color: var(--cyan);
  }

  .hidden {
    display: none;
  }

  .wide:hover:not(:disabled) {
    background: var(--surface-3);
  }

  .wide:disabled {
    opacity: 0.55;
    cursor: default;
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

  .hint.error {
    color: var(--warning);
  }

  .bands {
    display: flex;
    gap: 14px;
  }

  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dim);
    cursor: pointer;
  }

  .band input {
    width: 34px;
    height: 34px;
    padding: 0;
    border: 2px solid var(--line-strong);
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }

  .band input::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .band input::-webkit-color-swatch {
    border: none;
    border-radius: 50%;
  }

  .battery {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    text-align: right;
  }

  .battery-label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-label);
  }

  .battery-value {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
    font-weight: 700;
  }

  .stage-button {
    min-width: 250px;
    padding: 10px 30px;
    border-radius: 4px;
    background: var(--surface-3);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .stage-button:hover:not(:disabled) {
    background: #454545;
  }

  .stage-button:disabled {
    opacity: 0.6;
  }
</style>
