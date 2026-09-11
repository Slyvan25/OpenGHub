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
  }

  let { title, panel, stage, stageFooter }: Props = $props();
</script>

<div class="workspace">
  <aside class="panel">
    <h1>{title}</h1>
    {@render panel()}
  </aside>

  <section class="stage">
    <div class="render">{@render stage()}</div>
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
    padding: 26px 24px 28px;
    background: var(--surface);
    border-radius: var(--radius);
    overflow-y: auto;
  }

  h1 {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: 0.01em;
    margin-bottom: -2px;
  }

  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 22px;
    padding: 24px 32px;
    min-width: 0;
    /* Without this the render's intrinsic size wins and the art overflows. */
    min-height: 0;
    overflow: hidden;
  }

  .render {
    flex: 1;
    display: grid;
    place-items: center;
    width: 100%;
    min-height: 0;
    overflow: hidden;
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
