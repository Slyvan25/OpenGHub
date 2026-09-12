<script lang="ts">
  /**
   * Assignments — drag a command onto a control, exactly like G HUB.
   *
   * Bindings are stored in the active profile. Writing them into the device's
   * own remap table needs HID++ feature `0x1b04` (Special Keys & Buttons), which
   * this build does not implement yet, so the banner says so rather than
   * pretending the change reached the hardware.
   */
  import { untrack } from "svelte";
  import * as api from "$lib/api";
  import MacroRecorder from "$lib/components/MacroRecorder.svelte";
  import { encodedSize, type MacroDef } from "$lib/macros";
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { artwork } from "$lib/stores/artwork.svelte";
  import { controlSpots, type ControlSpot } from "$lib/zones";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { ApplicationCommands, Assignment, Device } from "$lib/types";

  interface Props {
    device: Device;
  }
  let { device }: Props = $props();

  interface Control {
    id: string;
    label: string;
    /** Default command shown when nothing is bound. */
    fallback: string;
  }

  const mouseControls: Control[] = [
    { id: "button-1", label: "Left click", fallback: "Left Click" },
    { id: "button-2", label: "Right click", fallback: "Right Click" },
    { id: "button-3", label: "Middle click", fallback: "Middle Click" },
    { id: "button-4", label: "Thumb back", fallback: "Back" },
    { id: "button-5", label: "Thumb forward", fallback: "Forward" },
    { id: "button-6", label: "DPI cycle", fallback: "DPI Cycle" },
    { id: "wheel-left", label: "Wheel left", fallback: "Unassigned" },
    { id: "wheel-right", label: "Wheel right", fallback: "Unassigned" },
  ];

  const keyboardControls: Control[] = Array.from({ length: 12 }, (_, i) => ({
    id: `g-${i + 1}`,
    label: `G${i + 1} (Fn+F${i + 1})`,
    fallback: `F${i + 1}`,
  }));

  const controls = $derived(device.kind === "keyboard" ? keyboardControls : mouseControls);

  interface Command {
    category: Assignment["category"];
    label: string;
    value: string;
  }

  const library: { name: string; icon: IconName; items: Command[] }[] = [
    {
      name: "Commands",
      icon: "assignments",
      items: [
        { category: "command", label: "Copy", value: "ctrl+c" },
        { category: "command", label: "Paste", value: "ctrl+v" },
        { category: "command", label: "Cut", value: "ctrl+x" },
        { category: "command", label: "Undo", value: "ctrl+z" },
        { category: "command", label: "Redo", value: "ctrl+shift+z" },
        { category: "command", label: "Select all", value: "ctrl+a" },
        { category: "command", label: "Save", value: "ctrl+s" },
        { category: "command", label: "Find", value: "ctrl+f" },
      ],
    },
    {
      name: "Keys",
      icon: "keycap",
      items: [
        { category: "key", label: "Escape", value: "Escape" },
        { category: "key", label: "Tab", value: "Tab" },
        { category: "key", label: "Enter", value: "Return" },
        { category: "key", label: "Space", value: "space" },
        { category: "key", label: "Shift", value: "Shift_L" },
        { category: "key", label: "Ctrl", value: "Control_L" },
        { category: "key", label: "Alt", value: "Alt_L" },
        { category: "key", label: "Super", value: "Super_L" },
      ],
    },
    {
      name: "Actions",
      icon: "sliders",
      items: [
        { category: "action", label: "DPI up", value: "dpi-up" },
        { category: "action", label: "DPI down", value: "dpi-down" },
        { category: "action", label: "DPI cycle", value: "dpi-cycle" },
        { category: "action", label: "DPI shift", value: "dpi-shift" },
        { category: "action", label: "Next profile", value: "profile-next" },
        { category: "action", label: "Back", value: "mouse-back" },
        { category: "action", label: "Forward", value: "mouse-forward" },
        { category: "action", label: "Unassign", value: "" },
      ],
    },
    {
      name: "System",
      icon: "gear",
      items: [
        { category: "system", label: "Volume up", value: "XF86AudioRaiseVolume" },
        { category: "system", label: "Volume down", value: "XF86AudioLowerVolume" },
        { category: "system", label: "Mute", value: "XF86AudioMute" },
        { category: "system", label: "Play / pause", value: "XF86AudioPlay" },
        { category: "system", label: "Next track", value: "XF86AudioNext" },
        { category: "system", label: "Previous track", value: "XF86AudioPrev" },
        { category: "system", label: "Screenshot", value: "Print" },
        { category: "system", label: "Lock screen", value: "lock-screen" },
      ],
    },
  ];

  let assignments = $state<Assignment[]>([]);
  let selected = $state<string>("");
  let dragOver = $state<string | null>(null);
  let search = $state("");

  let seededFor = $state<string | null>(null);

  // Once per device: assigning writes the profile back, so tracking the config
  // store here would re-enter this effect on every binding change.
  $effect(() => {
    const id = device.id;
    if (seededFor === id) return;
    seededFor = id;

    untrack(() => {
      const profile = configStore.deviceProfile(id);
      assignments = [...profile.assignments];
      macros = [...(profile.macros ?? [])];
      // The control set depends on the device kind, so pick a valid default.
      if (!controls.some((c) => c.id === selected)) selected = controls[0]?.id ?? "";
    });
  });

  const filtered = $derived(
    library
      .map((group) => ({
        ...group,
        items: group.items.filter((item) =>
          item.label.toLowerCase().includes(search.trim().toLowerCase()),
        ),
      }))
      .filter((group) => group.items.length > 0),
  );

  function assignmentFor(controlId: string): Assignment | undefined {
    return assignments.find((a) => a.control === controlId);
  }

  async function assign(controlId: string, command: Command) {
    const next = assignments.filter((a) => a.control !== controlId);
    if (command.value !== "") {
      next.push({
        control: controlId,
        category: command.category,
        label: command.label,
        value: command.value,
      });
    }
    assignments = next;
    const profile = configStore.deviceProfile(device.id);
    await configStore.saveDeviceProfile(device.id, { ...profile, assignments: next });
    ui.toast(
      command.value === ""
        ? `Cleared ${controlLabel(controlId)}.`
        : `${controlLabel(controlId)} → ${command.label}`,
      "success",
      2200,
    );
  }

  function controlLabel(id: string): string {
    return controls.find((c) => c.id === id)?.label ?? id;
  }

  function onDragStart(event: DragEvent, command: Command) {
    event.dataTransfer?.setData("application/json", JSON.stringify(command));
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "copy";
  }

  function onDrop(event: DragEvent, controlId: string) {
    event.preventDefault();
    dragOver = null;
    const raw = event.dataTransfer?.getData("application/json");
    if (!raw) return;
    assign(controlId, JSON.parse(raw) as Command);
  }

  /** Which library group the panel is showing, like G HUB's sub-tabs. */
  let group = $state("Commands");

  // -- game commands ----------------------------------------------------------
  //
  // When the active profile is bound to a game, G HUB's COMMANDS tab shows that
  // game's own keybinds from Logitech's database, grouped by category with the
  // category colours. Otherwise the generic editing commands are shown.

  let gameCommands = $state<ApplicationCommands | null>(null);
  let gameFor = $state<string | null>(null);

  $effect(() => {
    const appId = configStore.active?.applicationId ?? null;
    if (appId === gameFor) return;
    gameFor = appId;
    if (!appId) {
      gameCommands = null;
      return;
    }
    untrack(() =>
      api
        .getApplicationCommands(appId)
        .then((c) => (gameCommands = c))
        .catch(() => (gameCommands = null)),
    );
  });

  /** Game commands as library groups, one per category, coloured. */
  const gameGroups = $derived.by(() => {
    if (!gameCommands) return [];
    const q = search.trim().toLowerCase();
    const colour = new Map(gameCommands.categoryColors.map((c) => [c.tag, c.hex]));
    const byCategory = new Map<string, Command[]>();
    for (const c of gameCommands.commands) {
      if (q && !c.name.toLowerCase().includes(q)) continue;
      const list = byCategory.get(c.category) ?? [];
      list.push({ category: "key", label: c.name, value: c.keystroke.join("+") });
      byCategory.set(c.category, list);
    }
    return [...byCategory].map(([name, items]) => ({
      name,
      colour: colour.get(name) ?? null,
      items,
    }));
  });

  // -- macros ---------------------------------------------------------------

  let macros = $state<MacroDef[]>([]);
  let editing = $state<string | null>(null);
  let writing = $state(false);

  /**
   * Physical button index for a control id, which is what the device's profile
   * stores. `button-1` is index 0; wheel tilts are not addressable this way.
   */
  function buttonIndexFor(controlId: string): number | null {
    const m = /^button-(\d+)$/.exec(controlId);
    return m ? Number(m[1]) - 1 : null;
  }

  const macroBudget = 253;
  const macroBytes = $derived(
    assignments
      .filter((a) => a.category === "macro")
      .reduce((total, a) => {
        const def = macros.find((m) => m.id === a.value);
        return total + (def ? encodedSize(def.steps) : 0);
      }, 0),
  );

  function newMacro() {
    const def: MacroDef = { id: `m${Date.now()}`, name: `Macro ${macros.length + 1}`, steps: [] };
    macros = [...macros, def];
    editing = def.id;
    persistMacros();
  }

  function updateMacro(next: MacroDef) {
    macros = macros.map((m) => (m.id === next.id ? next : m));
    persistMacros();
  }

  function deleteMacro(id: string) {
    macros = macros.filter((m) => m.id !== id);
    assignments = assignments.filter((a) => !(a.category === "macro" && a.value === id));
    if (editing === id) editing = null;
    persistMacros();
  }

  async function persistMacros() {
    const profile = configStore.deviceProfile(device.id);
    await configStore.saveDeviceProfile(device.id, { ...profile, macros, assignments });
  }

  /** Binds a macro to the selected control and writes it to the device. */
  async function assignMacro(def: MacroDef) {
    const index = buttonIndexFor(selected);
    if (index === null) {
      ui.toast("That control cannot hold a macro — pick a numbered button.", "error");
      return;
    }
    if (def.steps.length === 0) {
      ui.toast("Record some keys first.", "error");
      return;
    }

    const next = assignments.filter((a) => a.control !== selected);
    next.push({ control: selected, category: "macro", label: def.name, value: def.id });
    assignments = next;
    await persistMacros();
    await writeToDevice();
  }

  /** Renders every macro binding into the device's onboard memory. */
  async function writeToDevice() {
    if (!device.capabilities.onboardMemory) {
      ui.toast("This device has no onboard memory to store macros in.", "error");
      return;
    }
    writing = true;
    try {
      const payload = assignments
        .filter((a) => a.category === "macro")
        .flatMap((a) => {
          const index = buttonIndexFor(a.control);
          const def = macros.find((m) => m.id === a.value);
          return index !== null && def ? [{ button: index, steps: def.steps }] : [];
        });
      const backup = await api.applyOnboardMacros(device.id, payload);
      ui.toast(`Written to the device. Backup: ${backup.split("/").pop()}`, "success", 5000);
    } catch (e) {
      ui.toast(`Could not write macros: ${api.errorMessage(e)}`, "error", 7000);
    } finally {
      writing = false;
    }
  }
  /**
   * Button positions. An imported G HUB layout is authoritative — it is the
   * exact marker/label geometry G HUB draws — and adds any buttons the generic
   * table does not know. Otherwise the per-category guesses apply.
   */
  const layout = $derived(artwork.layoutFor(artworkIds(device)));
  const spots = $derived.by<Record<string, ControlSpot>>(() => {
    const generic = controlSpots(device.kind);
    const front = layout?.views.find((v) => v.view === "front");
    if (!front) return generic;
    const exact: Record<string, ControlSpot> = {};
    for (const c of front.controls) {
      // Labels beyond the image edge are clamped into the margins we draw.
      const lx = c.side === "left" ? 0.02 : c.side === "right" ? 0.98 : c.labelX;
      const ly = c.side === "top" ? 0.02 : c.labelY;
      exact[c.control] = {
        dot: { x: c.markerX, y: c.markerY },
        label: { x: lx, y: ly },
        side: c.side,
      };
    }
    return { ...generic, ...exact };
  });
  /** Controls with a position, including layout-only ones not in the list. */
  const placed = $derived.by(() => {
    const known = controls.filter((c) => spots[c.id]);
    const extra = Object.keys(spots)
      .filter((id) => !controls.some((c) => c.id === id))
      .map((id) => ({ id, label: id.replace("button-", "G"), fallback: "Unassigned" }));
    return [...known, ...extra];
  });
  const activeGroup = $derived(filtered.find((g) => g.name === group) ?? filtered[0]);
</script>

<DeviceWorkspace title="Assignments">
  {#snippet panel()}
    <div class="group-tabs" role="tablist">
      {#each [...library, { name: "Macros" }] as g (g.name)}
        <button
          class="group-tab"
          class:active={g.name === group}
          role="tab"
          aria-selected={g.name === group}
          onclick={() => (group = g.name)}
        >
          {g.name}
        </button>
      {/each}
    </div>

    <p class="hint">Drag a command onto a target to assign it to this device.</p>

    <label class="search">
      <Icon name="search" size={14} />
      <input type="text" placeholder="Search for a command" bind:value={search} spellcheck="false" />
    </label>

    {#if group === "Macros"}
      <div class="macros">
        {#each macros as def (def.id)}
          <div class="macro">
            <button class="macro-row" onclick={() => assignMacro(def)}>
              <span class="macro-name">{def.name}</span>
              <span class="macro-meta">{def.steps.length} steps</span>
            </button>
            <button
              class="icon"
              onclick={() => (editing = editing === def.id ? null : def.id)}
              aria-label="Edit macro"
            >
              <Icon name="pencil" size={13} />
            </button>
            <button class="icon" onclick={() => deleteMacro(def.id)} aria-label="Delete macro">
              <Icon name="trash" size={13} />
            </button>
          </div>
          {#if editing === def.id}
            <MacroRecorder
              macro={def}
              budget={macroBudget}
              onchange={updateMacro}
              onclose={() => (editing = null)}
            />
          {/if}
        {:else}
          <p class="hint">No macros yet.</p>
        {/each}

        <button class="new-macro" onclick={newMacro}>
          <Icon name="plus" size={14} /> New macro
        </button>

        <p class="hint">
          Click a macro to bind it to <strong>{controlLabel(selected)}</strong> and write it to
          the device. Using {macroBytes}/{macroBudget} bytes of the macro sector.
        </p>
        <button class="new-macro" onclick={writeToDevice} disabled={writing}>
          <Icon name="chip" size={14} />
          {writing ? "Writing…" : "Re-write macros to device"}
        </button>
      </div>
    {:else if group === "Commands" && gameCommands}
      <div class="commands">
        <p class="hint game-head">
          <strong>{gameCommands.name}</strong> — {gameCommands.commands.length} commands from
          Logitech's database.
        </p>
        {#each gameGroups as g (g.name)}
          <div class="game-cat">
            <span class="swatch" style="background: {g.colour ?? 'var(--text-dimmer)'}"></span>
            {g.name}
          </div>
          {#each g.items as item (item.label)}
            <button
              class="command"
              draggable="true"
              ondragstart={(e) => onDragStart(e, item)}
              onclick={() => assign(selected, item)}
            >
              <span class="command-key">{item.value || "—"}</span>
              <span class="command-name">{item.label}</span>
            </button>
          {/each}
        {:else}
          <p class="hint">No commands match “{search}”.</p>
        {/each}
      </div>
    {:else}
    <div class="commands">
      {#if activeGroup}
        {#each activeGroup.items as item (item.label)}
          <button
            class="command"
            class:clear-command={item.value === ""}
            draggable="true"
            ondragstart={(e) => onDragStart(e, item)}
            onclick={() => assign(selected, item)}
          >
            <span class="command-key">{item.value || "—"}</span>
            <span class="command-name">{item.label}</span>
          </button>
        {/each}
      {:else}
        <p class="hint">No commands match “{search}”.</p>
      {/if}
    </div>
    {/if}

    <p class="hint foot">
      Assigning to <strong>{controlLabel(selected)}</strong>. Macros are written into the
      device's onboard memory (<code>0x8100</code>) and a backup is taken first. Other command
      types are stored in <strong>{configStore.active?.name}</strong> only, and do not reach the
      hardware yet.
    </p>
  {/snippet}

  {#snippet stage()}
    <div class="callouts">
      <DeviceArt kind={device.kind} productIds={artworkIds(device)} class="render" />

      <!-- Leader lines, drawn under the markers. -->
      <svg class="lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        {#each placed as control (control.id)}
          {@const spot = spots[control.id]}
          <line
            x1={spot.label.x * 100}
            y1={spot.label.y * 100}
            x2={spot.dot.x * 100}
            y2={spot.dot.y * 100}
            class:active={selected === control.id}
          />
        {/each}
      </svg>

      {#each placed as control (control.id)}
        {@const spot = spots[control.id]}
        {@const bound = assignmentFor(control.id)}
        <button
          class="dot"
          class:active={selected === control.id}
          class:dragover={dragOver === control.id}
          style="left: {spot.dot.x * 100}%; top: {spot.dot.y * 100}%"
          aria-label={control.label}
          onclick={() => (selected = control.id)}
          ondragover={(e) => {
            e.preventDefault();
            dragOver = control.id;
          }}
          ondragleave={() => (dragOver = null)}
          ondrop={(e) => onDrop(e, control.id)}
        ></button>

        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="callout {spot.side}"
          class:active={selected === control.id}
          class:dragover={dragOver === control.id}
          style="left: {spot.label.x * 100}%; top: {spot.label.y * 100}%"
          onclick={() => (selected = control.id)}
          ondragover={(e) => {
            e.preventDefault();
            dragOver = control.id;
          }}
          ondragleave={() => (dragOver = null)}
          ondrop={(e) => onDrop(e, control.id)}
        >
          <span class="callout-name">{control.label}</span>
          <span class="callout-binding" class:bound={!!bound}>
            {bound?.label ?? control.fallback}
          </span>
        </div>
      {/each}
    </div>
  {/snippet}
</DeviceWorkspace>

<style>
  .group-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 18px;
    border-bottom: 1px solid var(--line);
  }

  .group-tab {
    padding: 0 0 9px;
    font-family: var(--font);
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-dim);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }

  .group-tab:hover {
    color: var(--text);
  }

  .group-tab.active {
    color: var(--text);
    border-bottom-color: var(--cyan);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 0 8px;
    border-bottom: 1px solid var(--line);
    color: var(--text-dimmer);
  }

  .search input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 13px;
    user-select: text;
  }

  .commands {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .game-head {
    padding: 4px 6px 8px;
  }

  .game-cat {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 6px 4px;
    font-family: var(--font);
    font-size: 12.5px;
    font-weight: 600;
  }

  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .command {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 9px 6px;
    border-bottom: 1px solid var(--line);
    text-align: left;
    cursor: grab;
  }

  .command:hover {
    background: var(--surface-2);
  }

  .command:active {
    cursor: grabbing;
  }

  .command-key {
    min-width: 84px;
    font-family: var(--font);
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text);
  }

  .command-name {
    font-size: 12.5px;
    color: var(--text-dim);
  }

  .clear-command .command-key {
    color: var(--text-dimmer);
  }

  .macros {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .macro {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .macro-row {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 9px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    text-align: left;
  }

  .macro-row:hover {
    border-color: var(--cyan);
  }

  .macro-name {
    flex: 1;
    font-family: var(--font);
    font-size: 13.5px;
    font-weight: 600;
  }

  .macro-meta {
    font-size: 11px;
    color: var(--text-dimmer);
  }

  .new-macro {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 9px;
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-dim);
  }

  .new-macro:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--text-dim);
  }

  .new-macro:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--text-dimmer);
  }

  .icon:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .callouts {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .callouts :global(.render) {
    /* Leave room around the edges for the labels. */
    padding: 0 22%;
  }

  .lines {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .lines line {
    stroke: var(--line-strong);
    stroke-width: 0.18;
    vector-effect: non-scaling-stroke;
  }

  .lines line.active {
    stroke: var(--cyan);
  }

  .dot {
    position: absolute;
    width: 13px;
    height: 13px;
    transform: translate(-50%, -50%);
    border: 2px solid #fff;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.35);
    transition: transform 120ms var(--ease), border-color 120ms var(--ease);
  }

  .dot:hover {
    transform: translate(-50%, -50%) scale(1.25);
  }

  .dot.active {
    border-color: var(--cyan);
    background: var(--cyan);
  }

  .dot.dragover {
    border-color: var(--cyan);
    transform: translate(-50%, -50%) scale(1.5);
  }

  .callout {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-width: 22%;
    padding: 4px 7px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
  }

  .callout.left {
    transform: translate(0, -50%);
    text-align: left;
  }

  .callout.right {
    transform: translate(-100%, -50%);
    text-align: right;
  }

  .callout.top {
    transform: translate(-50%, 0);
    text-align: center;
  }

  .callout:hover {
    border-color: var(--line-strong);
  }

  .callout.active {
    border-color: var(--cyan);
  }

  .callout.dragover {
    border-color: var(--cyan);
    border-style: dashed;
    background: rgba(0, 181, 226, 0.12);
  }

  .callout-name {
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
  }

  .callout-binding {
    font-size: 11px;
    color: var(--text-dimmer);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .callout-binding.bound {
    color: var(--cyan);
  }

  .hint {
    font-size: 11.5px;
    color: var(--text-dimmer);
    line-height: 1.55;
  }

  .hint strong {
    color: var(--text-dim);
    font-weight: 600;
  }

  .hint code {
    font-family: ui-monospace, "DejaVu Sans Mono", monospace;
  }

  .foot {
    margin-top: auto;
  }
</style>
