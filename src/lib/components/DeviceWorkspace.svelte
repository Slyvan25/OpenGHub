<script lang="ts">
  /**
   * The two-pane layout G HUB uses on every device screen: a fixed-width
   * control panel on the left, and the device render filling the rest.
   */
  import type { Snippet } from "svelte";

  interface Props {
    /** Section heading, e.g. "LIGHTSYNC". */
    title: string;
    panel: Snippet;
    stage: Snippet;
    /** Optional row under the render, for stage-wide actions. */
    stageFooter?: Snippet;
    /** Optional block pinned to the stage's top-right corner (battery, legend). */
    stageAside?: Snippet;
    /** Renders sit centred; text-led stages (DPI) start from the top. */
    stageAlign?: "center" | "top";
  }

  let { title, panel, stage, stageFooter, stageAside, stageAlign = "center" }: Props = $props();
</script>

<div class="workspace">
  <aside class="panel">
    <h1>{title}</h1>
    {@render panel()}
  </aside>

  <section class="stage">
    {#if stageAside}
      <div class="stage-aside">{@render stageAside()}</div>
    {/if}
    <div class="render" class:top={stageAlign === "top"}>{@render stage()}</div>
    {#if stageFooter}
      <div class="stage-footer">{@render stageFooter()}</div>
    {/if}
  </section>
</div>

<style>
  .workspace {
    display: grid;
    grid-template-columns: 310px minmax(0, 1fr);
    gap: 0;
    flex: 1;
    min-height: 0;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 22px 16px 24px;
    background: var(--surface);
    border-radius: var(--radius-lg);
    overflow-y: auto;
  }

  /* G HUB's `.title-group`: 24px / 700 / -0.96px / 28px. */
  h1 {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.8px;
    line-height: 28px;
    margin-bottom: -4px;
  }

  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 22px;
    padding: 12px 32px 8px;
    min-width: 0;
    /* Without this the render's intrinsic size wins and the art overflows. */
    min-height: 0;
    overflow: hidden;
  }

  /* G HUB keeps the render to roughly two thirds of the stage height. */
  .render {
    flex: 0 1 64%;
    display: grid;
    place-items: center;
    width: 100%;
    min-height: 0;
    margin-top: 24px;
    overflow: hidden;
  }

  .render.top {
    flex: 1;
    place-items: start center;
    margin-top: 0;
  }

  .stage-aside {
    position: absolute;
    top: 8px;
    right: 32px;
    z-index: 2;
  }

  .stage-footer {
    flex: none;
    display: flex;
    justify-content: center;
    gap: 10px;
  }

  @media (max-width: 980px) {
    .workspace {
      grid-template-columns: 260px minmax(0, 1fr);
    }
    .stage {
      padding: 16px;
    }
  }
</style>
