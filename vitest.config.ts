import { defineConfig } from "vitest/config";
import { artifactPaths } from "./tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/root`,
  test: {
    // `.claude/worktrees` holds checkouts of other branches. Their specs are copies of these
    // ones, so running them doubles every result and reports another branch's failures as this
    // one's.
    exclude: [".artifacts/**", "node_modules/**", ".claude/**"],
  },
});
