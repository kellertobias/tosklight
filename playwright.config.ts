import { defineConfig } from "./apps/control-ui/node_modules/@playwright/test/index.js";

const visualRecording = process.env.LIGHT_VISUAL_RECORDING === "1";
const helpScreenshots = process.env.LIGHT_HELP_SCREENSHOTS === "1";

export default defineConfig({
  testDir: "./tests",
  testIgnore: visualRecording
    ? /02-help-screenshots\.spec\.ts/
    : helpScreenshots
      ? /visual-recording\.spec\.ts/
      : [/visual-recording\.spec\.ts/, /02-help-screenshots\.spec\.ts/],
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
    launchOptions: visualRecording
      ? { slowMo: Number(process.env.LIGHT_VISUAL_SLOW_MO ?? 180) }
      : undefined,
    viewport: visualRecording ? { width: 1920, height: 1080 } : { width: 1280, height: 720 },
    video: visualRecording ? { mode: "on", size: { width: 1920, height: 1080 } } : "off",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
