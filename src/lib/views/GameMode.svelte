<script lang="ts">
  /**
   * Game Mode (G HUB's keyboard tab): keys to disable while gaming. The
   * Windows and Menu keys are always part of it; click keys to add or remove
   * more. The list is per profile, like every other device setting.
   */
  import ProfileLock from "$lib/components/ProfileLock.svelte";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import * as api from "$lib/api";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import KeyboardMap from "$lib/components/KeyboardMap.svelte";
  import Toggle from "$lib/components/Toggle.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { keyMapFor, type Key } from "$lib/keyboards";
  import type { Device } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  /** Always disabled in Game Mode: both Windows keys and Menu. */
  const FIXED = [0xe3, 0xe7, 0x65];

  const keyMap = $derived(keyMapFor(device));
  const on = $derived((configStore.settings.gameModeDevices ?? []).includes(device.id));
  const disabled = $derived(configStore.deviceProfile(device.id).gameModeKeys ?? FIXED);
  const marked = $derived(
    new Set((keyMap?.keys ?? []).filter((k) => k.usage !== undefined && disabled.includes(k.usage)).map((k) => k.id)),
  );

  async function save(keys: number[]) {
    try {
      await configStore.saveDeviceProfile(device.id, { ...configStore.deviceProfile(device.id), gameModeKeys: keys });
      // Re-apply with the new list when Game Mode is on.
      if (on) configStore.apply(await api.setGameMode(device.id, true));
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  function pick(keys: Key[]) {
    const usages = keys.flatMap((k) => (k.usage !== undefined && !FIXED.includes(k.usage) ? [k.usage] : []));
    if (!usages.length) return;
    // A single click toggles; a box adds, unless every key in it was already off.
    const allIn = usages.every((u) => disabled.includes(u));
    const next = allIn ? disabled.filter((u) => !usages.includes(u)) : [...new Set([...disabled, ...usages])];
    void save(next);
  }

  async function toggle(value: boolean) {
    try {
      configStore.apply(await api.setGameMode(device.id, value));
      ui.toast(value ? "Game Mode on." : "Game Mode off.", "success", 2000);
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }
</script>

<DeviceWorkspace title="GAME MODE">
  {#snippet panel()}
    <ProfileLock deviceId={device.id} feature="gamemode" />
    <div class="row">
      <span class="label">Game Mode</span>
      <Toggle checked={on} onchange={toggle} />
    </div>
    <p class="hint">
      Disables the keys shown in white so they cannot be hit by accident. The Windows and Menu keys are always
      included. Click a key, or drag over several, to add or remove it.
    </p>
    <p class="hint">Keys come back on when Game Mode is switched off or OpenGHub quits.</p>
    <button class="wide" onclick={() => save(FIXED)}>Restore default settings</button>
  {/snippet}

  {#snippet stage()}
    {#if keyMap}
      <div class="map">
        <KeyboardMap
          keys={keyMap.keys}
          size={keyMap.size}
          image={keyMap.image}
          {marked}
          pickable={(k) => k.usage !== undefined}
          onpick={pick}
        />
      </div>
    {:else}
      <p class="hint">
        The key map comes from G HUB's data for this keyboard. Import it under
        <a href="/settings">Settings → G HUB data</a>; until then the Windows and Menu keys are what Game
        Mode disables.
      </p>
    {/if}
  {/snippet}
</DeviceWorkspace>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .label {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .hint {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-dimmer);
  }

  .wide {
    height: 34px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .map {
    width: min(100%, 980px);
    margin: auto;
    padding: 0 20px;
  }
</style>
