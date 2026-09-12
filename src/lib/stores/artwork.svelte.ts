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
import type { ArtworkLayout, Device } from "$lib/types";

class ArtworkStore {
  /** product id (`"c08d"`) → asset URL */
  images = $state<Record<string, string>>({});
  /** product id → zone rectangles and button markers from a G HUB depot */
  layouts = $state<Record<string, ArtworkLayout>>({});
  dir = $state("");
  loaded = $state(false);

  async load() {
    try {
      const [paths, dir] = await Promise.all([api.getArtwork(), api.getArtworkDir()]);
      this.dir = dir;

      const convert = await this.converter();
      const next: Record<string, string> = {};
      const layoutKeys: string[] = [];
      for (const [key, path] of Object.entries(paths)) {
        if (key.endsWith(".layout")) layoutKeys.push(key.slice(0, -".layout".length));
        else next[key] = convert(path);
      }
      this.images = next;

      // Layouts are small JSON; pull them through IPC rather than the asset protocol.
      const layouts: Record<string, ArtworkLayout> = {};
      await Promise.all(
        layoutKeys.map(async (key) => {
          const l = await api.getArtworkLayout([parseInt(key, 16)]).catch(() => null);
          if (l) layouts[key] = l;
        }),
      );
      this.layouts = layouts;
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

  /** Dashboard thumbnail if imported, else the front render. */
  thumbFor(productIds: number[]): string | null {
    for (const id of productIds) {
      const hit = this.images[`${id.toString(16).padStart(4, "0")}-thumb`];
      if (hit) return hit;
    }
    return this.forProductIds(productIds);
  }

  /** The side view, when the depot shipped one. */
  sideFor(productIds: number[]): string | null {
    for (const id of productIds) {
      const hit = this.images[`${id.toString(16).padStart(4, "0")}-side`];
      if (hit) return hit;
    }
    return null;
  }

  /** Imported layout — the exact zone and button geometry G HUB uses. */
  layoutFor(productIds: number[]): ArtworkLayout | null {
    for (const id of productIds) {
      const hit = this.layouts[id.toString(16).padStart(4, "0")];
      if (hit) return hit;
    }
    return null;
  }

  /** First candidate id that has a file. */
  forProductIds(productIds: number[]): string | null {
    for (const id of productIds) {
      const hit = this.images[id.toString(16).padStart(4, "0")];
      if (hit) return hit;
    }
    return null;
  }

  /**
   * A per-zone mask, if the user supplied one.
   *
   * This mirrors how G HUB does lighting: its device descriptors give each zone
   * a `render_icon_key` pointing at an image, never a coordinate, and the image
   * is used as a CSS mask filled with the live colour. A mask therefore gives
   * pixel-accurate lighting where a positioned blob can only approximate.
   */
  maskFor(productIds: number[], zone: number): string | null {
    for (const id of productIds) {
      const hit = this.images[`${id.toString(16).padStart(4, "0")}-zone${zone}`];
      if (hit) return hit;
    }
    return null;
  }
}

export const artwork = new ArtworkStore();
