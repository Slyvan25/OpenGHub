<script lang="ts">
  /**
   * Pedals — G HUB's pedal panel for wheels: per pedal a sensitivity preset
   * (Low / Medium / High, or the slider behind them), dead zones at both ends
   * and inversion, plus "combined pedals". All of it is applied on the
   * virtual axes OpenGHub's own driver reports, so it works in any game.
   */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, PedalCurve, PedalSettings, WheelSettings, WheelState } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  type PedalKey = "clutch" | "brake" | "accelerator";
  const PEDALS: { key: PedalKey; label: string }[] = [
    { key: "clutch", label: "Clutch" },
    { key: "brake", label: "Brake" },
    { key: "accelerator", label: "Accelerator" },
  ];
  const PRESETS: { label: string; sensitivity: number }[] = [
    { label: "Low", sensitivity: 25 },
    { label: "Medium", sensitivity: 50 },
    { label: "High", sensitivity: 75 },
  ];
  const DEFAULT_CURVE: PedalCurve = { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false };

  let settings = $state<WheelSettings | null>(null);
  let selected = $state<PedalKey>("brake");
  let live = $state<WheelState>({ steeringRaw: 32768, steering: 0, accelerator: 0, brake: 0, clutch: 0, buttons: 0, hat: 8 });
  let seededFor = $state<string | null>(null);

  const pedals = $derived<PedalSettings>(
    settings?.pedals ?? { accelerator: DEFAULT_CURVE, brake: DEFAULT_CURVE, clutch: DEFAULT_CURVE, combined: false },
  );
  const curve = $derived(pedals[selected]);
  const image = $derived(artwork.pedalsFor(artworkIds(device)));

  $effect(() => {
    const id = device.id;
    if (seededFor === id) return;
    seededFor = id;
    untrack(() => {
      api.getWheelSettings(id).then((s) => (settings = s)).catch(() => {});
      api.getWheelState(id).then((s) => (live = s)).catch(() => {});
    });
  });

  $effect(() => {
    let off: (() => void) | undefined;
    api.on<{ deviceId: string; state: WheelState }>(api.events.wheelState, (e) => {
      if (e.deviceId === device.id) live = e.state;
    }).then((u) => (off = u));
    const timer = api.isTauri ? null : setInterval(() => api.getWheelState(device.id).then((s) => (live = s)), 50);
    return () => {
      off?.();
      if (timer) clearInterval(timer);
    };
  });

  async function commitCurve(patch: Partial<PedalCurve>) {
    if (!settings) return;
    const next: WheelSettings = { ...settings, pedals: { ...pedals, [selected]: { ...curve, ...patch } } };
    settings = next;
    try {
      settings = await api.setWheelSettings(device.id, next);
    } catch (e) {
      ui.toast(`Could not apply: ${api.errorMessage(e)}`, "error", 6000);
    }
  }

  async function commitCombined(combined: boolean) {
    if (!settings) return;
    const next: WheelSettings = { ...settings, pedals: { ...pedals, combined } };
    settings = next;
    try {
      settings = await api.setWheelSettings(device.id, next);
    } catch (e) {
      ui.toast(`Could not apply: ${api.errorMessage(e)}`, "error", 6000);
    }
  }

  /** The response curve as an SVG path, for the preview. */
  function curvePath(c: PedalCurve): string {
    const lo = c.deadZoneLow / 100;
    const hi = 1 - c.deadZoneHigh / 100;
    const exp = c.sensitivity <= 50 ? 2 - c.sensitivity / 50 : Math.max(0.5, 1 - (c.sensitivity - 50) / 100);
    const pts: string[] = [];
    for (let i = 0; i <= 40; i++) {
      const x = i / 40;
      const t = Math.min(1, Math.max(0, (x - lo) / Math.max(0.05, hi - lo)));
      let y = Math.pow(t, exp);
      if (c.inverted) y = 1 - y;
      pts.push(`${(x * 100).toFixed(1)},${(100 - y * 100).toFixed(1)}`);
    }
    return "M" + pts.join(" L");
  }
</script>

<DeviceWorkspace title="Pedals">
  {#snippet panel()}
    <div class="tabs" role="tablist">
      {#each PEDALS as p (p.key)}
        <button class="tab" class:active={selected === p.key} role="tab" aria-selected={selected === p.key} onclick={() => (selected = p.key)}>
          {p.label}
        </button>
      {/each}
    </div>

    <div class="field">
      <span class="label">Sensitivity</span>
      <div class="presets">
        {#each PRESETS as p (p.label)}
          <button class="preset" class:active={curve.sensitivity === p.sensitivity} onclick={() => commitCurve({ sensitivity: p.sensitivity })}>
            {p.label}
          </button>
        {/each}
      </div>
    </div>

    <Slider
      value={curve.sensitivity}
      min={0}
      max={100}
      label="Response"
      oninput={(v) => settings && (settings = { ...settings, pedals: { ...pedals, [selected]: { ...curve, sensitivity: v } } })}
      onchange={(v) => commitCurve({ sensitivity: v })}
    />
    <Slider
      value={curve.deadZoneLow}
      min={0}
      max={40}
      label="Dead zone (start)"
      suffix="%"
      oninput={(v) => settings && (settings = { ...settings, pedals: { ...pedals, [selected]: { ...curve, deadZoneLow: v } } })}
      onchange={(v) => commitCurve({ deadZoneLow: v })}
    />
    <Slider
      value={curve.deadZoneHigh}
      min={0}
      max={40}
      label="Dead zone (end)"
      suffix="%"
      oninput={(v) => settings && (settings = { ...settings, pedals: { ...pedals, [selected]: { ...curve, deadZoneHigh: v } } })}
      onchange={(v) => commitCurve({ deadZoneHigh: v })}
    />

    <label class="check">
      <input type="checkbox" checked={curve.inverted} onchange={(e) => commitCurve({ inverted: e.currentTarget.checked })} />
      <span>Invert this pedal</span>
    </label>

    <label class="check">
      <input type="checkbox" checked={pedals.combined} onchange={(e) => commitCombined(e.currentTarget.checked)} />
      <span>Combined pedals (brake and accelerator on one axis)</span>
    </label>

    <p class="hint">
      Applied on the virtual wheel OpenGHub's driver reports to games; a game reading the raw
      wheel over hidraw sees the pedals unshaped.
    </p>
  {/snippet}

  {#snippet stage()}
    <div class="stage-inner">
      <div class="preview">
        <svg viewBox="-8 -8 116 116" aria-hidden="true">
          <rect x="0" y="0" width="100" height="100" class="grid" />
          <path d="M0 50 H100 M50 0 V100" class="grid-line" />
          <path d={curvePath(curve)} class="curve" />
          {#if live}
            {@const raw = selected === "clutch" ? live.clutch : selected === "brake" ? live.brake : live.accelerator}
            <line x1={raw * 100} y1="0" x2={raw * 100} y2="100" class="marker" />
          {/if}
        </svg>
        <div class="axis-labels"><span>Pedal travel</span><span>Output</span></div>
      </div>

      <div class="pedals-art">
        {#if image}
          <img src={image} alt="" draggable="false" />
        {/if}
        <div class="bars">
          {#each PEDALS as p (p.key)}
            {@const v = p.key === "clutch" ? live.clutch : p.key === "brake" ? live.brake : live.accelerator}
            <button class="bar-wrap" class:active={selected === p.key} onclick={() => (selected = p.key)}>
              <div class="bar"><div class="fill" style="height: {v * 100}%"></div></div>
              <span>{p.label}</span>
              <small>{Math.round(v * 100)}%</small>
            </button>
          {/each}
        </div>
      </div>
    </div>
  {/snippet}
</DeviceWorkspace>

<style>
  .tabs {
    display: flex;
    gap: 22px;
    border-bottom: 1px solid var(--line);
  }

  .tab {
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

  .tab.active {
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

  .presets {
    display: flex;
    gap: 6px;
  }

  .preset {
    flex: 1;
    padding: 8px 0;
    border-radius: 4px;
    background: var(--surface-3);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .preset.active {
    background: var(--cyan);
    color: #fff;
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

  .hint {
    font-size: 11px;
    line-height: 1.45;
    color: var(--text-dimmer);
  }

  .stage-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 26px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .preview {
    width: 240px;
    flex: none;
  }

  .preview svg {
    width: 100%;
    display: block;
  }

  .grid {
    fill: var(--surface);
    stroke: var(--line-strong);
    stroke-width: 0.8;
  }

  .grid-line {
    stroke: var(--line);
    stroke-width: 0.6;
  }

  .curve {
    fill: none;
    stroke: var(--cyan);
    stroke-width: 2.2;
    stroke-linecap: round;
  }

  .marker {
    stroke: #f5b400;
    stroke-width: 1.2;
    stroke-dasharray: 3 2;
  }

  .axis-labels {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .pedals-art {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    gap: 14px;
    width: min(100%, 460px);
    min-height: 0;
  }

  .pedals-art img {
    flex: 1;
    min-height: 0;
    max-width: 100%;
    object-fit: contain;
  }

  .bars {
    display: flex;
    gap: 26px;
  }

  .bar-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-family: var(--font);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .bar-wrap.active {
    color: var(--text);
  }

  .bar-wrap small {
    font-size: 10px;
    color: var(--text-dimmer);
  }

  .bar {
    position: relative;
    width: 12px;
    height: 64px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }

  .bar-wrap.active .bar {
    outline: 1px solid var(--cyan);
  }

  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    background: var(--cyan);
  }
</style>
