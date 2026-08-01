import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/patch-library-vitest`,
  plugins: [react()],
  resolve: { dedupe: ["react", "react-dom"] },
  test: {
    environment: "jsdom",
    setupFiles: "./src/testing/setup.ts",
    css: true,
  },
});
