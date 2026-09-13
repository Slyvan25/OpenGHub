<script lang="ts">
  /**
   * G HUB's macro editor, full screen and in three steps: name it, pick a type
   * (no repeat / repeat while holding / toggle / sequence), then build it from
   * recorded keystrokes, typed text and delays. Sequence macros have three
   * sections (on press / while holding / on release); the others have one.
   *
   * Everything the device can actually play is offered. G HUB's Action, Launch
   * Application and System entries run on the host and have no onboard
   * representation, so they are shown but disabled rather than pretending.
   */
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import { modifierFor, usageFor, MAX_DELAY_MS, type MacroStep } from "$lib/macros";
  import type { MacroDef, MacroKind, MacroSections } from "$lib/types";

  interface Props {
    macro: MacroDef;
    /** True for a macro created a moment ago: starts at the naming step. */
    fresh?: boolean;
    onsave: (macro: MacroDef) => void;
    onclose: () => void;
  }

  let { macro, fresh = false, onsave, onclose }: Props = $props();

  type Step = "name" | "type" | "edit";
  type Section = keyof MacroSections;

  const KINDS: { id: MacroKind; label: string; icon: IconName; blurb: string }[] = [
    { id: "noRepeat", label: "No repeat", icon: "arrowRight", blurb: "Press to play the macro once." },
    { id: "repeatWhileHolding", label: "Repeat while holding", icon: "repeat", blurb: "Press and hold to repeat the macro until released." },
    { id: "toggle", label: "Toggle", icon: "toggle", blurb: "Press to start repeating the macro. Press again to stop." },
    { id: "sequence", label: "Sequence", icon: "sequence", blurb: "Press to play the macro. Press and hold to repeat the “while holding” section." },
  ];

  const SECTIONS: { id: Section; label: string }[] = [
    { id: "onPress", label: "On press" },
    { id: "whileHolding", label: "While holding" },
    { id: "onRelease", label: "On release" },
  ];

  const COLORS = [null, "#e4002b", "#ff8a00", "#f5c400", "#2dc84d", "#1196ff", "#9b4dff", "#ff4fa3"];

  // The editor seeds its working copy once; the parent replaces the component
  // (keyed on the macro id) to open a different macro.
  // svelte-ignore state_referenced_locally
  let step = $state<Step>(fresh ? "name" : "edit");
  // svelte-ignore state_referenced_locally
  let name = $state(macro.name);
  // svelte-ignore state_referenced_locally
  let kind = $state<MacroKind>(macro.kind ?? "noRepeat");
  // svelte-ignore state_referenced_locally
  let sections = $state<MacroSections>(seedSections(macro));
  // svelte-ignore state_referenced_locally
  let useStandardDelays = $state(macro.useStandardDelays ?? true);
  // svelte-ignore state_referenced_locally
  let standardDelayMs = $state(macro.standardDelayMs ?? 50);
  let showUpDown = $state(true);
  // svelte-ignore state_referenced_locally
  let color = $state<string | null>(macro.color ?? null);
  let dirty = $state(false);

  /** Section whose "+" menu is open. */
  let menuFor = $state<Section | null>(null);
  /** Section being recorded into. */
  let recordingInto = $state<Section | null>(null);
  let textFor = $state<Section | null>(null);
  let textDraft = $state("");
  let delayFor = $state<Section | null>(null);
  let delayDraft = $state(100);
  let lastEventAt = 0;

  function seedSections(m: MacroDef): MacroSections {
    if (m.sections) {
      return {
        onPress: [...m.sections.onPress],
        whileHolding: [...m.sections.whileHolding],
        onRelease: [...m.sections.onRelease],
      };
    }
    return { onPress: [...m.steps], whileHolding: [], onRelease: [] };
  }

  const visibleSections = $derived(kind === "sequence" ? SECTIONS : SECTIONS.slice(0, 1));
  const hasSteps = $derived(SECTIONS.some((s) => sections[s.id].length > 0));

  // -- recording -----------------------------------------------------------------

  function push(section: Section, ...steps: MacroStep[]) {
    const list = [...sections[section]];
    if (list.length > 0) {
      const gap = Math.min(MAX_DELAY_MS, Math.round(performance.now() - lastEventAt));
      if (gap > 0) list.push({ step: "delay", ms: gap });
    }
    lastEventAt = performance.now();
    list.push(...steps);
    sections = { ...sections, [section]: list };
    dirty = true;
  }

  function startRecording(section: Section) {
    menuFor = null;
    recordingInto = section;
    lastEventAt = performance.now();
  }

  function stopRecording() {
    recordingInto = null;
  }

  function onKeyDown(event: KeyboardEvent) {
    if (recordingInto === null) return;
    if (event.key === "Escape") {
      event.preventDefault();
      stopRecording();
      return;
    }
    event.preventDefault();
    if (event.repeat) return;
    const modifier = modifierFor(event.code);
    if (modifier !== null) return push(recordingInto, { step: "modifiersDown", mask: modifier });
    const usage = usageFor(event.code);
    if (usage !== null) push(recordingInto, { step: "keyDown", usage });
  }

  function onKeyUp(event: KeyboardEvent) {
    if (recordingInto === null) return;
    event.preventDefault();
    const modifier = modifierFor(event.code);
    if (modifier !== null) return push(recordingInto, { step: "modifiersUp", mask: modifier });
    const usage = usageFor(event.code);
    if (usage !== null) push(recordingInto, { step: "keyUp", usage });
  }

  /** Mouse buttons are recorded too, except clicks that land on the controls. */
  function onPointerDown(event: PointerEvent) {
    if (recordingInto === null || event.pointerType !== "mouse") return;
    if ((event.target as HTMLElement).closest("button, input, a")) return;
    event.preventDefault();
    push(recordingInto, { step: "mouseDown", mask: 1 << event.button });
  }

  function onPointerUp(event: PointerEvent) {
    if (recordingInto === null || event.pointerType !== "mouse") return;
    if ((event.target as HTMLElement).closest("button, input, a")) return;
    push(recordingInto, { step: "mouseUp", mask: 1 << event.button });
  }

  function onContextMenu(event: MouseEvent) {
    if (recordingInto !== null) event.preventDefault();
  }

  // -- text & delay entries -------------------------------------------------------

  const SHIFTED: Record<string, string> = {
    "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6", "&": "7", "*": "8", "(": "9", ")": "0",
    _: "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";", '"': "'", "~": "`", "<": ",", ">": ".", "?": "/",
  };
  const PUNCT: Record<string, string> = {
    "-": "Minus", "=": "Equal", "[": "BracketLeft", "]": "BracketRight", "\\": "Backslash", ";": "Semicolon",
    "'": "Quote", "`": "Backquote", ",": "Comma", ".": "Period", "/": "Slash", " ": "Space", "\n": "Enter", "\t": "Tab",
  };

  /** ASCII text → key presses; anything the device cannot type is skipped. */
  function stepsForText(text: string): MacroStep[] {
    const out: MacroStep[] = [];
    for (const ch of text) {
      let shift = false;
      let code: string | null = null;
      if (/[a-z]/.test(ch)) code = `Key${ch.toUpperCase()}`;
      else if (/[A-Z]/.test(ch)) {
        code = `Key${ch}`;
        shift = true;
      } else if (/[0-9]/.test(ch)) code = `Digit${ch}`;
      else if (ch in SHIFTED) {
        const base = SHIFTED[ch];
        code = /[0-9]/.test(base) ? `Digit${base}` : PUNCT[base] ?? null;
        shift = true;
      } else if (ch in PUNCT) code = PUNCT[ch];
      if (!code) continue;
      const usage = usageFor(code);
      if (usage === null) continue;
      if (shift) out.push({ step: "modifiersDown", mask: 0x02 });
      out.push({ step: "keyDown", usage }, { step: "keyUp", usage });
      if (shift) out.push({ step: "modifiersUp", mask: 0x02 });
    }
    return out;
  }

  function addText(section: Section) {
    const steps = stepsForText(textDraft);
    if (steps.length) {
      sections = { ...sections, [section]: [...sections[section], ...steps] };
      dirty = true;
    }
    textFor = null;
    textDraft = "";
  }

  function addDelay(section: Section) {
    const ms = Math.max(1, Math.min(MAX_DELAY_MS, Math.round(delayDraft)));
    sections = { ...sections, [section]: [...sections[section], { step: "delay", ms }] };
    dirty = true;
    delayFor = null;
  }

  function removeStep(section: Section, index: number) {
    sections = { ...sections, [section]: sections[section].filter((_, i) => i !== index) };
    dirty = true;
  }

  // -- chips -----------------------------------------------------------------------

  interface Chip {
    label: string;
    /** "down" | "up" | "both" | "delay" */
    dir: "down" | "up" | "both" | "delay";
    /** Indexes of the underlying steps, for removal. */
    indexes: number[];
  }

  function keyLabel(s: MacroStep): string {
    switch (s.step) {
      case "keyDown":
      case "keyUp":
        return usageName(s.usage);
      case "modifiersDown":
      case "modifiersUp":
        return modifierName(s.mask);
      case "mouseDown":
      case "mouseUp":
        return `M${Math.log2(s.mask) + 1}`;
      case "delay":
        return `${s.ms} ms`;
    }
  }

  const USAGE_LABELS: Record<number, string> = {
    0x28: "ENTER", 0x29: "ESC", 0x2a: "BKSP", 0x2b: "TAB", 0x2c: "SPACE", 0x2d: "-", 0x2e: "=", 0x2f: "[",
    0x30: "]", 0x31: "\\", 0x33: ";", 0x34: "'", 0x35: "`", 0x36: ",", 0x37: ".", 0x38: "/", 0x39: "CAPS",
    0x4a: "HOME", 0x4b: "PGUP", 0x4c: "DEL", 0x4d: "END", 0x4e: "PGDN", 0x4f: "→", 0x50: "←", 0x51: "↓", 0x52: "↑",
  };

  function usageName(usage: number): string {
    if (usage >= 0x04 && usage <= 0x1d) return String.fromCharCode(65 + usage - 0x04);
    if (usage >= 0x1e && usage <= 0x26) return String(usage - 0x1e + 1);
    if (usage === 0x27) return "0";
    if (usage >= 0x3a && usage <= 0x45) return `F${usage - 0x3a + 1}`;
    return USAGE_LABELS[usage] ?? `0x${usage.toString(16)}`;
  }

  function modifierName(mask: number): string {
    const names: Record<number, string> = {
      0x01: "CTRL", 0x02: "SHIFT", 0x04: "ALT", 0x08: "SUPER", 0x10: "R CTRL", 0x20: "R SHIFT", 0x40: "R ALT", 0x80: "R SUPER",
    };
    return names[mask] ?? "MOD";
  }

  function sameKey(a: MacroStep, b: MacroStep): boolean {
    if (a.step === "keyDown" && b.step === "keyUp") return a.usage === b.usage;
    if (a.step === "modifiersDown" && b.step === "modifiersUp") return a.mask === b.mask;
    if (a.step === "mouseDown" && b.step === "mouseUp") return a.mask === b.mask;
    return false;
  }

  function isDown(s: MacroStep): boolean {
    return s.step === "keyDown" || s.step === "modifiersDown" || s.step === "mouseDown";
  }

  /** Steps as chips; a down/up pair collapses to one chip when up/down is hidden. */
  function chipsFor(steps: MacroStep[]): Chip[] {
    const out: Chip[] = [];
    for (let i = 0; i < steps.length; i++) {
      const s = steps[i];
      if (s.step === "delay") {
        if (!useStandardDelays) out.push({ label: keyLabel(s), dir: "delay", indexes: [i] });
        continue;
      }
      if (!showUpDown && isDown(s)) {
        // Look past recorded delays for the matching release.
        let j = i + 1;
        while (j < steps.length && steps[j].step === "delay") j++;
        if (j < steps.length && sameKey(s, steps[j])) {
          const indexes = [i];
          for (let k = i + 1; k <= j; k++) indexes.push(k);
          out.push({ label: keyLabel(s), dir: "both", indexes });
          i = j;
          continue;
        }
      }
      out.push({ label: keyLabel(s), dir: isDown(s) ? "down" : "up", indexes: [i] });
    }
    return out;
  }

  function removeChip(section: Section, chip: Chip) {
    const drop = new Set(chip.indexes);
    sections = { ...sections, [section]: sections[section].filter((_, i) => !drop.has(i)) };
    dirty = true;
  }

  // -- save ------------------------------------------------------------------------

  /** Flattens the sections into what the device plays. */
  function flatten(): MacroStep[] {
    const all = [...sections.onPress, ...sections.whileHolding, ...sections.onRelease];
    if (!useStandardDelays) return all;
    const out: MacroStep[] = [];
    for (const s of all) {
      if (s.step === "delay") continue;
      if (out.length > 0) out.push({ step: "delay", ms: standardDelayMs });
      out.push(s);
    }
    return out;
  }

  function save() {
    onsave({
      ...macro,
      name: name.trim() || macro.name,
      kind,
      sections: kind === "sequence" ? { ...sections } : { onPress: sections.onPress, whileHolding: [], onRelease: [] },
      useStandardDelays,
      standardDelayMs,
      color,
      steps: flatten(),
    });
    dirty = false;
  }

  function reset() {
    sections = seedSections(macro);
    dirty = false;
  }

  function nextColor() {
    const i = COLORS.indexOf(color);
    color = COLORS[(i + 1) % COLORS.length];
    dirty = true;
  }

  function back() {
    if (step === "edit" && recordingInto !== null) stopRecording();
    onclose();
  }
</script>

<svelte:window
  onkeydown={onKeyDown}
  onkeyup={onKeyUp}
  onpointerdown={onPointerDown}
  onpointerup={onPointerUp}
  oncontextmenu={onContextMenu}
/>

<div class="editor" class:recording={recordingInto !== null}>
  {#if recordingInto === null}
    <button class="back" onclick={back} aria-label="Back">
      <Icon name="arrowLeft" size={24} strokeWidth={1.8} />
    </button>
  {/if}

  {#if step === "name"}
    <form class="center" onsubmit={(e) => { e.preventDefault(); if (name.trim()) step = "type"; }}>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="big-name" type="text" placeholder="Name this macro" bind:value={name} autofocus spellcheck="false" />
      {#if name.trim()}
        <button type="submit" class="continue">Continue</button>
      {/if}
    </form>
  {:else if step === "type"}
    <div class="center types-step">
      <h2 class="name-title">{name}</h2>
      <p class="prompt">Select a type of macro you want to create</p>
      <div class="types">
        {#each KINDS as k (k.id)}
          <button class="type" class:selected={kind === k.id} onclick={() => { kind = k.id; step = "edit"; dirty = true; }}>
            <span class="type-tile"><Icon name={k.icon} size={26} strokeWidth={1.6} /></span>
            <span class="type-label">{k.label}</span>
          </button>
        {/each}
      </div>
      <p class="blurb">{KINDS.find((k) => k.id === kind)?.blurb}</p>
    </div>
  {:else}
    <header class="edit-head">
      <div class="left">
        <input class="name" type="text" bind:value={name} onchange={() => (dirty = true)} spellcheck="false" aria-label="Macro name" />
        <div class="kinds">
          <span>Macro types</span>
          {#each KINDS as k (k.id)}
            <button
              class="kind"
              class:active={kind === k.id}
              title={k.label}
              aria-label={k.label}
              disabled={recordingInto !== null}
              onclick={() => { kind = k.id; dirty = true; }}
            >
              <Icon name={k.icon} size={16} strokeWidth={1.8} />
            </button>
          {/each}
        </div>
        {#if recordingInto !== null}
          <p class="rec-status"><span class="rec-dot"></span> Recording keystrokes and mouse clicks</p>
        {/if}
      </div>

      <div class="options" class:dim={recordingInto !== null}>
        <label class="opt">
          <input type="checkbox" bind:checked={useStandardDelays} onchange={() => (dirty = true)} />
          <span class="opt-label">Use standard delays</span>
          <input
            class="ms"
            type="number"
            min="1"
            max={MAX_DELAY_MS}
            bind:value={standardDelayMs}
            onchange={() => (dirty = true)}
            disabled={!useStandardDelays}
            aria-label="Standard delay in milliseconds"
          />
          <span class="ms-unit">ms</span>
        </label>
        <p class="opt-hint">
          {useStandardDelays ? "Currently using a standard delay between actions" : "Using the delays as recorded"}
        </p>
        <label class="opt">
          <input type="checkbox" bind:checked={showUpDown} />
          <span class="opt-label">Show key down/key up</span>
        </label>
        <button class="opt color" onclick={nextColor}>
          <span class="swatch" style={color ? `background:${color}` : ""} class:none={!color}></span>
          <span class="opt-label">Macro color</span>
        </button>
      </div>
    </header>

    <div class="sections" class:single={kind !== "sequence"}>
      {#each visibleSections as sec (sec.id)}
        {@const chips = chipsFor(sections[sec.id])}
        {@const active = recordingInto === sec.id}
        {@const other = recordingInto !== null && !active}
        <section class="section" class:dim={other}>
          {#if kind === "sequence"}
            <h3>{sec.label}</h3>
          {/if}
          <div class="row">
            {#each chips as chip, i (i)}
              <button
                class="chip {chip.dir}"
                class:live={active}
                title="Remove"
                disabled={recordingInto !== null}
                onclick={() => removeChip(sec.id, chip)}
              >
                {#if chip.dir === "up"}<span class="tri up"></span>{/if}
                <span class="chip-label">{chip.label}</span>
                {#if chip.dir === "down"}<span class="tri down"></span>{/if}
              </button>
            {/each}

            {#if active}
              <button class="stop" onclick={stopRecording}>
                <span class="stop-square"></span>
                <span class="stop-label">Stop recording</span>
              </button>
            {:else if recordingInto === null}
              <div class="adder">
                <button
                  class="add"
                  class:open={menuFor === sec.id}
                  onclick={() => (menuFor = menuFor === sec.id ? null : sec.id)}
                  aria-label="Add to {sec.label}"
                >
                  <Icon name={menuFor === sec.id ? "close" : "plus"} size={20} strokeWidth={2} />
                </button>
                {#if chips.length === 0 && menuFor !== sec.id}
                  <span class="start-now">Start now</span>
                {/if}

                {#if menuFor === sec.id}
                  <div class="menu">
                    <button class="mi rec" onclick={() => startRecording(sec.id)}>
                      <span class="mi-icon"></span> Record keystrokes
                    </button>
                    <button class="mi text" onclick={() => { menuFor = null; textFor = sec.id; textDraft = ""; }}>
                      <span class="mi-icon">T</span> Text & emojis
                    </button>
                    <button class="mi action" disabled title="Runs on the host in G HUB; the device has no onboard equivalent.">
                      <span class="mi-icon">⚡</span> Action
                    </button>
                    <button class="mi launch" disabled title="Runs on the host in G HUB; the device has no onboard equivalent.">
                      <span class="mi-icon">▢</span> Launch application
                    </button>
                    <button class="mi system" disabled title="Runs on the host in G HUB; the device has no onboard equivalent.">
                      <span class="mi-icon">S</span> System
                    </button>
                    <button class="mi delay" onclick={() => { menuFor = null; delayFor = sec.id; delayDraft = 100; }}>
                      <span class="mi-icon">◷</span> Delay
                    </button>
                  </div>
                {/if}
              </div>
            {/if}
          </div>

          {#if textFor === sec.id}
            <form class="inline" onsubmit={(e) => { e.preventDefault(); addText(sec.id); }}>
              <!-- svelte-ignore a11y_autofocus -->
              <input type="text" placeholder="Type text to be entered" bind:value={textDraft} autofocus spellcheck="false" />
              <button type="submit" class="mini">Add</button>
              <button type="button" class="mini ghost" onclick={() => (textFor = null)}>Cancel</button>
              <span class="inline-hint">ASCII only — that is what the device can type.</span>
            </form>
          {/if}
          {#if delayFor === sec.id}
            <form class="inline" onsubmit={(e) => { e.preventDefault(); addDelay(sec.id); }}>
              <!-- svelte-ignore a11y_autofocus -->
              <input type="number" min="1" max={MAX_DELAY_MS} bind:value={delayDraft} autofocus />
              <span class="inline-hint">ms</span>
              <button type="submit" class="mini">Add</button>
              <button type="button" class="mini ghost" onclick={() => (delayFor = null)}>Cancel</button>
            </form>
          {/if}
        </section>
      {/each}
    </div>

    <footer class="edit-foot" class:dim={recordingInto !== null}>
      <button class="reset" onclick={reset} disabled={recordingInto !== null || !dirty}>Reset</button>
      <button class="save" onclick={save} disabled={recordingInto !== null || !dirty || !hasSteps}>Save</button>
    </footer>
  {/if}
</div>

<style>
  .editor {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    padding-top: var(--titlebar-h, 32px);
  }

  .back {
    position: absolute;
    top: calc(var(--titlebar-h, 32px) + 20px);
    left: 24px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    color: var(--text);
  }

  .back:hover {
    background: var(--surface);
  }

  /* ---- name & type steps -------------------------------------------------- */
  .center {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 28px;
  }

  .big-name {
    width: min(560px, 80vw);
    border: none;
    background: none;
    font-family: var(--font);
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-align: center;
    color: var(--text);
    user-select: text;
  }

  .big-name::placeholder {
    color: var(--text-dim);
  }

  .big-name:focus {
    outline: none;
  }

  .continue {
    padding: 10px 34px;
    border-radius: 4px;
    background: var(--cyan);
    font-family: var(--font);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #fff;
  }

  .name-title {
    font-size: 20px;
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .types-step {
    gap: 0;
  }

  .prompt {
    margin: 100px 0 46px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .types {
    display: flex;
    gap: 44px;
  }

  .type {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    width: 104px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    text-align: center;
    color: var(--text);
  }

  .type-tile {
    display: grid;
    place-items: center;
    width: 104px;
    height: 104px;
    border: 2px solid transparent;
    border-radius: 2px;
    background: #1c1d20;
  }

  .type:hover .type-tile {
    background: #26272b;
  }

  .type.selected .type-tile {
    border-color: var(--cyan);
  }

  .type.selected .type-label {
    color: var(--cyan);
  }

  .blurb {
    max-width: 260px;
    min-height: 40px;
    margin-top: 34px;
    align-self: flex-end;
    margin-right: calc(50% - 300px);
    font-size: 12px;
    line-height: 1.5;
    text-align: center;
  }

  /* ---- editor -------------------------------------------------------------- */
  .edit-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 76px 72px 0 56px;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .name {
    width: 320px;
    border: none;
    background: none;
    font-family: var(--font);
    font-size: 18px;
    font-weight: 700;
    color: var(--text);
    user-select: text;
  }

  .name:focus {
    outline: none;
    border-bottom: 1px solid var(--line-strong);
  }

  .kinds {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
  }

  .kinds span {
    margin-right: 10px;
  }

  .kind {
    display: grid;
    place-items: center;
    width: 30px;
    height: 26px;
    border-radius: 2px;
    border: 1px solid transparent;
    background: #1c1d20;
    color: var(--text);
  }

  .kind.active {
    border-color: var(--text);
  }

  .kind:disabled {
    opacity: 0.4;
  }

  .rec-status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 700;
    color: #e4002b;
  }

  .rec-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #e4002b;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: 300px;
    transition: opacity 150ms;
  }

  .dim {
    opacity: 0.3;
    pointer-events: none;
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font);
    color: var(--text);
    cursor: pointer;
  }

  .opt input[type="checkbox"] {
    width: 13px;
    height: 13px;
    accent-color: #6b6c70;
  }

  .opt-label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .ms {
    width: 52px;
    margin-left: auto;
    border: none;
    border-bottom: 1px solid var(--line-strong);
    background: none;
    font-size: 11px;
    font-weight: 700;
    text-align: right;
    color: var(--text);
    user-select: text;
  }

  .ms:disabled {
    opacity: 0.5;
  }

  .ms-unit {
    font-size: 11px;
    font-weight: 700;
  }

  .opt-hint {
    margin: -6px 0 0 23px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text);
  }

  .swatch {
    width: 18px;
    height: 18px;
    border-radius: 50%;
  }

  .swatch.none {
    background: repeating-linear-gradient(45deg, #e4002b 0 2px, transparent 2px 5px);
    border: 2px solid #e4002b;
    box-sizing: border-box;
  }

  .sections {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 44px;
    padding: 20px 56px;
    overflow-y: auto;
  }

  .sections.single {
    justify-content: center;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 18px;
    transition: opacity 150ms;
  }

  .section h3 {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    padding: 0 0 0 28px;
  }

  .chip {
    position: relative;
    display: grid;
    place-items: center;
    min-width: 34px;
    height: 34px;
    padding: 0 8px;
    border-radius: 2px;
    background: #5c1a2e;
    font-family: var(--font);
    font-size: 14px;
    font-weight: 700;
    color: #fff;
  }

  .chip.live {
    background: transparent;
    border: 1px solid #e4002b;
  }

  .chip.delay {
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    background: #2f3a6b;
    font-size: 10px;
  }

  .chip:hover:not(:disabled)::after {
    content: "×";
    position: absolute;
    top: -8px;
    right: -6px;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    font-size: 11px;
    color: #000;
  }

  .tri {
    position: absolute;
    left: 50%;
    width: 0;
    height: 0;
    border-left: 5px solid transparent;
    border-right: 5px solid transparent;
    transform: translateX(-50%);
  }

  .tri.down {
    bottom: -12px;
    border-top: 6px solid #fff;
  }

  .tri.up {
    top: -12px;
    border-bottom: 6px solid #fff;
  }

  .adder {
    position: relative;
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .add {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 2px;
    background: #3a3b3f;
    color: #fff;
  }

  .add:hover,
  .add.open {
    background: #4a4b4f;
  }

  .start-now {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .menu {
    position: absolute;
    left: 56px;
    top: 50%;
    z-index: 5;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    transform: translateY(-50%);
  }

  .mi {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    height: 30px;
    padding: 0 14px 0 8px;
    border-radius: 2px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
    color: #fff;
  }

  .mi:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .mi-icon {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    font-size: 10px;
    background: rgba(255, 255, 255, 0.25);
  }

  .mi.rec {
    background: #a4123d;
  }

  .mi.rec .mi-icon {
    background: #ff2d55;
  }

  .mi.text {
    background: #8a6f00;
  }

  .mi.text .mi-icon {
    background: #f5c400;
    color: #000;
  }

  .mi.action {
    background: #7a4a00;
  }

  .mi.action .mi-icon {
    background: #ff8a00;
  }

  .mi.launch {
    background: #3a3b3f;
  }

  .mi.system {
    background: #3a3b3f;
  }

  .mi.delay {
    background: #2f3a6b;
  }

  .mi.delay .mi-icon {
    background: #6c7bd6;
  }

  .stop {
    display: inline-flex;
    align-items: center;
    gap: 16px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #fff;
  }

  .stop-square {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 2px;
    background: #3a3b3f;
  }

  .stop-square::before {
    content: "";
    width: 16px;
    height: 16px;
    border-radius: 2px;
    background: #e4002b;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-left: 28px;
  }

  .inline input {
    width: 260px;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--line-strong);
    border-radius: 2px;
    background: #000;
    font-size: 13px;
    color: var(--text);
    user-select: text;
  }

  .inline input[type="number"] {
    width: 90px;
  }

  .inline-hint {
    font-size: 11px;
    color: var(--text-dim);
  }

  .mini {
    height: 32px;
    padding: 0 14px;
    border-radius: 2px;
    background: var(--cyan);
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #fff;
  }

  .mini.ghost {
    background: #3a3b3f;
  }

  .edit-foot {
    display: flex;
    justify-content: center;
    gap: 14px;
    padding: 0 0 36px;
    transition: opacity 150ms;
  }

  .reset,
  .save {
    width: 176px;
    height: 30px;
    border-radius: 2px;
    font-family: var(--font);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #fff;
  }

  .reset {
    background: #2a2b2f;
  }

  .save {
    background: var(--cyan);
  }

  .reset:disabled,
  .save:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .recording .name {
    opacity: 0.3;
  }
</style>
