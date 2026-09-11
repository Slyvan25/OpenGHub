// Tauri has no Node server, so the frontend is a pure SPA: no SSR, and
// adapter-static's `index.html` fallback serves every route.
export const ssr = false;
export const prerender = false;
