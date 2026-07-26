import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/light-hardware-controls`,
  plugins: [react()],
  build: { outDir: artifactPaths.hardwareFrontend, emptyOutDir: true },
  server: { port: 4176, strictPort: true },
  clearScreen: false,
});
