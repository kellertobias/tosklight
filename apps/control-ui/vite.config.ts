import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 4175,
    strictPort: true,
    proxy: {
      "/api": { target: "http://127.0.0.1:5000", ws: true },
    },
  },
  preview: { port: 4175, strictPort: true },
  test: { environment: "jsdom", setupFiles: "./src/test/setup.ts", exclude: ["e2e/**", "node_modules/**"] },
});
