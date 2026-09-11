// Tauri has no Node.js server to do proper SSR, so we use adapter-static with an
// index.html fallback to put the site in SPA mode.
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
    paths: {
      // The packaged app is served from Tauri's custom protocol, where
      // root-absolute asset URLs do not resolve. Relative paths work under both
      // that and the Vite dev server.
      relative: true,
    },
  },
};

export default config;
