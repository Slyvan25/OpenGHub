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
  import DeviceArt from "$lib/components/DeviceArt.svelte";
  import DeviceWorkspace from "$lib/components/DeviceWorkspace.svelte";
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import { artworkIds } from "$lib/device-ui";
  import { controlSpots } from "$lib/zones";
  import { configStore } from "$lib/stores/config.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Assignment, Device } from "$lib/types";

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
      assignments = [...configStore.deviceProfile(id).assignments];
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
  const spots = $derived(controlSpots(device.kind));
  /** Only controls we have a position for can be drawn on the render. */
  const placed = $derived(controls.filter((c) => spots[c.id]));
  const activeGroup = $derived(filtered.find((g) => g.name === group) ?? filtered[0]);
</script>

<DeviceWorkspace title="Assignments">
  {#snippet panel()}
    <div class="group-tabs" role="tablist">
      {#each library as g (g.name)}
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

    <p class="hint foot">
      Assigning to <strong>{controlLabel(selected)}</strong>. Bindings are saved to
      <strong>{configStore.active?.name}</strong>, but are <strong>not written to the device
      yet</strong> — that needs the onboard profile memory (<code>0x8100</code>) on this
      hardware, or <code>0x1b04</code> on devices that expose it.
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
