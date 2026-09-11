<script lang="ts">
  /** Sensitivity (DPI) — stages, pointer speed and report rate. */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import DpiStages from "$lib/components/DpiStages.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Segmented from "$lib/components/Segmented.svelte";
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
    });
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

  const rateOptions = $derived(
    (device.reportRate?.availableHz ?? []).map((hz) => ({
      value: hz,
      label: hz >= 1000 ? `${hz / 1000}K` : `${hz}`,
    })),
  );
</script>

<DeviceWorkspace title="Sensitivity (DPI)">
  {#snippet panel()}
    {#if dpi}
      <DpiStages
        bind:stages
        bind:activeStage
        min={dpi.min}
        max={dpi.max}
        step={dpi.step || 50}
        {allowed}
        onchange={persist}
        onselect={persist}
        oncommit={applyDpi}
      />

      <dl class="facts">
        <div><dt>Sensor range</dt><dd>{dpi.min.toLocaleString()} – {dpi.max.toLocaleString()}</dd></div>
        <div><dt>Granularity</dt><dd>{dpi.step ? `${dpi.step} DPI` : `${dpi.steps.length} presets`}</dd></div>
        <div><dt>Device default</dt><dd>{dpi.default.toLocaleString()} DPI</dd></div>
        <div><dt>Reported now</dt><dd>{dpi.current.toLocaleString()} DPI</dd></div>
      </dl>
    {:else}
      <p class="hint">This device has no adjustable sensor.</p>
    {/if}

    <div class="rate">
      <span class="label">Report rate</span>
      {#if device.reportRate && rateOptions.length}
        <Segmented
          value={device.reportRate.currentHz}
          options={rateOptions}
          disabled={busy}
          onselect={applyRate}
        />
        <p class="hint">
          <Icon name="info" size={12} />
          {device.reportRate.extended
            ? "HID++ feature 0x8061 (extended report rate)."
            : "HID++ feature 0x8060."}
          A device running its onboard profile owns this setting, so OpenGHub takes host mode
          when it has to. The device returns to its own profile when reconnected.
        </p>
      {:else}
        <p class="hint">This device runs at a fixed report rate.</p>
      {/if}
    </div>

    <p class="hint foot">
      {#if busy}Applying…{:else}Stored in <strong>{configStore.active?.name}</strong>.{/if}
    </p>
  {/snippet}

  {#snippet stage()}
    <div class="stage-inner">
      <DeviceArt kind={device.kind} productIds={artworkIds(device)} class="render" />
      <div class="readout">
        <span class="big">{(stages[activeStage] ?? dpi?.current ?? 0).toLocaleString()}</span>
        <span class="unit">DPI</span>
      </div>
    </div>
  {/snippet}
</DeviceWorkspace>

<style>
  .stage-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .stage-inner :global(.render) {
    flex: 1;
    min-height: 0;
  }

  .readout {
    display: flex;
    align-items: baseline;
    gap: 9px;
  }

  .big {
    font-family: var(--font);
    font-size: 44px;
    font-weight: 600;
    line-height: 1;
  }

  .unit {
    font-size: 12px;
    letter-spacing: 0.2em;
    color: var(--text-dim);
  }

  .rate {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
  }

  .label {
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .facts div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  dt {
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  dd {
    font-family: var(--font);
    font-size: 14px;
    font-weight: 600;
  }

  .hint {
    display: flex;
    align-items: flex-start;
    gap: 6px;
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
</style>
