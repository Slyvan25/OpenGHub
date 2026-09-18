// @ts-nocheck — node built-ins without @types/node
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import process from "node:process";
import fs from "node:fs";
import path from "node:path";
const host = process.env.TAURI_DEV_HOST;

/**
 * Dev-only: lets the browser mock use the artwork OpenGHub has already
 * fetched for this user (~/.local/share/openghub/devices), so screenshots
 * of the mock show real renders and layouts. Nothing is bundled — the
 * files stay on this machine and are served straight from that directory.
 */
function mockArtwork() {
  const dir =
    process.env.OPENGHUB_MOCK_ARTWORK_DIR ||
    path.join(process.env.XDG_DATA_HOME || path.join(process.env.HOME || "", ".local/share"), "openghub/devices");
  const types = { png: "image/png", webp: "image/webp", jpg: "image/jpeg", jpeg: "image/jpeg", json: "application/json" };
  return {
    name: "openghub-mock-artwork",
    configureServer(server) {
      server.middlewares.use("/__mock-artwork", (req, res, next) => {
        const name = decodeURIComponent((req.url || "/").slice(1));
        if (name === "" || name === "index.json") {
          let files = [];
          try {
            files = fs.readdirSync(dir);
          } catch {
            /* no artwork yet */
          }
          res.setHeader("content-type", "application/json");
          res.end(JSON.stringify(files));
          return;
        }
        if (name.includes("/") || name.includes("..")) return next();
        const file = path.join(dir, name);
        if (!fs.existsSync(file)) return next();
        res.setHeader("content-type", types[name.split(".").pop()] || "application/octet-stream");
        fs.createReadStream(file).pipe(res);
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit(), mockArtwork()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    // 4. Pre-transform every component in the client environment at startup.
    //    SvelteKit emits <link> tags for each component's virtual CSS module,
    //    and the browser fetches those before the component's own JS. Vite 8
    //    keeps client and SSR module graphs apart, so if the client graph has
    //    not transformed the component yet, vite-plugin-svelte finds no CSS
    //    and serves the raw .svelte source as the stylesheet — the component
    //    then renders unstyled. Warming the client graph removes the race.
    warmup: {
      clientFiles: ["./src/routes/**/*.svelte", "./src/lib/**/*.svelte"],
    },
  },
}));
