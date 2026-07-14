import { defineConfig } from "./apps/control-ui/node_modules/@playwright/test/index.js";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  workers: process.env.CI ? 4 : undefined,
  retries: process.env.CI ? 2 : 0,
  outputDir: "./test-results",
  reporter: process.env.CI
    ? [["html", { open: "never", outputFolder: "playwright-report" }], ["list"]]
    : "list",
  use: {
    browserName: "chromium",
    channel: "chrome",
    viewport: { width: 1280, height: 720 },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
