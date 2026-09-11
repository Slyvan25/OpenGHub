<script lang="ts">
  import { page } from "$app/state";
  import { deviceStore } from "$lib/stores/devices.svelte";
  import Assignments from "$lib/views/Assignments.svelte";
  import DeviceSettings from "$lib/views/DeviceSettings.svelte";
  import Lighting from "$lib/views/Lighting.svelte";
  import Sensitivity from "$lib/views/Sensitivity.svelte";

  const device = $derived(deviceStore.get(page.params.id!));
  const tab = $derived(page.params.tab);
</script>

{#if device}
  {#key `${device.id}:${tab}`}
    {#if tab === "sensitivity"}
      <Sensitivity {device} />
    {:else if tab === "assignments"}
      <Assignments {device} />
    {:else if tab === "lighting"}
      <Lighting {device} />
    {:else}
      <DeviceSettings {device} />
    {/if}
  {/key}
{/if}
