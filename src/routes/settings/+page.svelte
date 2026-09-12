<script lang="ts">
  /** Application settings and Linux-specific setup notes. */
  import { onMount } from "svelte";
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import Toggle from "$lib/components/Toggle.svelte";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let configPath = $state("");
  let pollSeconds = $state(60);
  let ghubCache = $state<import("$lib/types").GhubCacheInfo | null>(null);
  let importing = $state(false);
  let fetching = $state(false);
  let lastImport = $state<import("$lib/types").ImportReport | null>(null);

  onMount(async () => {
    configPath = await api.getConfigPath().catch(() => "unavailable");
    pollSeconds = configStore.settings.batteryPollSeconds;
    ghubCache = await api.getGhubCacheInfo().catch(() => null);
  });

  /** Lets the user point at their own G HUB ProgramData folder. */
  async function importGhub() {
    let path: string | null = null;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ directory: true, title: "Select the LGHUB folder from ProgramData" });
      path = typeof picked === "string" ? picked : null;
    } catch {
      ui.toast("The folder picker needs the desktop app.", "error");
      return;
    }
    if (!path) return;
    importing = true;
    try {
      lastImport = await api.importGhubProgramData(path);
      ghubCache = await api.getGhubCacheInfo().catch(() => null);
      await artwork.load();
      ui.toast(`Imported ${lastImport.imported.length} device(s) from G HUB build ${lastImport.buildId}.`, "success", 5000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 8000);
    } finally {
      importing = false;
    }
  }

  /** Downloads depots for every connected device that has no artwork yet. */
  async function fetchAll() {
    fetching = true;
    let done = 0;
    try {
      for (const d of deviceStore.devices) {
        if (d.demo || !d.online) continue;
        try {
          await api.fetchDeviceArtwork(d.id);
          done++;
        } catch (e) {
          ui.toast(`${d.name}: ${api.errorMessage(e)}`, "error", 6000);
        }
      }
      await artwork.load();
      if (done) ui.toast(`Fetched artwork for ${done} device(s) from Logitech.`, "success", 4000);
    } finally {
      fetching = false;
    }
  }

  async function save(patch: Partial<typeof configStore.settings>) {
    try {
      await configStore.saveSettings({ ...configStore.settings, ...patch });
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  // The filename must sort below systemd's 73-seat-late.rules, which is what
  // actually applies the ACL for a uaccess-tagged device.
  const udevRule = `# /etc/udev/rules.d/70-openghub.rules
KERNEL=="hidraw*", ATTRS{idVendor}=="046d", TAG+="uaccess"`;

  async function copyRule() {
    try {
      await navigator.clipboard.writeText(udevRule);
      ui.toast("udev rule copied to the clipboard.", "success");
    } catch {
      ui.toast("Clipboard is unavailable — select the text instead.", "error");
    }
  }
</script>

<div class="page">
  <header>
    <h1>Settings</h1>
    <p>Application-wide options. Per-device settings live on each device's own page.</p>
  </header>

  <section class="card panel">
    <h2 class="section-title">General</h2>

    <Toggle
      checked={configStore.settings.startMinimised}
      label="Start minimised to tray"
      description="Launch into the system tray instead of opening the window."
      onchange={(v) => save({ startMinimised: v })}
    />

    <Toggle
      checked={configStore.settings.showBatteryNotifications}
      label="Battery notifications"
      description="Warn when a wireless device drops below 20 %."
      onchange={(v) => save({ showBatteryNotifications: v })}
    />

    <Toggle
      checked={configStore.settings.autoSwitchProfiles}
      label="Switch profiles with games"
      description="Activate a profile bound to a game when it starts, and return to Desktop when it stops."
      onchange={(v) => save({ autoSwitchProfiles: v })}
    />

    <Toggle
      checked={configStore.settings.illuminationFollowsProfile}
      label="Apply lighting on profile switch"
      description="Push each device's saved lighting when the active profile changes."
      onchange={(v) => save({ illuminationFollowsProfile: v })}
    />
  </section>

  <section class="card panel">
    <h2 class="section-title">Battery polling</h2>
    <p class="lede">
      HID++ round trips wake sleeping wireless devices, so OpenGHub polls slowly. The backend
      clamps this to a minimum of 15 seconds.
    </p>
    <Slider
      bind:value={pollSeconds}
      min={15}
      max={600}
      step={15}
      label="Interval"
      suffix=" s"
      onchange={(v) => save({ batteryPollSeconds: v })}
    />
    <button class="ghost" onclick={() => api.readBatteries().catch(() => {})}>
      <Icon name="battery" size={14} />
      Read batteries now
    </button>
  </section>

  <section class="card panel">
    <h2 class="section-title">Linux device access</h2>
    <p class="lede">
      HID++ needs read/write access to the device's <code>hidraw</code> node. Distributions do
      not grant that to desktop users by default, which is why devices can show as missing even
      when they are plugged in.
    </p>
    <pre>{udevRule}</pre>
    <div class="actions">
      <button class="ghost" onclick={copyRule}><Icon name="pencil" size={14} /> Copy rule</button>
      <button class="ghost" onclick={() => deviceStore.load(true)}>
        <Icon name="refresh" size={14} /> Rescan devices
      </button>
    </div>
    <p class="lede small">
      Save it, then run <code>sudo udevadm control --reload-rules</code> followed by
      <code>sudo udevadm trigger --action=add --subsystem-match=hidraw</code>, and reconnect the
      device. The <code>70-</code> prefix matters: systemd's <code>73-seat-late.rules</code> is
      what turns the <code>uaccess</code> tag into an ACL, so a higher-numbered file is tagged
      too late to have any effect.
    </p>
  </section>

  <section class="card panel">
    <h2 class="section-title">G HUB data</h2>
    <p class="lede">
      G HUB stores each device's render, thumbnail and the exact positions of its lighting
      zones and buttons in a per-device <em>depot</em>. Point OpenGHub at a G HUB installation's
      <code>C:\ProgramData\LGHUB</code> folder and it imports what is there — and caches the
      depot index, so any device you connect later is fetched straight from Logitech's CDN the
      same way G HUB does it.
    </p>
    {#if ghubCache}
      <dl class="facts">
        <div><dt>Build</dt><dd>{ghubCache.buildId} · {ghubCache.version}</dd></div>
        <div><dt>Depots indexed</dt><dd>{ghubCache.depots}</dd></div>
        <div><dt>Devices known</dt><dd>{ghubCache.deviceDefinitions}</dd></div>
      </dl>
    {:else}
      <p class="lede small">No G HUB data imported yet — automatic fetching is unavailable until then.</p>
    {/if}
    <div class="actions">
      <button class="ghost" onclick={importGhub} disabled={importing}>
        <Icon name="chip" size={14} />
        {importing ? "Importing…" : "Import from G HUB folder"}
      </button>
      <button class="ghost" onclick={fetchAll} disabled={fetching || !ghubCache}>
        <Icon name="refresh" size={14} />
        {fetching ? "Fetching…" : "Fetch artwork for connected devices"}
      </button>
    </div>
    {#if lastImport}
      <p class="lede small">
        Imported: {lastImport.imported.map((d) => d.displayName).join(", ") || "nothing new"}.
      </p>
    {/if}
    <Toggle
      checked={configStore.settings.autoFetchArtwork}
      label="Fetch artwork automatically"
      description="Download a device's depot from Logitech the first time it is seen, as G HUB does."
      onchange={(v) => save({ autoFetchArtwork: v })}
    />
  </section>

  <section class="card panel">
    <h2 class="section-title">Device artwork</h2>
    <p class="lede">
      OpenGHub ships drawings rather than photos — Logitech's product renders are their
      copyright, so none are bundled. Drop an image named after the product id into the
      folder below and it replaces the drawing on that device's card.
    </p>
    <pre>{artwork.dir || "…"}/&lt;product-id&gt;.png     e.g. c08d.png</pre>
    <p class="lede">
      <code>scripts/fetch-artwork.sh</code> fills it in for the devices attached to this
      machine. Currently {Object.keys(artwork.images).length} image(s) loaded.
    </p>
    <button class="ghost" onclick={() => artwork.load()}>
      <Icon name="refresh" size={14} />
      Rescan artwork
    </button>
  </section>

  <section class="card panel">
    <h2 class="section-title">About</h2>
    <dl class="facts">
      <div><dt>Config file</dt><dd><code>{configPath}</code></dd></div>
      <div><dt>Devices detected</dt><dd>{deviceStore.devices.length}</dd></div>
      <div><dt>Mode</dt><dd>{deviceStore.demo ? "Demo devices" : "Live hardware"}</dd></div>
      <div><dt>Runtime</dt><dd>{api.isTauri ? "Tauri" : "Browser (mock backend)"}</dd></div>
    </dl>
  </section>
</div>

<style>
  .page {
    max-width: 880px;
    margin: 0 auto;
    padding: 0 var(--content-pad) 50px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  header {
    margin-bottom: 8px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
  }

  header p {
    margin-top: 6px;
    font-size: 13px;
    color: var(--text-dim);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 22px 22px;
  }

  .lede {
    font-size: 12.5px;
    color: var(--text-dim);
    line-height: 1.6;
    max-width: 70ch;
  }

  .lede.small {
    font-size: 11.5px;
    color: var(--text-dimmer);
  }

  pre {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: #101010;
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    line-height: 1.6;
    color: var(--text-dim);
    overflow-x: auto;
    user-select: text;
  }

  code {
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    color: var(--text);
  }

  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 8px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    color: var(--text-dim);
    align-self: flex-start;
  }

  .ghost:hover {
    border-color: var(--line-strong);
    color: var(--text);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 14px;
  }

  .facts div {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  dt {
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  dd {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
</style>
