<script lang="ts">
  /**
   * Assignments — drag a command onto a control, exactly like G HUB.
   *
   * Bindings are stored in the active profile and pushed to the device right
   * away: in software mode the mouse reports raw presses (0x8110) and OpenGHub
   * performs the action; devices with onboard profiles get their button table
   * written too, so they keep working when the app is closed.
   */
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import * as api from "$lib/api";
  import MacroEditor from "$lib/components/MacroEditor.svelte";
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
    { id: "button-1", label: "Left click", fallback: "Primary Click" },
    { id: "button-2", label: "Right click", fallback: "Secondary Click" },
    { id: "button-3", label: "Middle click", fallback: "Middle Click" },
    { id: "button-4", label: "Thumb back", fallback: "Back" },
    { id: "button-5", label: "Thumb forward", fallback: "Forward" },
    { id: "button-6", label: "DPI cycle", fallback: "DPI Cycle" },
    { id: "wheel-left", label: "Wheel left", fallback: "Scroll Left" },
    { id: "wheel-right", label: "Wheel right", fallback: "Scroll Right" },
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
    /** G HUB groups the COMMANDS list under collapsible headers. */
    group?: string;
  }

  const library: { name: string; icon: IconName; items: Command[] }[] = [
    {
      name: "Commands",
      icon: "assignments",
      items: [
        { category: "command", label: "Go Back", value: "alt+Left", group: "Navigation" },
        { category: "command", label: "Go Forward", value: "alt+Right", group: "Navigation" },
        { category: "command", label: "Switch Window", value: "alt+Tab", group: "Navigation" },
        { category: "command", label: "Activities / Launcher", value: "super", group: "Navigation" },
        { category: "command", label: "Next Workspace", value: "ctrl+alt+Right", group: "Navigation" },
        { category: "command", label: "Previous Workspace", value: "ctrl+alt+Left", group: "Navigation" },
        { category: "command", label: "Copy", value: "ctrl+c", group: "Editing" },
        { category: "command", label: "Paste", value: "ctrl+v", group: "Editing" },
        { category: "command", label: "Cut", value: "ctrl+x", group: "Editing" },
        { category: "command", label: "Undo", value: "ctrl+z", group: "Editing" },
        { category: "command", label: "Redo", value: "ctrl+shift+z", group: "Editing" },
        { category: "command", label: "Select All", value: "ctrl+a", group: "Editing" },
        { category: "command", label: "Save", value: "ctrl+s", group: "Editing" },
        { category: "command", label: "Find", value: "ctrl+f", group: "Editing" },
        { category: "command", label: "Close Window", value: "alt+F4", group: "Desktop" },
        { category: "command", label: "Maximise Window", value: "super+Up", group: "Desktop" },
        { category: "command", label: "Show Desktop", value: "super+d", group: "Desktop" },
        { category: "command", label: "Open Terminal", value: "ctrl+alt+t", group: "Desktop" },
        { category: "command", label: "Lock Screen", value: "super+l", group: "Desktop" },
        { category: "command", label: "New Tab", value: "ctrl+t", group: "Browser" },
        { category: "command", label: "Close Tab", value: "ctrl+w", group: "Browser" },
        { category: "command", label: "Reopen Closed Tab", value: "ctrl+shift+t", group: "Browser" },
        { category: "command", label: "Next Tab", value: "ctrl+Tab", group: "Browser" },
        { category: "command", label: "Previous Tab", value: "ctrl+shift+Tab", group: "Browser" },
        { category: "command", label: "Reload", value: "F5", group: "Browser" },
        { category: "command", label: "Address Bar", value: "ctrl+l", group: "Browser" },
        { category: "command", label: "Browser Back", value: "XF86Back", group: "Browser" },
        { category: "command", label: "Browser Forward", value: "XF86Forward", group: "Browser" },
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
        { category: "action", label: "Primary Click", value: "mouse-left" },
        { category: "action", label: "Secondary Click", value: "mouse-right" },
        { category: "action", label: "Middle Click", value: "mouse-middle" },
        { category: "action", label: "DPI up", value: "dpi-up" },
        { category: "action", label: "DPI down", value: "dpi-down" },
        { category: "action", label: "DPI cycle", value: "dpi-cycle" },
        { category: "action", label: "DPI shift", value: "dpi-shift" },
        { category: "action", label: "G-Shift", value: "gshift" },
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
  /** G HUB's DEFAULT / G-SHIFT layers: shifted bindings use a suffixed control id. */
  let layer = $state<"default" | "gshift">("default");
  let collapsed = $state<Record<string, boolean>>({});
  /** Macro open in the full-screen editor, and whether it was just created. */
  let editingMacro = $state<MacroDef | null>(null);
  let editingFresh = $state(false);
  let dragOver = $state<string | null>(null);
  let search = $state("");

  let seededFor = $state<string | null>(null);

  // Once per device: assigning writes the profile back, so tracking the config
  // store here would re-enter this effect on every binding change.
  $effect(() => {
    if (spots[selected] === undefined) {
      const first = Object.keys(spots)[0];
      if (first) selected = first;
    }
  });

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

      // `?macro=new` / `?macro=<id>` deep-links straight into the editor.
      const want = page.url.searchParams.get("macro");
      if (want === "new") {
        group = "Macros";
        createMacro();
      } else if (want) {
        const def = macros.find((m) => m.id === want);
        if (def) {
          group = "Macros";
          openMacro(def);
        }
      }
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

  /** Control id as stored for the current layer. */
  function keyFor(controlId: string): string {
    return layer === "gshift" ? `${controlId}:gshift` : controlId;
  }

  function assignmentFor(controlId: string): Assignment | undefined {
    const key = keyFor(controlId);
    return assignments.find((a) => a.control === key);
  }

  /** G HUB refuses to leave a mouse without a primary click. */
  let primaryDialog = $state<{ control: string } | null>(null);

  function isPrimaryClick(a: Pick<Assignment, "category" | "value">): boolean {
    return a.category === "action" && a.value === "mouse-left";
  }

  /** True when, after `next` is applied, some button still fires the primary click. */
  function keepsPrimary(next: Assignment[]): boolean {
    if (device.kind !== "mouse") return true;
    const base = (c: string) => c.split(":")[0];
    const layer = next.filter((a) => !a.control.includes(":"));
    const button1 = layer.find((a) => base(a.control) === "button-1");
    if (!button1 || isPrimaryClick(button1)) return true;
    return layer.some((a) => base(a.control) !== "button-1" && isPrimaryClick(a));
  }

  async function assign(controlId: string, command: Command) {
    const key = keyFor(controlId);
    const next = assignments.filter((a) => a.control !== key);
    if (command.value !== "") {
      next.push({ control: key, category: command.category, label: command.label, value: command.value });
    }
    if (!keepsPrimary(next)) {
      primaryDialog = { control: controlId };
      return;
    }
    await commitAssignments(next, controlId, command);
  }

  async function commitAssignments(next: Assignment[], controlId: string, command: Command) {
    assignments = next;
    const profile = configStore.deviceProfile(device.id);
    await configStore.saveDeviceProfile(device.id, { ...profile, assignments: next });
    await pushToDevice(
      command.value === "" ? `Cleared ${controlLabel(controlId)}.` : `${controlLabel(controlId)} → ${command.label}`,
    );
  }

  /** Applies the saved assignments to the hardware and reports how. */
  async function pushToDevice(what: string) {
    try {
      const r = await api.applyAssignments(device.id);
      const how = r.software && r.onboard ? "live + onboard" : r.software ? "live" : r.onboard ? "onboard" : "saved";
      ui.toast(`${what} (${how})`, "success", 2200);
    } catch (e) {
      ui.toast(`${what} — not applied: ${api.errorMessage(e)}`, "error", 6000);
    }
  }

  /** "ctrl+shift+z" → "CTRL + SHIFT + Z", as G HUB prints keystrokes. */
  function prettyKeys(value: string): string {
    if (!value) return "—";
    if (!value.includes("+") && value.length > 1 && !/^[a-z]/.test(value)) return value;
    return value
      .split("+")
      .map((k) => k.trim())
      .map((k) =>
        k.length === 1
          ? k.toUpperCase()
          : k
              .replace(/^(left |right )?windows$/i, "Super")
              .replace(/^super$/i, "Super")
              .replace(/^XF86/, "")
              .replace(/^\w/, (c) => c.toUpperCase()),
      )
      .join(" + ")
      .replace(/\b(Ctrl|Alt|Shift)\b/g, (m) => m.toUpperCase());
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

  function deleteMacro(id: string) {
    macros = macros.filter((m) => m.id !== id);
    assignments = assignments.filter((a) => !(a.category === "macro" && a.value === id));
    if (editingMacro?.id === id) editingMacro = null;
    persistMacros().then(() => pushToDevice("Macro removed"));
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

    const key = keyFor(selected);
    const next = assignments.filter((a) => a.control !== key);
    next.push({ control: key, category: "macro", label: def.name, value: def.id });
    if (!keepsPrimary(next)) {
      primaryDialog = { control: selected };
      return;
    }
    assignments = next;
    await persistMacros();
    await pushToDevice(`${controlLabel(selected)} → ${def.name}`);
  }

  /** Re-pushes every assignment (macros included) to the device. */
  async function writeToDevice() {
    writing = true;
    try {
      await pushToDevice("Assignments written");
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
  /** `front` or `side`; G HUB switches with ◀ ▶ when the depot has both. */
  let view = $state<"front" | "side">("front");
  const views = $derived<("front" | "side")[]>((layout?.views.map((v) => v.view) as ("front" | "side")[] | undefined) ?? ["front"]);
  const layoutView = $derived(layout?.views.find((v) => v.view === view) ?? null);

  /**
   * Button positions. An imported G HUB layout is authoritative and replaces
   * the generic table entirely — it is the exact marker/label geometry G HUB
   * draws, and it knows which buttons are visible in which view. Without one,
   * the per-category guesses apply.
   */
  const spots = $derived.by<Record<string, ControlSpot>>(() => {
    if (!layoutView) return controlSpots(device.kind);
    const exact: Record<string, ControlSpot> = {};
    for (const c of layoutView.controls) {
      exact[c.control] = {
        dot: { x: c.markerX, y: c.markerY },
        // Labels sit in the margins beside the image; the side decides which.
        label: { x: c.side === "left" ? -0.06 : c.side === "right" ? 1.06 : c.labelX, y: c.side === "top" ? -0.05 : c.labelY },
        side: c.side,
      };
    }
    return exact;
  });

  /** Controls to draw: only those with a position in the current view. */
  const placed = $derived.by(() =>
    Object.keys(spots).map((id) => {
      const known = controls.find((c) => c.id === id);
      return known ?? { id, label: id.replace("button-", "G"), fallback: "Unassigned" };
    }),
  );

  // The render is `object-fit: contain`; markers are fractions of the image,
  // so the overlay is sized to the image's letterboxed rectangle, not the box.
  let boxW = $state(1);
  let boxH = $state(1);
  const imgAspect = $derived(layoutView ? layoutView.width / layoutView.height : 0.6);
  const imgRect = $derived.by(() => {
    // Leave room either side for labels: the image gets the middle 56%.
    const availW = boxW * 0.56;
    const availH = boxH * 0.92;
    let w: number, h: number;
    if (imgAspect > availW / Math.max(1, availH)) {
      w = availW;
      h = w / imgAspect;
    } else {
      h = availH;
      w = h * imgAspect;
    }
    return { w, h, left: (boxW - w) / 2, top: (boxH - h) / 2 };
  });
  const activeGroup = $derived(filtered.find((g) => g.name === group) ?? filtered[0]);

  /** The active tab's items under their headers, in first-seen order. */
  const grouped = $derived.by(() => {
    if (!activeGroup) return [];
    const map = new Map<string, Command[]>();
    for (const item of activeGroup.items) {
      const g = item.group ?? activeGroup.name;
      map.set(g, [...(map.get(g) ?? []), item]);
    }
    return [...map].map(([name, items]) => ({ name, items, colour: null as string | null }));
  });

  function openMacro(def: MacroDef, isNew = false) {
    editingMacro = def;
    editingFresh = isNew;
  }

  function createMacro() {
    const def: MacroDef = { id: `m${Date.now()}`, name: "", steps: [], kind: "noRepeat" };
    openMacro(def, true);
  }

  async function saveMacro(next: MacroDef) {
    const exists = macros.some((m) => m.id === next.id);
    macros = exists ? macros.map((m) => (m.id === next.id ? next : m)) : [...macros, next];
    editingMacro = next;
    editingFresh = false;
    await persistMacros();
    if (assignments.some((a) => a.category === "macro" && a.value === next.id)) {
      await pushToDevice(`Saved ${next.name}`);
    } else {
      ui.toast(`Saved ${next.name}.`, "success", 2200);
    }
  }
</script>

<DeviceWorkspace title="Assignments">
  {#snippet panel()}
    <div class="group-tabs" role="tablist">
      {#each [...library.slice(0, 3), { name: "Macros" }, ...library.slice(3)] as g (g.name)}
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
            <button class="macro-row" onclick={() => assignMacro(def)} title="Assign to {controlLabel(selected)}">
              <span class="macro-swatch" style={def.color ? `background:${def.color}` : ""}></span>
              <span class="macro-name">{def.name}</span>
              <span class="macro-meta">{def.steps.length} steps</span>
            </button>
            <button class="icon" onclick={() => openMacro(def)} aria-label="Edit macro">
              <Icon name="pencil" size={13} />
            </button>
            <button class="icon" onclick={() => deleteMacro(def.id)} aria-label="Delete macro">
              <Icon name="trash" size={13} />
            </button>
          </div>
        {:else}
          <p class="hint">No macros yet.</p>
        {/each}

        <button class="new-macro" onclick={createMacro}>
          <Icon name="plus" size={14} strokeWidth={2} /> Create new macro
        </button>

        <p class="hint">
          Click a macro to bind it to <strong>{controlLabel(selected)}</strong>.
          {#if device.capabilities.onboardMemory}Using {macroBytes}/{macroBudget} bytes of the onboard macro sector.{/if}
        </p>
        <button class="new-macro" onclick={writeToDevice} disabled={writing}>
          <Icon name="chip" size={14} />
          {writing ? "Writing…" : "Re-apply to device"}
        </button>
      </div>
    {:else if group === "Commands" && gameCommands}
      <div class="commands">
        <p class="hint game-head">
          <strong>{gameCommands.name}</strong> — {gameCommands.commands.length} commands from
          Logitech's database.
        </p>
        {#each gameGroups as g (g.name)}
          {@render commandGroup(g)}
        {:else}
          <p class="hint">No commands match “{search}”.</p>
        {/each}
      </div>
    {:else}
      <div class="commands">
        {#each grouped as g (g.name)}
          {@render commandGroup(g)}
        {:else}
          <p class="hint">No commands match “{search}”.</p>
        {/each}
      </div>
    {/if}
  {/snippet}



  {#snippet stageFooter()}
    <div class="stage-controls">
      {#if views.length > 1}
        <div class="views">
          {#each views as v, i (v)}
            <button class="view-pill" class:active={view === v} onclick={() => (view = v)}>View {i + 1}</button>
          {/each}
        </div>
      {/if}
      <div class="layers">
        <span class:on={layer === "default"}>Default</span>
        <button
          class="switch"
          class:right={layer === "gshift"}
          role="switch"
          aria-checked={layer === "gshift"}
          aria-label="G-Shift layer"
          onclick={() => (layer = layer === "default" ? "gshift" : "default")}
        ></button>
        <span class:on={layer === "gshift"}>G-Shift</span>
      </div>
    </div>
  {/snippet}

  {#snippet stage()}
    <div class="callouts" bind:clientWidth={boxW} bind:clientHeight={boxH}>
      <!-- The image box: everything positional is a fraction of this. -->
      <div
        class="image-box"
        style="left: {imgRect.left}px; top: {imgRect.top}px; width: {imgRect.w}px; height: {imgRect.h}px"
      >
        <DeviceArt
          kind={device.kind}
          productIds={artworkIds(device)}
          variant={view}
          class="render tight"
        />

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
            <span class="callout-binding" class:bound={!!bound} title={control.label}>
              {bound?.label ?? control.fallback}
            </span>
          </div>
        {/each}
      </div>

    </div>
  {/snippet}
</DeviceWorkspace>

{#snippet commandGroup(g: { name: string; items: Command[]; colour: string | null })}
  <button class="group-head" onclick={() => (collapsed = { ...collapsed, [g.name]: !collapsed[g.name] })}>
    {#if g.colour}<span class="swatch" style="background: {g.colour}"></span>{/if}
    <span class="group-name">{g.name}</span>
    <Icon name={collapsed[g.name] ? "chevronDown" : "chevronUp"} size={14} />
  </button>
  {#if !collapsed[g.name]}
    {#each g.items as item (item.label)}
      <button
        class="command"
        class:clear-command={item.value === ""}
        draggable="true"
        ondragstart={(e) => onDragStart(e, item)}
        onclick={() => assign(selected, item)}
      >
        <span class="command-key">{prettyKeys(item.value)}</span>
        <span class="command-name">{item.label}</span>
      </button>
    {/each}
  {/if}
{/snippet}

{#if primaryDialog}
  <div class="scrim" role="presentation" onclick={(e) => e.target === e.currentTarget && (primaryDialog = null)}>
    <div class="dialog" role="dialog" aria-modal="true">
      <Icon name="alert" size={30} strokeWidth={1.5} />
      <h3>Primary click required</h3>
      <p>
        {controlLabel(primaryDialog.control)} is your only Primary Click. Assign
        <strong>Actions → Primary Click</strong> to another button first, then change this one.
      </p>
      <div class="dialog-actions">
        <button class="dialog-ok" onclick={() => (primaryDialog = null)}>OK</button>
      </div>
    </div>
  </div>
{/if}

{#if editingMacro}
  {#key editingMacro.id}
    <MacroEditor
    macro={editingMacro}
    fresh={editingFresh}
    onsave={saveMacro}
    onclose={() => {
      editingMacro = null;
      editingFresh = false;
    }}
    />
  {/key}
{/if}

<style>
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

  .dialog p strong {
    color: var(--text);
  }

  .dialog-actions {
    display: flex;
    gap: 10px;
    margin-top: 6px;
  }

  .dialog-ok {
    padding: 9px 26px;
    border-radius: 4px;
    background: var(--cyan);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #fff;
  }

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

  .group-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 4px 6px;
    font-family: var(--font);
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
    text-align: left;
  }

  .group-name {
    flex: 1;
  }

  .macro-swatch {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-dimmer);
  }

  .stage-controls {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 22px;
  }

  .views {
    display: flex;
    gap: 6px;
  }

  .view-pill {
    height: 20px;
    padding: 0 10px;
    border-radius: 999px;
    border: 1px solid var(--text);
    font-family: var(--font);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text);
  }

  .view-pill.active {
    background: var(--text);
    color: #000;
  }

  .layers {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dimmer);
  }

  .layers .on {
    color: var(--text);
  }

  .switch {
    position: relative;
    width: 30px;
    height: 16px;
    border-radius: 999px;
    background: #3a3b3f;
  }

  .switch::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    transition: left 120ms var(--ease);
  }

  .switch.right::after {
    left: 16px;
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

  .image-box {
    position: absolute;
  }

  .image-box :global(.render) {
    position: absolute;
    inset: 0;
  }

  .lines {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
    z-index: 26;
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
    z-index: 27;
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
    max-width: 180px;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
  }

  /* The anchor is where the leader line ends; the label grows away from the
     image, so left-side labels end at the anchor and right-side ones begin. */
  .callout.left {
    transform: translate(-100%, -50%);
    text-align: right;
    align-items: flex-end;
  }

  .callout.right {
    transform: translate(0, -50%);
    text-align: left;
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

  /* G HUB prints the bound command as the label: white by default, yellow when changed. */
  .callout-binding {
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
    white-space: nowrap;
  }

  .callout-binding.bound {
    color: #f5b400;
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


</style>
