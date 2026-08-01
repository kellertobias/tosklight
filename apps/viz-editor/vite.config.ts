import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/viz-editor`,
  plugins: [react()],
  build: { outDir: artifactPaths.vizEditorFrontend, emptyOutDir: true },
  resolve: { dedupe: ["react", "react-dom", "@tauri-apps/api"] },
  server: { port: 4177, strictPort: true },
  clearScreen: false,
});
