import react from "@vitejs/plugin-react";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  resolve: { dedupe: ["react", "react-dom"] },
  test: {
    environment: "jsdom",
    setupFiles: "./src/testing/setup.ts",
    css: true,
    exclude: [...configDefaults.exclude, "storybook/**"],
  },
});
