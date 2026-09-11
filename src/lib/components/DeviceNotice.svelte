<script lang="ts">
  /**
   * Explains an empty device list. "No Logitech hardware" and "hardware is
   * present but this user cannot open its hidraw node" need entirely different
   * fixes, so each gets its own message and call to action.
   */
  import Icon from "$lib/components/Icon.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { EmptyReason } from "$lib/types";

  interface Props {
    reason: EmptyReason | null;
    demo: boolean;
    onrescan: () => void;
  }

  let { reason, demo, onrescan }: Props = $props();

  // The `70-` prefix is load-bearing: systemd's 73-seat-late.rules is what turns
  // the uaccess tag into an ACL, so a higher-numbered file is tagged too late.
  const UDEV_RULE = `sudo cp packaging/70-openghub.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --action=add --subsystem-match=hidraw`;

  const permission = $derived(
    reason && typeof reason === "object" && "permissionDenied" in reason
      ? reason.permissionDenied
      : null,
  );

  const hidUnavailable = $derived(
    reason && typeof reason === "object" && "hidUnavailable" in reason
      ? reason.hidUnavailable
      : null,
  );

  async function copyCommands() {
    try {
      await navigator.clipboard.writeText(UDEV_RULE);
      ui.toast("Commands copied to the clipboard.", "success");
    } catch {
      ui.toast("Clipboard unavailable — select the text instead.", "error");
    }
  }
</script>

{#if hidUnavailable}
  <div class="notice error">
    <Icon name="alert" size={16} />
    <div class="body">
      <strong>HID access failed.</strong>
      {hidUnavailable.message}
    </div>
  </div>
{:else if permission}
  <div class="notice warn">
    <Icon name="alert" size={16} />
    <div class="body">
      <strong>
        {permission.devices.length} Logitech device{permission.devices.length === 1 ? "" : "s"}
        found, but OpenGHub cannot open
        {permission.devices.length === 1 ? "it" : "them"}.
      </strong>
      <p class="devices">{permission.devices.join(", ")}</p>
      <p>
        The <code>hidraw</code> nodes are root-only on this system. Install the udev rule, then
        unplug and reconnect the device (or its receiver):
      </p>
      <pre>{UDEV_RULE}</pre>
      <div class="actions">
        <button class="primary" onclick={copyCommands}>Copy commands</button>
        <button onclick={onrescan}>Rescan</button>
      </div>
      <p class="aside">Showing demo devices until then.</p>
    </div>
  </div>
{:else if reason === "noResponse"}
  <div class="notice warn">
    <Icon name="alert" size={16} />
    <div class="body">
      <strong>Devices found, but none answered.</strong>
      A wireless device may be switched off or asleep — wake it and rescan. Devices that do not
      speak HID++ 2.0 are not supported.
      <div class="actions"><button onclick={onrescan}>Rescan</button></div>
    </div>
  </div>
{:else if reason === "noHardware"}
  <div class="notice">
    <Icon name="info" size={16} />
    <div class="body">
      <strong>No Logitech device connected.</strong>
      Showing demo devices so you can explore the interface.
    </div>
    <button class="inline" onclick={onrescan}>Rescan</button>
  </div>
{:else if demo}
  <div class="notice">
    <Icon name="info" size={16} />
    <div class="body"><strong>Demo devices.</strong> Nothing here is talking to hardware.</div>
    <button class="inline" onclick={onrescan}>Rescan</button>
  </div>
{/if}

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 14px;
    padding: 13px 15px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-sm);
    background: var(--surface);
    font-size: 12.5px;
    color: var(--text-dim);
    line-height: 1.55;
  }

  .notice.warn {
    border-left-color: var(--warning);
  }

  .notice.error {
    border-left-color: var(--danger);
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  strong {
    color: var(--text);
    font-weight: 600;
  }

  .devices {
    margin-top: 3px;
    color: var(--text);
  }

  p {
    margin-top: 6px;
  }

  .aside {
    color: var(--text-dimmer);
    font-size: 11.5px;
  }

  code {
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    color: var(--text);
  }

  pre {
    margin-top: 9px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: #101010;
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
    font-size: 11.5px;
    line-height: 1.65;
    color: var(--text-dim);
    overflow-x: auto;
    user-select: text;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }

  button {
    padding: 7px 14px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-dim);
  }

  button:hover {
    color: var(--text);
    background: var(--surface-2);
  }

  button.primary {
    border-color: transparent;
    background: var(--accent);
    color: #fff;
  }

  button.primary:hover {
    background: var(--accent-hover);
  }

  button.inline {
    flex: none;
    margin-left: auto;
  }
</style>
