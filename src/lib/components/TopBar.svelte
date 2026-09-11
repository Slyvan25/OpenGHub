<script lang="ts">
  /** Hamburger, profile pill and account avatar — the row under the title bar. */
  import { ui } from "$lib/stores/ui.svelte";
  import Icon from "./Icon.svelte";
  import ProfilePicker from "./ProfilePicker.svelte";
</script>

<div class="topbar">
  <div class="inner">
    <button class="hamburger" onclick={() => (ui.navOpen = true)} aria-label="Open menu">
      <Icon name="menu" size={20} strokeWidth={2} />
    </button>

    <div class="right">
      <ProfilePicker />
    </div>
  </div>

  <button class="avatar" aria-label="Account">
    <Icon name="user" size={16} strokeWidth={2} />
  </button>
</div>

<style>
  .topbar {
    flex: none;
    height: var(--topbar-h);
    display: flex;
    align-items: center;
    gap: 14px;
    padding-right: 16px;
  }

  .inner {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
    max-width: var(--content-max);
    margin: 0 auto;
    padding: 0 var(--content-pad);
  }

  .hamburger {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin-left: -6px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }

  .hamburger:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .avatar {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: linear-gradient(140deg, #f0a63c, #d4741a);
    color: #26160a;
    transition: filter 120ms var(--ease);
  }

  .avatar:hover {
    filter: brightness(1.12);
  }
</style>
