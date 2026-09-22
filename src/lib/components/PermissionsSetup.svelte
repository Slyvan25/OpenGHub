<script lang="ts">
  /**
   * First-run device access. Desktop users cannot open Logitech hidraw nodes
   * (or /dev/uinput) until OpenGHub's udev rule is installed, so on launch
   * the app checks for it and offers to install it. The install runs through
   * polkit (`pkexec`): the desktop shows its own password dialog and the app
   * never handles the password. Shown until the rule is current, unless the
   * user ticks "don't ask again"; the Devices page keeps an Install button.
   */
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { UdevRuleStatus } from "$lib/types";
  import { onMount } from "svelte";

  let status = $state<UdevRuleStatus | null>(null);
  let open = $state(false);
  let installing = $state(false);
  let dontAsk = $state(false);

  onMount(async () => {
    try {
      // The settings carry "don't ask again"; read them directly so this does
      // not race the layout's own config load.
      const [s, cfg] = await Promise.all([api.getUdevRuleStatus(), api.getConfig()]);
      status = s;
      open = !s.current && !cfg.settings.udevSetupDismissed;
    } catch {
      /* not fatal: the Devices page still explains the manual route */
    }
  });

  async function install() {
    installing = true;
    try {
      status = await api.installUdevRule();
      open = false;
      ui.toast("Device access set up. Wireless devices may need their receiver unplugged and reconnected once.", "success", 7000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error", 7000);
    } finally {
      installing = false;
    }
  }

  async function later() {
    open = false;
    if (dontAsk) {
      try {
        await configStore.saveSettings({ ...configStore.settings, udevSetupDismissed: true });
      } catch (e) {
        ui.toast(api.errorMessage(e), "error");
      }
    }
  }
</script>

{#if open && status}
  <div class="scrim" role="presentation">
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="perm-title">
      <span class="badge"><Icon name="lock" size={26} strokeWidth={1.6} /></span>
      <h2 id="perm-title">{status.installed ? "Update device access" : "Set up device access"}</h2>
      <p>
        {#if status.installed}
          The udev rule OpenGHub installed earlier is out of date — this version also needs
          <code>/dev/uinput</code> for the wheel driver and virtual keyboard.
        {:else}
          Linux gives only root access to Logitech devices. OpenGHub installs a small udev rule
          (<code>{status.path}</code>) that hands your devices and <code>/dev/uinput</code> to
          whoever is logged in at the desktop.
        {/if}
        {#if status.stale}
          The old <code>99-openghub.rules</code> is removed at the same time.
        {/if}
      </p>
      {#if status.canInstall}
        <p class="sub">Your desktop will ask for your password (administrator rights are needed once).</p>
        <label class="check">
          <input type="checkbox" bind:checked={dontAsk} />
          <span>Don't ask again</span>
        </label>
        <div class="actions">
          <button class="secondary" onclick={later} disabled={installing}>Not now</button>
          <button class="primary" onclick={install} disabled={installing}>
            {installing ? "Waiting for authentication…" : "Install"}
          </button>
        </div>
      {:else}
        <p class="sub">
          <code>pkexec</code> (polkit) is not available here, so run this in a terminal:
        </p>
        <pre>sudo cp packaging/70-openghub.rules {status.path}
sudo udevadm control --reload-rules
sudo udevadm trigger --action=add --subsystem-match=hidraw</pre>
        <div class="actions">
          <button class="primary" onclick={later}>OK</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.7);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    width: 460px;
    padding: 30px 30px 24px;
    border-radius: var(--radius-lg);
    background: var(--surface);
    text-align: center;
    color: var(--text);
  }

  .badge {
    display: grid;
    place-items: center;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }

  h2 {
    margin: 4px 0 0;
    font-size: 18px;
  }

  p {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .sub {
    font-size: 12px;
  }

  code {
    font-size: 12px;
    color: var(--text);
  }

  pre {
    width: 100%;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: #000;
    font-size: 11.5px;
    text-align: left;
    user-select: text;
    overflow-x: auto;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-dim);
    cursor: pointer;
  }

  .check input {
    accent-color: var(--accent);
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-top: 6px;
  }

  .actions button {
    padding: 9px 18px;
    border-radius: var(--radius-sm);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
  }

  .primary {
    background: var(--accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .secondary {
    background: var(--surface-3);
  }

  button:disabled {
    opacity: 0.6;
  }
</style>
