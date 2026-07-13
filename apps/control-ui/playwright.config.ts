import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  workers: process.env.CI ? 4 : undefined,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [["html", { open: "never" }], ["list"]] : "list",
  use: { browserName: "chromium", channel: "chrome", viewport: { width: 1280, height: 720 }, trace: "retain-on-failure", screenshot: "only-on-failure" },
});
