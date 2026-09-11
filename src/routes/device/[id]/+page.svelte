<script lang="ts">
  /** Bare `/device/:id` redirects to whichever tab the device supports first. */
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { defaultTab } from "$lib/device-ui";
  import { deviceStore } from "$lib/stores/devices.svelte";

  $effect(() => {
    const device = deviceStore.get(page.params.id!);
    if (device) goto(`/device/${device.id}/${defaultTab(device)}`, { replaceState: true });
  });
</script>
