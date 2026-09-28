/**
 * The key map to draw for a keyboard: from its imported G HUB depot, drawn on
 * the depot's render. Without G HUB data there is no key map; the pages that
 * need one explain how to import it.
 */
import { artworkIds } from "$lib/device-ui";
import { artwork } from "$lib/stores/artwork.svelte";
import type { Device } from "$lib/types";
import { keysFromLayout, type Key } from "./fromLayout";

export type { Key };

export interface KeyMap {
  keys: Key[];
  size: { w: number; h: number };
  /** The device render the keys sit on. */
  image: string | null;
}

export function keyMapFor(device: Device): KeyMap | null {
  const ids = artworkIds(device);
  const front = artwork.layoutFor(ids)?.views.find((v) => v.view === "front");
  const fromDepot = front ? keysFromLayout(front) : null;
  return fromDepot ? { ...fromDepot, image: artwork.forProductIds(ids) } : null;
}
