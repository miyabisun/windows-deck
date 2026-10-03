import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// `tauri dev` loads the page from this fixed port.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
  test: { include: ["src/**/*.test.js"], environment: "node" },
});
