/**
 * User-supplied device artwork.
 *
 * No product images ship with OpenGHub — they are Logitech's copyright. The
 * backend scans a directory on the user's own machine and this store turns what
 * it finds into asset URLs the webview can load. Nothing found simply means the
 * built-in SVG drawing is used instead.
 */
import * as api from "$lib/api";
import { artworkIds } from "$lib/device-ui";
import type { Device } from "$lib/types";

class ArtworkStore {
  /** product id (`"c08d"`) → asset URL */
  images = $state<Record<string, string>>({});
  dir = $state("");
  loaded = $state(false);

  async load() {
    try {
      const [paths, dir] = await Promise.all([api.getArtwork(), api.getArtworkDir()]);
      this.dir = dir;

      const convert = await this.converter();
      const next: Record<string, string> = {};
      for (const [key, path] of Object.entries(paths)) {
        next[key] = convert(path);
      }
      this.images = next;
    } catch {
      // Artwork is decorative; a failure here must never break the dashboard.
      this.images = {};
    } finally {
      this.loaded = true;
    }
  }

  /** Outside Tauri there is no asset protocol, so the map stays empty. */
  private async converter(): Promise<(path: string) => string> {
    if (!api.isTauri) return (p) => p;
    const { convertFileSrc } = await import("@tauri-apps/api/core");
    return (p) => convertFileSrc(p);
  }

  /** The artwork for a device, or `null` to fall back to the SVG. */
  for(device: Pick<Device, "productId" | "modelIds">): string | null {
    return this.forProductIds(artworkIds(device));
  }

  /** First candidate id that has a file. */
  forProductIds(productIds: number[]): string | null {
    for (const id of productIds) {
      const hit = this.images[id.toString(16).padStart(4, "0")];
      if (hit) return hit;
    }
    return null;
  }
}

export const artwork = new ArtworkStore();
