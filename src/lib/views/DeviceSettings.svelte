<script lang="ts">
  /** Per-device settings, live state and the HID++ feature dump. */
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import Segmented from "$lib/components/Segmented.svelte";
  import { batteryLabel, connectionLabel } from "$lib/device-ui";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Device, FeatureInfo } from "$lib/types";

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

<style>
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
