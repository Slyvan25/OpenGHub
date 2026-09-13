import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

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
