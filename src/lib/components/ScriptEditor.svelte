<script lang="ts">
  /**
   * G HUB's scripting window for one profile: a Lua editor above a console.
   * "Save & Run" stores the script and restarts it when the profile is active;
   * the console tails `OutputLogMessage` and errors while the window is open.
   */
  import * as api from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Profile } from "$lib/types";
  import { onMount } from "svelte";

  interface Props {
    profile: Profile;
    active: boolean;
    onsaved: (script: string | null) => void;
    onclose: () => void;
  }

  let { profile, active, onsaved, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  const TEMPLATE = `-- ${profile.name}
-- OnEvent runs for PROFILE_ACTIVATED, MOUSE_BUTTON_PRESSED / _RELEASED
-- (arg = button number) and PROFILE_DEACTIVATED.
function OnEvent(event, arg, family)
  OutputLogMessage("event = %s, arg = %d\\n", event, arg)
  if event == "MOUSE_BUTTON_PRESSED" and arg == 4 then
    PressAndReleaseKey("f5")
  end
end
`;

  // svelte-ignore state_referenced_locally
  let source = $state(profile.script ?? TEMPLATE);
  // svelte-ignore state_referenced_locally
  let saved = $state(profile.script ?? "");
  let log = $state<string[]>([]);
  let running = $state(false);
  let onboardDevices = $state<string[]>([]);
  let busy = $state(false);
  let consoleEl = $state<HTMLPreElement | null>(null);
  let editorEl = $state<HTMLTextAreaElement | null>(null);

  const dirty = $derived(source !== saved && !(saved === "" && source === TEMPLATE));
  const lines = $derived(source.split("\n").length);

  async function refresh() {
    try {
      const [l, s] = await Promise.all([api.getScriptLog(), api.getScriptStatus()]);
      const grew = l.length !== log.length;
      log = l;
      running = s.running && s.profileId === profile.id;
      onboardDevices = s.onboardDevices ?? [];
      if (grew && consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
    } catch {
      /* backend gone; keep the last view */
    }
  }

  onMount(() => {
    refresh();
    const timer = setInterval(refresh, 500);
    return () => clearInterval(timer);
  });

  async function save(run: boolean) {
    busy = true;
    try {
      const script = source.trim() ? source : null;
      await api.setProfileScript(profile.id, script);
      saved = script ?? "";
      onsaved(script);
      if (run && !active) {
        ui.toast("Saved. The script runs while this profile is active.", "info", 4000);
      } else if (run) {
        ui.toast("Script saved and running.", "success", 2500);
      } else {
        ui.toast("Script saved.", "success", 2000);
      }
      await refresh();
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      busy = false;
    }
  }

  async function stop() {
    busy = true;
    try {
      await api.setProfileScript(profile.id, null);
      saved = "";
      onsaved(null);
      await refresh();
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    } finally {
      busy = false;
    }
  }

  async function clearConsole() {
    try {
      await api.clearScriptLog();
      log = [];
    } catch (e) {
      ui.toast(api.errorMessage(e), "error");
    }
  }

  async function importFile() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ title: "Import Lua script", multiple: false, filters: [{ name: "Lua", extensions: ["lua"] }] });
      const path = typeof picked === "string" ? picked : null;
      if (!path) return;
      source = await api.readTextFile(path);
    } catch (e) {
      ui.toast(api.isTauri ? api.errorMessage(e) : "Importing needs the desktop app.", "error");
    }
  }

  async function exportFile() {
    try {
      const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
      const picked = await saveDialog({
        title: "Export Lua script",
        defaultPath: `${profile.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}.lua`,
        filters: [{ name: "Lua", extensions: ["lua"] }],
      });
      const path = typeof picked === "string" ? picked : null;
      if (!path) return;
      await api.writeTextFile(path, source);
      ui.toast("Script exported.", "success", 2000);
    } catch (e) {
      ui.toast(api.isTauri ? api.errorMessage(e) : "Exporting needs the desktop app.", "error");
    }
  }

  /** Tab inserts two spaces; Ctrl+S saves and runs, as in G HUB. */
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Tab" && editorEl) {
      e.preventDefault();
      const { selectionStart: s, selectionEnd: end } = editorEl;
      source = source.slice(0, s) + "  " + source.slice(end);
      queueMicrotask(() => editorEl?.setSelectionRange(s + 2, s + 2));
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save(true);
    } else if (e.key === "Escape") {
      onclose();
    }
  }
</script>

<div class="scrim" role="presentation">
  <div class="window" role="dialog" aria-modal="true" aria-label="Script editor">
    <header class="bar">
      <div class="title">
        <span class="eyebrow">Active Lua script</span>
        <strong>{profile.id === "default" ? "Desktop" : profile.name}</strong>
        <span class="status" class:on={running}>
          <i></i>{running ? "Running" : active ? "Stopped" : "Profile not active"}
        </span>
      </div>
      <nav class="menu" aria-label="Script">
        <button onclick={() => save(true)} disabled={busy}><Icon name="play" size={14} />Save & Run</button>
        <button onclick={() => save(false)} disabled={busy || !dirty}>Save</button>
        <button onclick={importFile} disabled={busy}>Import…</button>
        <button onclick={exportFile} disabled={busy}>Export…</button>
        <button onclick={stop} disabled={busy || !saved} title="Removes the script from the profile">Stop</button>
      </nav>
      <button class="close" aria-label="Close" onclick={onclose}><Icon name="close" size={18} /></button>
    </header>

    <div class="editor">
      <div class="gutter" aria-hidden="true">
        {#each { length: lines } as _, i (i)}<span>{i + 1}</span>{/each}
      </div>
      <textarea
        bind:this={editorEl}
        bind:value={source}
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        wrap="off"
        {onkeydown}
      ></textarea>
    </div>

    <section class="console">
      <header>
        <span>Console</span>
        {#if onboardDevices.length}
          <span class="note" title="Turn off on-board memory mode on the Devices page to script these buttons">
            <Icon name="onboard" size={13} />
            {onboardDevices.join(", ")} in on-board memory mode — buttons are not forwarded to scripts
          </span>
        {/if}
        <button onclick={clearConsole}>Clear</button>
      </header>
      <pre bind:this={consoleEl}>{#each log as line, i (i)}{line}
{:else}<span class="hint">OutputLogMessage() output and script errors appear here.</span>{/each}</pre>
    </section>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.7);
  }

  .window {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) 200px;
    width: min(1100px, calc(100vw - 48px));
    height: min(760px, calc(100vh - 48px));
    border-radius: var(--radius-lg);
    background: var(--surface);
    overflow: hidden;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 14px 18px 14px 22px;
    border-bottom: 1px solid var(--line);
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }

  .eyebrow {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .title strong {
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .status i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-dimmer);
  }

  .status.on {
    color: var(--success);
  }

  .status.on i {
    background: var(--success);
    box-shadow: 0 0 6px var(--success);
  }

  .menu {
    display: flex;
    gap: 4px;
    margin-left: auto;
  }

  .menu button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: var(--radius-sm);
    font-family: var(--font);
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
  }

  .menu button:first-child {
    background: var(--accent);
  }

  .menu button:first-child:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .menu button:not(:first-child):hover:not(:disabled) {
    background: var(--surface-3);
  }

  .menu button:disabled {
    opacity: 0.45;
  }

  .close {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    color: var(--text-dim);
  }

  .close:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .editor {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    min-height: 0;
    background: #0d0e10;
    font-family: "JetBrains Mono", "Fira Code", "DejaVu Sans Mono", monospace;
    font-size: 13px;
    line-height: 20px;
  }

  .gutter {
    display: flex;
    flex-direction: column;
    padding: 12px 10px 12px 16px;
    border-right: 1px solid var(--line);
    color: var(--text-dimmer);
    text-align: right;
    overflow: hidden;
    user-select: none;
  }

  textarea {
    padding: 12px 16px;
    border: 0;
    background: transparent;
    font: inherit;
    color: var(--text);
    resize: none;
    outline: none;
    tab-size: 2;
    user-select: text;
  }

  .console {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    border-top: 1px solid var(--line);
  }

  .console header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 16px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  .note {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    margin-right: 16px;
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    color: var(--warning);
  }

  .console header button {
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    color: var(--text-dim);
  }

  .console header button:hover {
    color: var(--text);
  }

  .console pre {
    margin: 0;
    padding: 6px 16px 12px;
    overflow: auto;
    font-family: "JetBrains Mono", "Fira Code", "DejaVu Sans Mono", monospace;
    font-size: 12px;
    line-height: 18px;
    color: var(--text);
    user-select: text;
  }

  .hint {
    color: var(--text-dimmer);
  }
</style>
