import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [["html", { open: "never" }], ["list"]] : "list",
  use: { baseURL: "http://127.0.0.1:5011", channel: "chrome", viewport: { width: 1280, height: 720 }, trace: "retain-on-failure", screenshot: "only-on-failure" },
  webServer: {
    command: "npm run build && cargo run --manifest-path ../../Cargo.toml -p light-server -- --bind 127.0.0.1:5011 --data-dir ../../target/control-ui-e2e",
    url: "http://127.0.0.1:5011/api/v1/readiness",
    timeout: 120_000,
    reuseExistingServer: false,
  },
});
