<script lang="ts">
  /** Per-device settings, live state and the HID++ feature dump. */
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import Segmented from "$lib/components/Segmented.svelte";
  import { batteryLabel, connectionLabel } from "$lib/device-ui";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, DeviceSettings, FeatureInfo, WheelSettings } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  let features = $state<FeatureInfo[]>([]);
  let loadingFeatures = $state(false);
  let featureError = $state<string | null>(null);
  let refreshing = $state(false);

  const hex = (n: number) => `0x${n.toString(16).padStart(4, "0")}`;

  const rateOptions = $derived(
    (device.reportRate?.availableHz ?? []).map((hz) => ({
      value: hz,
      label: hz >= 1000 ? `${hz / 1000}K` : `${hz}`,
    })),
  );

  // -- G HUB's device settings: power, low battery, button layout -------------
  let ds = $state<DeviceSettings>({
    autoSleepMin: 0,
    inactivityLightingMin: 0,
    lowBatteryMode: false,
    lowBatteryThreshold: 15,
    lowBatteryBrightness: 20,
    leftHanded: false,
  });
  const SLEEP_OPTIONS = [0, 1, 2, 5, 10, 20, 30, 60];

  $effect(() => {
    api.getDeviceSettings(device.id).then((s) => (ds = s)).catch(() => {});
  });

  async function saveDs(patch: Partial<DeviceSettings>) {
    const next = { ...ds, ...patch };
    ds = next;
    try {
      ds = await api.setDeviceSettings(device.id, next);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 6000);
    }
  }

  // -- wheels: centre calibration and the force-feedback driver ---------------
  type CalStep = "idle" | "start" | "finish";
  let calStep = $state<CalStep>("idle");
  let calError = $state<string | null>(null);
  let calibrating = $state(false);
  let wheelSettings = $state<WheelSettings | null>(null);
  const CAL_MAX_DEG = 10;

  $effect(() => {
    if (!device.capabilities.wheel) return;
    api.getWheelSettings(device.id).then((s) => (wheelSettings = s)).catch(() => {});
  });

  async function calibrate() {
    calibrating = true;
    calError = null;
    try {
      wheelSettings = await api.calibrateWheelCenter(device.id, CAL_MAX_DEG);
      calStep = "finish";
    } catch (e) {
      calError = api.errorMessage(e);
    } finally {
      calibrating = false;
    }
  }

  async function resetCenter() {
    try {
      wheelSettings = await api.resetWheelCenter(device.id);
      ui.toast("Centre offset cleared.", "success", 2500);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function toggleOnboardMode(on: boolean) {
    try {
      deviceStore.patch(await api.setOnboardMode(device.id, on));
      ui.toast(on ? "On-board memory mode on — profile written to the device." : "On-board memory mode off.", "success", 3000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 6000);
    }
  }

  async function toggleDriver(enabled: boolean) {
    try {
      configStore.apply(await api.setWheelDriver(enabled));
      await deviceStore.refreshDevice(device.id).catch(() => {});
      ui.toast(enabled ? "Force-feedback driver started." : "Force-feedback driver stopped.", "success", 2500);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 7000);
    }
  }

  async function loadFeatures() {
    loadingFeatures = true;
    featureError = null;
    try {
      features = await api.getDeviceFeatures(device.id);
    } catch (e) {
      featureError = api.errorMessage(e);
    } finally {
      loadingFeatures = false;
    }
  }

  async function refresh() {
    refreshing = true;
    try {
      await deviceStore.refreshDevice(device.id);
      ui.toast("Device state refreshed.", "success", 2000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      refreshing = false;
    }
  }

  async function setRate(hz: number) {
    try {
      await deviceStore.setReportRate(device.id, hz);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }
</script>

<div class="settings-scroll">
  <div class="grid">
  <section class="card panel">
    <div class="panel-head">
      <h2 class="section-title">Device</h2>
      <button class="ghost" onclick={refresh} disabled={refreshing}>
        <Icon name="refresh" size={14} />
        {refreshing ? "Reading…" : "Refresh"}
      </button>
    </div>

    <dl class="facts">
      <div><dt>Name</dt><dd>{device.name}</dd></div>
      <div><dt>Connection</dt><dd>{connectionLabel(device.connection)}</dd></div>
      <div><dt>Vendor / product</dt><dd>{hex(device.vendorId)} : {hex(device.productId)}</dd></div>
      <div><dt>HID++ version</dt><dd>{device.protocolVersion || "—"}</dd></div>
      <div><dt>Serial</dt><dd>{device.serial ?? "—"}</dd></div>
      <div><dt>Status</dt><dd>{device.online ? "Connected" : "Offline"}</dd></div>
    </dl>
  </section>

  <section class="card panel">
    <h2 class="section-title">Power</h2>
    {#if device.battery}
      <div class="battery">
        <div class="gauge">
          <div
            class="level"
            class:low={device.battery.percentage <= 20}
            style="width: {device.battery.percentage}%"
          ></div>
        </div>
        <div class="battery-meta">
          <strong>{device.battery.percentage}%</strong>
          <span>{batteryLabel(device.battery)}</span>
          {#if device.battery.voltageMv}
            <span>{(device.battery.voltageMv / 1000).toFixed(2)} V</span>
          {/if}
          {#if device.battery.approximate}
            <span class="approx">Estimated from coarse levels</span>
          {/if}
        </div>
      </div>
    {:else}
      <p class="none">This device has no battery.</p>
    {/if}
  </section>

  <section class="card panel">
    <h2 class="section-title">Report rate</h2>
    {#if device.reportRate && rateOptions.length}
      <Segmented value={device.reportRate.currentHz} options={rateOptions} onselect={setRate} />
    {:else}
      <p class="none">Fixed report rate.</p>
    {/if}
  </section>

  <section class="card panel">
    <h2 class="section-title">Capabilities</h2>
    <ul class="caps">
      <li class:on={device.capabilities.dpi}>
        <Icon name={device.capabilities.dpi ? "check" : "close"} size={13} strokeWidth={2.2} />
        Adjustable DPI <code>0x2201</code>
      </li>
      <li class:on={device.capabilities.reportRate}>
        <Icon name={device.capabilities.reportRate ? "check" : "close"} size={13} strokeWidth={2.2} />
        Report rate <code>0x8060</code>
      </li>
      <li class:on={device.capabilities.battery}>
        <Icon name={device.capabilities.battery ? "check" : "close"} size={13} strokeWidth={2.2} />
        Battery <code>0x1000 / 0x1004</code>
      </li>
      <li class:on={device.capabilities.lighting}>
        <Icon name={device.capabilities.lighting ? "check" : "close"} size={13} strokeWidth={2.2} />
        Lighting <code>0x8070 / 0x8071</code>
      </li>
      <li class:on={device.capabilities.onboardMemory}>
        <Icon name={device.capabilities.onboardMemory ? "check" : "close"} size={13} strokeWidth={2.2} />
        Onboard profiles <code>0x8100</code>
      </li>
    </ul>
  </section>

  {#if device.firmware?.length}
    <section class="card panel">
      <h2 class="section-title">Firmware version</h2>
      <dl class="facts">
        {#each device.firmware as fw (fw.kind + fw.version)}
          <div>
            <dt>{fw.kind}{fw.active ? " · active" : ""}</dt>
            <dd>{fw.version}</dd>
          </div>
        {/each}
      </dl>
    </section>
  {/if}

  {#if device.capabilities.onboardMemory}
    <section class="card panel">
      <h2 class="section-title">Power management</h2>
      <p class="none">Stored in the device's onboard profile, so it applies with OpenGHub closed too.</p>
      <div class="field-row">
        <span class="field-label">Auto sleep after</span>
        <div class="select">
          <select value={ds.autoSleepMin} onchange={(e) => saveDs({ autoSleepMin: Number(e.currentTarget.value) })}>
            {#each SLEEP_OPTIONS as m (m)}
              <option value={m}>{m === 0 ? "Device default" : `${m} min`}</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </div>
      </div>
      <div class="field-row">
        <span class="field-label">Inactivity lighting off after</span>
        <div class="select">
          <select value={ds.inactivityLightingMin} onchange={(e) => saveDs({ inactivityLightingMin: Number(e.currentTarget.value) })}>
            {#each SLEEP_OPTIONS as m (m)}
              <option value={m}>{m === 0 ? "Device default" : `${m} min`}</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </div>
      </div>
    </section>
  {/if}

  {#if device.capabilities.battery && device.capabilities.lighting}
    <section class="card panel">
      <h2 class="section-title">Low battery mode</h2>
      <label class="check">
        <input type="checkbox" checked={ds.lowBatteryMode} onchange={(e) => saveDs({ lowBatteryMode: e.currentTarget.checked })} />
        <span>Dim the lighting when the battery runs low</span>
      </label>
      <div class="field-row">
        <span class="field-label">Below</span>
        <div class="select">
          <select value={ds.lowBatteryThreshold} onchange={(e) => saveDs({ lowBatteryThreshold: Number(e.currentTarget.value) })}>
            {#each [5, 10, 15, 20, 30] as t (t)}
              <option value={t}>{t}%</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </div>
      </div>
      <div class="field-row">
        <span class="field-label">Lighting brightness</span>
        <div class="select">
          <select value={ds.lowBatteryBrightness} onchange={(e) => saveDs({ lowBatteryBrightness: Number(e.currentTarget.value) })}>
            {#each [0, 10, 20, 30, 50] as b (b)}
              <option value={b}>{b === 0 ? "Off" : `${b}%`}</option>
            {/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </div>
      </div>
    </section>
  {/if}

  {#if device.kind === "mouse"}
    <section class="card panel">
      <h2 class="section-title">Button layout</h2>
      <label class="check">
        <input type="checkbox" checked={ds.leftHanded} onchange={(e) => saveDs({ leftHanded: e.currentTarget.checked })} />
        <span>Left-handed: swap primary and secondary click</span>
      </label>
    </section>
  {/if}

  {#if device.capabilities.onboardMemory}
    <section class="card panel">
      <h2 class="section-title">On-board memory mode</h2>
      <p class="none">
        With on-board memory mode on, the device runs the profile stored in its own memory:
        DPI ladder, report rate, lighting and button assignments are written into it, and they
        keep working on any computer with OpenGHub closed. Off, OpenGHub drives the device live
        (software effects, live assignments). Currently
        <strong>{device.onboardMode ? "on" : "off"}</strong>.
      </p>
      <label class="check">
        <input
          type="checkbox"
          checked={device.onboardMode === true}
          onchange={(e) => toggleOnboardMode(e.currentTarget.checked)}
        />
        <span>Store the active profile on the device (on-board memory mode)</span>
      </label>
    </section>
  {/if}

  {#if device.capabilities.wheel}
    <section class="card panel">
      <h2 class="section-title">Wheel center</h2>
      <p class="none">
        Use the calibrate function to reset the center position of your wheel if it is slightly
        off center.
      </p>
      <div class="wheel-actions">
        <button class="cal" onclick={() => { calStep = "start"; calError = null; }}>
          Calibrate wheel center position
        </button>
        {#if wheelSettings?.centerOffset}
          <button class="ghost" onclick={resetCenter}>
            Clear offset ({(wheelSettings.centerOffset / 32768 * (wheelSettings.rangeDeg / 2)).toFixed(1)}°)
          </button>
        {/if}
      </div>
    </section>

    <section class="card panel">
      <h2 class="section-title">Force feedback driver</h2>
      <p class="none">
        OpenGHub provides force feedback to games itself: a virtual wheel on <code>uinput</code>
        receives the effects games upload and drives the real wheel. No kernel module needed.
        Status: <strong>{device.wheel?.driverRunning ? "running" : "stopped"}</strong>.
      </p>
      <label class="check">
        <input
          type="checkbox"
          checked={configStore.settings.wheelDriver ?? true}
          onchange={(e) => toggleDriver(e.currentTarget.checked)}
        />
        <span>Enable the force-feedback driver</span>
      </label>
    </section>
  {/if}

  <section class="card panel wide">
    <div class="panel-head">
      <h2 class="section-title">HID++ features</h2>
      <button class="ghost" onclick={loadFeatures} disabled={loadingFeatures}>
        <Icon name="chip" size={14} />
        {loadingFeatures ? "Enumerating…" : "Enumerate"}
      </button>
    </div>

    <p class="none">
      Walks feature <code>0x0001</code> and lists everything the device exposes — useful when
      adding support for hardware that is not in the registry yet.
    </p>

    {#if featureError}
      <p class="err"><Icon name="alert" size={13} /> {featureError}</p>
    {/if}

    {#if features.length}
      <table>
        <thead>
          <tr><th>ID</th><th>Feature</th><th>Index</th><th>Flags</th></tr>
        </thead>
        <tbody>
          {#each features as feature (feature.id + feature.index)}
            <tr>
              <td><code>{feature.id}</code></td>
              <td>{feature.name}</td>
              <td><code>{feature.index}</code></td>
              <td class="flags">
                {#if feature.obsolete}<span>obsolete</span>{/if}
                {#if feature.hidden}<span>hidden</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
</div>
</div>

{#if calStep !== "idle"}
  <div class="scrim" role="presentation" onclick={(e) => e.target === e.currentTarget && (calStep = "idle")}>
    <div class="dialog" role="dialog" aria-modal="true">
      {#if calStep === "start"}
        <Icon name="alert" size={30} strokeWidth={1.5} />
        <h3>Set current wheel position as center?</h3>
        <p>
          Hold the wheel in the desired center position, and press Calibrate to set it as the
          new center position of the wheel.
          <br /><br />
          NOTE - The maximum offset that you can apply to the center position is ±{CAL_MAX_DEG}
          degrees.
        </p>
        {#if calError}
          <p class="cal-error">{calError}</p>
        {/if}
        <div class="dialog-actions">
          <button class="ghost" onclick={() => (calStep = "idle")}>Cancel</button>
          <button class="cal" onclick={calibrate} disabled={calibrating}>{calibrating ? "Calibrating…" : "Calibrate"}</button>
        </div>
      {:else}
        <Icon name="check" size={30} strokeWidth={1.8} />
        <h3>Your wheel's center position is now set.</h3>
        <p>Finish or Recalibrate the wheel's center position.</p>
        <div class="dialog-actions">
          <button class="ghost" onclick={() => (calStep = "start")}>Recalibrate</button>
          <button class="cal" onclick={() => (calStep = "idle")}>Finish</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .field-label {
    font-size: 12px;
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
    height: 32px;
    padding: 0 30px 0 12px;
    border: none;
    border-radius: 6px;
    background: var(--surface-3);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    color: var(--text);
    cursor: pointer;
  }

  .select :global(svg) {
    position: absolute;
    right: 10px;
    pointer-events: none;
  }

  .select option {
    background: #161616;
  }

  .wheel-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .cal {
    padding: 9px 16px;
    border-radius: 4px;
    background: var(--cyan);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #fff;
  }

  .cal:disabled {
    opacity: 0.6;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .check input {
    width: 14px;
    height: 14px;
    accent-color: var(--accent);
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.65);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    width: 420px;
    padding: 28px 28px 22px;
    border-radius: 8px;
    background: var(--surface);
    text-align: center;
    color: var(--text);
  }

  .dialog h3 {
    font-size: 16px;
    font-weight: 700;
  }

  .dialog p {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-dim);
  }

  .cal-error {
    color: var(--warning) !important;
  }

  .dialog-actions {
    display: flex;
    gap: 10px;
    margin-top: 6px;
  }

  .settings-scroll {
    flex: 1;
    width: 100%;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    align-items: start;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px 22px 22px;
  }

  .wide {
    grid-column: span 2;
  }

  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-dim);
  }

  .ghost:hover:not(:disabled) {
    border-color: var(--line-strong);
    color: var(--text);
  }

  .ghost:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 12px;
  }

  .facts div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  dt {
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  dd {
    font-size: 13.5px;
  }

  .battery {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .gauge {
    height: 8px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    overflow: hidden;
  }

  .level {
    height: 100%;
    border-radius: var(--radius-pill);
    background: linear-gradient(90deg, var(--cyan), var(--success));
    transition: width 400ms var(--ease);
  }

  .level.low {
    background: linear-gradient(90deg, var(--warning), var(--danger));
  }

  .battery-meta {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--text-dim);
  }

  .battery-meta strong {
    font-family: var(--font);
    font-size: 22px;
    font-weight: 600;
    color: var(--text);
  }

  .approx {
    color: var(--text-dimmer);
    font-style: italic;
  }

  .caps {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .caps li {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    color: var(--text-dimmer);
  }

  .caps li.on {
    color: var(--text);
  }

  code {
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    color: var(--text-dim);
  }

  .none {
    font-size: 12.5px;
    color: var(--text-dim);
    line-height: 1.55;
  }

  .err {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--danger);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }

  th {
    text-align: left;
    padding: 8px 10px;
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-dimmer);
    border-bottom: 1px solid var(--line);
  }

  td {
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
  }

  .flags span {
    margin-right: 6px;
    padding: 1px 6px;
    border-radius: var(--radius-pill);
    background: var(--surface-3);
    font-size: 10px;
    color: var(--text-dim);
  }

  @media (max-width: 900px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .wide {
      grid-column: span 1;
    }
  }
</style>
