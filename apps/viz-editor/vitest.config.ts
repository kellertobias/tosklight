import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/viz-editor-vitest`,
  plugins: [react()],
  resolve: { dedupe: ["react", "react-dom"] },
  // Dedicated Git worktrees reuse the repository's installed workspace packages. Their resolved
  // source paths therefore sit beside this worktree, which Vitest must be allowed to transform.
  server: { fs: { strict: false } },
  test: {
    environment: "jsdom",
    setupFiles: "./src/testing/setup.ts",
    css: true,
  },
});
