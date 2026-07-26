import react from "@vitejs/plugin-react";
import { configDefaults, defineConfig } from "vitest/config";
import { artifactPaths } from "../../tools/artifact-paths.mjs";

export default defineConfig({
  cacheDir: `${artifactPaths.viteCache}/ui-library-vitest`,
  plugins: [react()],
  resolve: { dedupe: ["react", "react-dom"] },
  test: {
    environment: "jsdom",
    setupFiles: "./src/testing/setup.ts",
    css: true,
    exclude: [...configDefaults.exclude, "storybook/**"],
  },
});
