<script lang="ts">
  /**
   * Steering Wheel — G HUB's wheel page: operating range, sensitivity,
   * centering spring, the TRUEFORCE block and RESTORE DEFAULT SETTINGS on the
   * left; on the right the live angle readout over a tick-mark arc that shows
   * the operating range, with the rim turning as the real wheel does.
   *
   * Range and spring go to the wheel immediately (classic commands).
   * Sensitivity and the force-feedback torque act inside OpenGHub's own
   * driver, which is what games talk to.
   */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import Segmented from "$lib/components/Segmented.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, LightingSettings, SoftwareEffect, WheelSettings, WheelState } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  const DEFAULTS: WheelSettings = {
    rangeDeg: 900,
    sensitivity: 50,
    centerSpring: 20,
    centerSpringInFfbGames: false,
    ffbGain: 100,
    centerOffset: 0,
    trueforceTorque: 100,
    trueforceAudio: 100,
    trueforceGameControl: true,
    pedals: {
      accelerator: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
      brake: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
      clutch: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
      combined: false,
    },
    gamepadMode: false,
  };

  let settings = $state<WheelSettings>({ ...DEFAULTS });
  /** Byte 51 of the PS report lands on buttons 21-28: gears 1-6, then reverse at bit 7. */
  function gearOf(buttons: number): number | null {
    const s = (buttons >>> 20) & 0xff;
    if (s & 0x80) return 0;
    for (let g = 0; g < 6; g++) if (s & (1 << g)) return g + 1;
    return null;
  }

  let live = $state<WheelState>({ steeringRaw: 32768, steering: 0, accelerator: 0, brake: 0, clutch: 0, buttons: 0, hat: 8 });
  const gear = $derived(gearOf(live.buttons));
  let seededFor = $state<string | null>(null);
  let busy = $state(false);
  let testingLeds = $state(false);

  const info = $derived(device.wheel);
  const rangeMin = $derived(info?.rangeMin ?? 40);
  const rangeMax = $derived(info?.rangeMax ?? 900);
  const angle = $derived(live.steering * settings.rangeDeg / 2);
  const rim = $derived(artwork.forProductIds(artworkIds(device)));
  const base = $derived(artwork.baseFor(artworkIds(device)));

  // Seed once per device; writing settings back must not re-seed.
  $effect(() => {
    const id = device.id;
    if (seededFor === id) return;
    seededFor = id;
    untrack(() => {
      const saved = configStore.deviceProfile(id).wheel;
      if (saved) settings = { ...DEFAULTS, ...saved };
      api.getWheelSettings(id).then((s) => (settings = { ...DEFAULTS, ...s })).catch(() => {});
      api.getWheelState(id).then((s) => (live = s)).catch(() => {});
    });
  });

  // Live inputs from the backend (30 Hz while moving).
  $effect(() => {
    let off: (() => void) | undefined;
    api.on<{ deviceId: string; state: WheelState }>(api.events.wheelState, (e) => {
      if (e.deviceId === device.id) live = e.state;
    }).then((u) => (off = u));
    // Outside Tauri the mock has no events: poll instead.
    const timer = api.isTauri ? null : setInterval(() => api.getWheelState(device.id).then((s) => (live = s)), 50);
    return () => {
      off?.();
      if (timer) clearInterval(timer);
    };
  });

  async function commit(patch: Partial<WheelSettings>) {
    const next = { ...settings, ...patch };
    settings = next;
    busy = true;
    try {
      settings = await api.setWheelSettings(device.id, next);
    } catch (e) {
      ui.toast(`Could not apply: ${api.errorMessage(e)}`, "error", 6000);
    } finally {
      busy = false;
    }
  }

  function restoreDefaults() {
    commit({ ...DEFAULTS, rangeDeg: rangeMax, centerOffset: settings.centerOffset, pedals: settings.pedals });
  }

  // -- RPM LEDs as a LightSync target -----------------------------------------
  //
  // The wheel has no colour zones, but its row of RPM LEDs makes a fine VU
  // meter: the audio visualizer lights them by loudness, the screen sampler
  // by how bright the sampled region is. Games keep the LEDs when "Games".
  type LedMode = "games" | "audio" | "screen";
  const LED_ZONE = "0";
  const AUDIO_DEFAULT = { kind: "audio" as const, low: "#ff2d2d", mid: "#00b8fc", high: "#ffffff", sensitivity: 60, brightness: 100 };
  const SCREEN_DEFAULT: SoftwareEffect = { kind: "screen", region: { x: 0, y: 0, w: 1, h: 1 }, brightness: 100 };
  const ledSettings = $derived<LightingSettings | null>(configStore.deviceProfile(device.id).lightingZones?.[LED_ZONE] ?? null);
  const ledMode = $derived<LedMode>(
    ledSettings?.effect === "audio" || ledSettings?.effect === "screen" ? ledSettings.effect : "games",
  );
  const ledSensitivity = $derived(ledSettings?.software?.kind === "audio" ? ledSettings.software.sensitivity : 60);
  let ledStatus = $state<{ error: string | null } | null>(null);

  $effect(() => {
    if (ledMode === "games") {
      ledStatus = null;
      return;
    }
    let alive = true;
    const tick = () => api.getLightSyncStatus().then((s) => alive && (ledStatus = s)).catch(() => {});
    tick();
    const timer = setInterval(tick, 1500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  });

  async function setLedMode(mode: LedMode, sensitivity = ledSensitivity) {
    const profile = configStore.deviceProfile(device.id);
    const zones = { ...(profile.lightingZones ?? {}) };
    let fx: SoftwareEffect | null = null;
    if (mode === "games") {
      delete zones[LED_ZONE];
    } else {
      fx = mode === "audio" ? { ...AUDIO_DEFAULT, sensitivity } : SCREEN_DEFAULT;
      zones[LED_ZONE] = { effect: mode, color: "#ffffff", brightness: 100, rateMs: 0, software: fx };
    }
    try {
      await api.setZoneSoftwareEffect(device.id, 0, fx);
      await configStore.saveDeviceProfile(device.id, { ...profile, lightingZones: zones });
      if (mode === "games") await api.setWheelLeds(device.id, 0);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 6000);
    }
  }

  /** Lights the RPM LEDs in a quick chase, so the user knows the channel is live. */
  async function testLeds() {
    if (testingLeds || !info?.rpmLeds) return;
    testingLeds = true;
    try {
      const n = info.rpmLeds;
      for (let i = 1; i <= n; i++) {
        await api.setWheelLeds(device.id, (1 << i) - 1);
        await new Promise((r) => setTimeout(r, 90));
      }
      for (let i = n - 1; i >= 0; i--) {
        await api.setWheelLeds(device.id, (1 << i) - 1);
        await new Promise((r) => setTimeout(r, 90));
      }
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      testingLeds = false;
    }
  }

  /** Tick marks along a 200° arc; the lit ones cover the current angle. */
  const TICKS = 61;
  const ARC = 200;
  function tickTransform(i: number): string {
    const a = -ARC / 2 + (ARC * i) / (TICKS - 1);
    return `rotate(${a})`;
  }
  function tickLit(i: number): boolean {
    const a = -ARC / 2 + (ARC * i) / (TICKS - 1);
    const pos = (angle / (settings.rangeDeg / 2)) * (ARC / 2);
    if (Math.abs(pos) < ARC / (TICKS - 1)) return Math.abs(a) < ARC / (TICKS - 1);
    return pos > 0 ? a > 0 && a <= pos : a < 0 && a >= pos;
  }
</script>

<DeviceWorkspace title="Steering Wheel">
  {#snippet panel()}
    <Slider
      value={settings.rangeDeg}
      min={rangeMin}
      max={rangeMax}
      step={10}
      label="Operating range"
      oninput={(v) => (settings = { ...settings, rangeDeg: v })}
      onchange={(v) => commit({ rangeDeg: v })}
    />

    <Slider
      value={settings.sensitivity}
      min={0}
      max={100}
      label="Sensitivity"
      oninput={(v) => (settings = { ...settings, sensitivity: v })}
      onchange={(v) => commit({ sensitivity: v })}
    />

    <Slider
      value={settings.centerSpring}
      min={0}
      max={100}
      label="Centering spring strength"
      oninput={(v) => (settings = { ...settings, centerSpring: v })}
      onchange={(v) => commit({ centerSpring: v })}
    />

    <label class="check">
      <input
        type="checkbox"
        checked={settings.centerSpringInFfbGames}
        onchange={(e) => commit({ centerSpringInFfbGames: e.currentTarget.checked })}
      />
      <span>Centering spring in force feedback games</span>
    </label>

    <div class="trueforce">
      <div class="tf-logo" aria-label="TRUEFORCE">TRUE<span>FORCE</span></div>
      <p class="tf-copy">TRUEFORCE utilizes the actual physics engine in supported games</p>
    </div>

    <label class="check">
      <input
        type="checkbox"
        checked={settings.trueforceGameControl}
        onchange={(e) => commit({ trueforceGameControl: e.currentTarget.checked })}
      />
      <span>Apply settings from game</span>
    </label>

    <Slider
      value={settings.ffbGain}
      min={0}
      max={100}
      label="Torque"
      oninput={(v) => (settings = { ...settings, ffbGain: v })}
      onchange={(v) => commit({ ffbGain: v })}
    />

    <Slider
      value={settings.trueforceAudio}
      min={0}
      max={100}
      label="Audio effects (profile only)"
      oninput={(v) => (settings = { ...settings, trueforceAudio: v })}
      onchange={(v) => commit({ trueforceAudio: v })}
    />
    <p class="hint">
      Torque is the force-feedback strength of OpenGHub's wheel driver. TrueForce itself is
      streamed by the game straight to the wheel and works under Proton. On Windows the Audio
      effects gain is applied by Logitech's TrueForce service, not by the wheel, so here it is
      only stored with the profile — set the TrueForce gain in the game's own options. Proton
      games need <code>PROTON_DISABLE_HIDRAW=0x046d/0xc266</code> to see this driver's wheel
      next to the raw one — see the README.
    </p>

    <button class="wide" onclick={restoreDefaults} disabled={busy}>Restore default settings</button>

    <div class="leds-block">
      <span class="block-label">Games without wheel support</span>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.gamepadMode ?? false}
          onchange={(e) => commit({ gamepadMode: e.currentTarget.checked })}
        />
        <span>Gamepad mode</span>
      </label>
      <p class="hint">
        Adds a virtual Xbox 360 controller next to the wheel: steering on the left stick, accelerator
        and brake on the triggers, ✕ ○ □ △ as A B X Y, paddles (or L2 / R2) as the bumpers,
        Share / Options / PS as Back / Start / Guide, D-pad as D-pad. For games that only take a
        controller — Rocket League, for instance. Needs the force-feedback driver on.
      </p>
    </div>

    {#if info?.rpmLeds}
      <div class="leds-block">
        <span class="block-label">RPM LEDs</span>
        <Segmented
          value={ledMode}
          options={[
            { value: "games", label: "Games" },
            { value: "audio", label: "Audio" },
            { value: "screen", label: "Screen" },
          ]}
          onselect={(m) => setLedMode(m as LedMode)}
        />
        {#if ledMode === "audio"}
          <Slider
            value={ledSensitivity}
            min={0}
            max={100}
            label="Sensitivity"
            suffix="%"
            onchange={(v) => setLedMode("audio", v)}
          />
        {/if}
        <p class="hint" class:error={!!ledStatus?.error}>
          {#if ledStatus?.error}
            {ledStatus.error}
          {:else if ledMode === "audio"}
            The LEDs follow what is playing on the default output, like a VU meter.
          {:else if ledMode === "screen"}
            The LEDs follow how bright the screen is; your desktop asks once which screen to share.
          {:else}
            Games drive the LEDs as an RPM indicator.
          {/if}
        </p>
      </div>
    {/if}
  {/snippet}

  {#snippet stageAside()}
    {#if info?.rpmLeds}
      <button class="leds" onclick={testLeds} disabled={testingLeds}>
        {testingLeds ? "Testing…" : "Test RPM LEDs"}
      </button>
    {/if}
  {/snippet}

  {#snippet stage()}
    <div class="gauge">
      <div class="readout">
        <span class="deg">{Math.round(angle)}</span>
        <span class="caret"></span>
      </div>

      <svg class="arc" viewBox="-110 -110 220 130" aria-hidden="true">
        <g>
          {#each Array.from({ length: TICKS }, (_, i) => i) as i (i)}
            <line
              x1="0"
              y1="-100"
              x2="0"
              y2={i % 10 === 0 ? -86 : -90}
              transform={tickTransform(i)}
              class:lit={tickLit(i)}
            />
          {/each}
        </g>
        <text x="0" y="-52" class="range-value">{settings.rangeDeg}</text>
        <text x="0" y="-38" class="range-label">OPERATING RANGE</text>
      </svg>

      <div class="wheel-art">
        {#if base}
          <img class="base" src={base} alt="" draggable="false" />
        {/if}
        <div class="rim" class:over-base={!!base} style="transform: rotate({angle}deg)">
          {#if rim}
            <img src={rim} alt="" draggable="false" />
          {:else}
            <DeviceArt kind="wheel" />
          {/if}
        </div>
      </div>

      <div class="shifter" aria-label="Shifter" title="Driving Force Shifter">
        <span class="gear" class:neutral={gear === null}>{gear === null ? "N" : gear === 0 ? "R" : gear}</span>
        <span>Shifter</span>
      </div>

      <div class="pedals" aria-label="Pedals">
        {#each [["Clutch", live.clutch], ["Brake", live.brake], ["Accelerator", live.accelerator]] as [name, v] (name)}
          <div class="pedal">
            <div class="bar"><div class="fill" style="height: {(v as number) * 100}%"></div></div>
            <span>{name}</span>
          </div>
        {/each}
      </div>
    </div>
  {/snippet}
</DeviceWorkspace>

<style>
  .shifter {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .gear {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 2px solid var(--accent);
    border-radius: 8px;
    font-size: 22px;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  .gear.neutral {
    border-color: var(--line-strong);
    color: var(--text-dim);
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .check input {
    width: 13px;
    height: 13px;
    margin-top: 1px;
    accent-color: var(--accent);
  }

  .trueforce {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 6px 0 2px;
  }

  .tf-logo {
    font-family: var(--font);
    font-size: 22px;
    font-weight: 800;
    letter-spacing: 0.06em;
    color: var(--text);
  }

  .tf-logo span {
    position: relative;
  }

  .tf-logo span::before {
    content: "";
    position: absolute;
    left: -2px;
    top: 45%;
    width: 8px;
    height: 3px;
    background: var(--cyan);
  }

  .tf-copy {
    font-size: 11px;
    line-height: 1.4;
    text-align: center;
    color: var(--text);
  }

  .hint {
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-dimmer);
  }

  .hint.error {
    color: var(--warning);
  }

  .leds-block {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }

  .block-label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-label);
  }

  .wide {
    width: 100%;
    margin-top: 4px;
    padding: 9px;
    border-radius: 4px;
    background: var(--surface-3);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .wide:hover:not(:disabled) {
    background: #454545;
  }

  .leds {
    padding: 7px 14px;
    border-radius: 4px;
    background: var(--surface);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .leds:hover:not(:disabled) {
    background: var(--surface-3);
  }

  /* ---- gauge ------------------------------------------------------------- */
  .gauge {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .readout {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .deg {
    font-family: var(--font);
    font-size: 28px;
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .caret {
    width: 0;
    height: 0;
    border-left: 5px solid transparent;
    border-right: 5px solid transparent;
    border-top: 6px solid var(--text);
  }

  .arc {
    width: min(100%, 400px);
    flex: none;
    margin-top: -4px;
  }

  .arc line {
    stroke: #2c5a72;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .arc line.lit {
    stroke: var(--cyan);
    stroke-width: 2.2;
  }

  .range-value {
    fill: var(--cyan);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    text-anchor: middle;
  }

  .range-label {
    fill: var(--cyan);
    font-family: var(--font);
    font-size: 6px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-anchor: middle;
  }

  .wheel-art {
    position: relative;
    flex: 1;
    width: min(100%, 460px);
    min-height: 0;
    margin-top: -56px;
  }

  .wheel-art img,
  .wheel-art :global(.art),
  .wheel-art :global(svg) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .rim {
    position: absolute;
    inset: 0;
    transform-origin: 50% 50%;
    transition: transform 40ms linear;
  }

  /* The depot's rim (2400 px) sits on its 3276 px base canvas: 73% wide,
     centred, its hub a little above the base's middle. */
  .rim.over-base {
    inset: 8% 13.4% auto;
    height: 73.3%;
  }

  .pedals {
    display: flex;
    gap: 22px;
    padding: 8px 0 4px;
  }

  .pedal {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .bar {
    position: relative;
    width: 10px;
    height: 44px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    background: var(--cyan);
  }
</style>
