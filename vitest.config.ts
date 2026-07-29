import { defineConfig } from "vitest/config";
import { artifactPaths } from "./tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/root`,
  test: {
    exclude: [".artifacts/**", "node_modules/**"],
  },
});
