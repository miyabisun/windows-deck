import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  // Enforce runes mode for this project's components only, not for dependencies.
  vitePlugin: {
    dynamicCompileOptions({ filename }) {
      if (!filename.includes("node_modules")) return { runes: true };
    },
  },
};
