<script lang="ts">
  /** Sensitivity (DPI) — stages, pointer speed and report rate. */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import DpiSlider from "$lib/components/DpiSlider.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  const dpi = $derived(device.dpi);
  /** Sensors that report a step size accept anything on the grid; others don't. */
  const allowed = $derived(dpi && dpi.step === 0 ? dpi.steps : []);

  let stages = $state<number[]>([]);
  let activeStage = $state(0);
  let shiftStage = $state<number | null>(null);
  let busy = $state(false);

  /** Which device the editor below has been seeded for. */
  let seededFor = $state<string | null>(null);

  // Seed the stage list once per device, from the saved profile or the DPI the
  // device reports. This must NOT track the config store: persisting a change
  // writes the profile back, which would re-run the effect and re-seed the very
  // state the user is editing — an infinite loop.
  $effect(() => {
    const id = device.id;
    if (seededFor === id) return;
    seededFor = id;

    untrack(() => {
      const saved = configStore.deviceProfile(id);
      if (saved.dpiStages.length) {
        stages = [...saved.dpiStages];
        activeStage = Math.min(saved.activeStage, saved.dpiStages.length - 1);
        shiftStage = saved.shiftStage ?? null;
      } else if (dpi) {
        const seeded = defaultStages(dpi.current, dpi.min, dpi.max);
        const index = seeded.indexOf(dpi.current);
        stages = seeded;
        activeStage = index >= 0 ? index : Math.floor(seeded.length / 2);
      }
    });
  });

  /** Four stages spread around the current value, the way G HUB seeds a new profile. */
  function defaultStages(current: number, min: number, max: number): number[] {
    const candidates = [current / 2, current, current * 2, current * 3]
      .map((v) => Math.round(Math.min(max, Math.max(min, v)) / 50) * 50)
      .filter((v, i, a) => a.indexOf(v) === i);
    return candidates.sort((a, b) => a - b);
  }

  async function persist() {
    const profile = configStore.deviceProfile(device.id);
    await configStore.saveDeviceProfile(device.id, {
      ...profile,
      dpiStages: stages,
      activeStage,
      shiftStage,
    });
  }

  function onStagesChange(next: number[], active: number, shift: number | null) {
    stages = next;
    activeStage = active;
    shiftStage = shift;
    persist();
  }

  /** G HUB's "click this ◆": marks the current speed as the DPI-shift speed. */
  function markShift() {
    shiftStage = shiftStage === activeStage ? null : activeStage;
    persist();
  }

  /** Back to the device's own default DPI as a single stage, at its top rate. */
  async function restoreDefaults() {
    if (!dpi) return;
    stages = defaultStages(dpi.default, dpi.min, dpi.max);
    const idx = stages.indexOf(dpi.default);
    activeStage = idx >= 0 ? idx : 0;
    shiftStage = null;
    await applyDpi(stages[activeStage]);
    const top = device.reportRate?.availableHz?.length ? Math.max(...device.reportRate.availableHz) : null;
    if (top && device.reportRate?.currentHz !== top) await applyRate(top);
  }

  async function applyDpi(value: number) {
    if (!device.capabilities.dpi) return;
    busy = true;
    try {
      const state = await deviceStore.setDpi(device.id, value);
      // The sensor may snap to a nearby value; reflect what it actually took.
      if (state.current !== value) {
        stages = stages.map((s, i) => (i === activeStage ? state.current : s));
      }
      await persist();
    } catch (e) {
      ui.toast(`Could not set DPI: ${api.errorMessage(e)}`, "error");
      await deviceStore.refreshDevice(device.id).catch(() => {});
    } finally {
      busy = false;
    }
  }

  async function applyRate(hz: number) {
    busy = true;
    try {
      await deviceStore.setReportRate(device.id, hz);
      const profile = configStore.deviceProfile(device.id);
      await configStore.saveDeviceProfile(device.id, { ...profile, reportRateHz: hz });
    } catch (e) {
      ui.toast(`Could not set report rate: ${api.errorMessage(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  /** G HUB lists rates fastest first. */
  const rateOptions = $derived([...(device.reportRate?.availableHz ?? [])].sort((a, b) => b - a));
</script>

<DeviceWorkspace title="Sensitivity (DPI)" stageAlign="top">
  {#snippet panel()}
    <p class="copy">
      DPI is the speed of your mouse on the screen. Use DPI buttons on your mouse to quickly
      change the DPI speed.
    </p>

    {#if dpi}
      <div class="section">
        <span class="label">DPI speeds</span>
        <div class="speeds">
          {#each stages as value, i (i)}
            <button
              class="speed"
              class:current={i === activeStage}
              class:shift={i === shiftStage}
              onclick={() => {
                if (i === activeStage) return;
                activeStage = i;
                persist();
                applyDpi(value);
              }}
            >
              {value.toLocaleString()}
            </button>
          {/each}
        </div>
      </div>

      <p class="copy">
        Your CURRENT DPI SPEED is underlined. Hold down a <span class="shift-word">DPI SHIFT</span>
        button to quickly toggle to another speed while gaming.
      </p>

      <a class="assign" href="/device/{device.id}/assignments">
        <Icon name="assignments" size={18} strokeWidth={1.8} />
        <span>Assign these to your mouse</span>
      </a>
    {:else}
      <p class="copy">This device has no adjustable sensor.</p>
    {/if}

    <div class="section">
      <span class="label">Report rate (per second)</span>
      {#if device.reportRate && rateOptions.length}
        <p class="copy">Choose how often the mouse reports information to your computer.</p>
        <div class="rates" role="radiogroup" aria-label="Report rate">
          {#each rateOptions as hz (hz)}
            <button
              class="rate"
              role="radio"
              aria-checked={device.reportRate.currentHz === hz}
              disabled={busy}
              onclick={() => applyRate(hz)}
            >
              <span class="rate-value">{hz}</span>
              <span class="radio" class:on={device.reportRate.currentHz === hz}></span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="copy">This device runs at a fixed report rate.</p>
      {/if}
    </div>

    <button class="restore" onclick={restoreDefaults} disabled={busy || !dpi}>Restore default settings</button>
  {/snippet}

  {#snippet stage()}
    {#if dpi}
      <div class="stage-inner">
        <h2>DPI speeds</h2>
        <div class="legend">
          <span class="diamond"></span>
          <span>DPI shift speed</span>
        </div>
        <p class="stage-copy">Click on the DPI speed number to change it manually.</p>

        <DpiSlider
          {stages}
          {activeStage}
          {shiftStage}
          min={dpi.min}
          max={dpi.max}
          step={dpi.step || 50}
          {allowed}
          disabled={busy}
          onchange={onStagesChange}
          oncommit={(value) => applyDpi(value)}
        />

        <div class="notes">
          <p>To delete a DPI speed, drag it off the slider.</p>
          <p class="shift-note">
            To change the current speed to the DPI Shift speed, click this:
            <button class="diamond-btn" onclick={markShift} aria-label="Mark the current speed as the DPI shift speed">
              <span class="diamond"></span>
            </button>
          </p>
        </div>
      </div>
    {/if}
  {/snippet}
</DeviceWorkspace>

<style>
  .copy {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .label {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--text-label);
  }

  .speeds {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .speed {
    padding: 2px 0 4px;
    font-family: var(--font);
    font-size: 17px;
    font-weight: 700;
    color: var(--text);
    border-bottom: 2px solid transparent;
  }

  .speed:hover {
    color: var(--accent);
  }

  .speed.current {
    border-bottom-color: currentColor;
  }

  .speed.shift,
  .shift-word {
    color: #f5b400;
  }

  .assign {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-decoration: underline;
    text-underline-offset: 3px;
    color: var(--text);
  }

  .assign:hover {
    color: var(--accent);
  }

  .rates {
    display: flex;
    gap: 6px;
  }

  .rate {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    width: 44px;
    padding: 4px 0;
    font-family: var(--font);
    color: var(--text-dim);
  }

  .rate-value {
    font-size: 12px;
    font-weight: 700;
  }

  .radio {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 1.5px solid var(--text);
  }

  .radio.on {
    background: var(--text);
  }

  .rate:disabled {
    opacity: 0.6;
  }

  .restore {
    width: 100%;
    margin-top: 6px;
    padding: 9px;
    border-radius: 4px;
    background: var(--surface-3);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .restore:hover:not(:disabled) {
    background: #454545;
  }

  .restore:disabled {
    opacity: 0.5;
  }

  /* ---- stage ------------------------------------------------------------ */
  .stage-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    max-width: 680px;
    padding-top: 10px;
  }

  h2 {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.4px;
    text-transform: uppercase;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 34px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .diamond {
    display: inline-block;
    width: 11px;
    height: 11px;
    border-radius: 2px;
    background: #f5b400;
    transform: rotate(45deg);
  }

  .stage-copy {
    margin-top: 14px;
    font-size: 12px;
  }

  .stage-inner :global(.dpi-slider) {
    margin-top: 30px;
  }

  .notes {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    margin-top: 60px;
    font-size: 12px;
  }

  .shift-note {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    color: #f5b400;
  }

  .diamond-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
  }

  .diamond-btn:hover .diamond {
    transform: rotate(45deg) scale(1.2);
  }
</style>
