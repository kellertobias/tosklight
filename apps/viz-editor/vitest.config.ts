import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

// A CI runner renders these component trees several times slower than a workstation does, and a
// test that is comfortable locally then fails on the clock rather than on an assertion. The media
// application already draws this distinction; the Architect's suite needs it for the same reason.
const isCi = Boolean(
  (globalThis as { process?: { env?: Record<string, string | undefined> } })
    .process?.env?.CI,
);

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
    testTimeout: isCi ? 20_000 : 5_000,
  },
});
