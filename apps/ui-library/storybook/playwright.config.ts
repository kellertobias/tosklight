import { defineConfig } from "@playwright/test";
import resolver from "../../../tools/artifact-paths.cjs";

const { artifactPaths, repositoryRoot } = resolver;
const port = 6086;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  outputDir: `${artifactPaths.results}/storybook`,
  reporter: process.env.CI
    ? [["html", { open: "never", outputFolder: `${artifactPaths.report}/storybook` }], ["list"]]
    : "list",
  webServer: {
    command: `node ${repositoryRoot}/tools/serve-pages.mjs ${artifactPaths.storybook} ${port}`,
    url: `http://127.0.0.1:${port}/index.json`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    browserName: "chromium",
    channel: "chrome",
    viewport: { width: 1496, height: 761 },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
