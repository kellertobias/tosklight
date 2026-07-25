import { test as base, type PlaywrightTestArgs } from "@playwright/test";
import { ApiDriver } from "./api";
import { DeskDriver } from "./desk";
import { LightBench, type TestShow } from "./lightBench";

export interface TestFixtures { api: ApiDriver; bench: LightBench; desk: DeskDriver; show: TestShow }
export type BenchContractContext = Pick<TestFixtures, "api" | "bench" | "show"> & Pick<PlaywrightTestArgs, "request">;
export type BenchUiContext = BenchContractContext & Pick<TestFixtures, "desk"> & Pick<PlaywrightTestArgs, "page">;

export const test = base.extend<TestFixtures>({
  bench: async ({}, use, testInfo) => {
    const bench = new LightBench();
    await bench.start(testInfo.workerIndex);
    try { await use(bench); }
    finally { await bench.stop(); }
  },
  baseURL: async ({ bench }, use) => use(bench.baseUrl),
  show: [async ({ bench }, use, testInfo) => {
    const show = await bench.createTwelveDimmerShow();
    await use(show);
    if (testInfo.status !== testInfo.expectedStatus) {
      for (const [name, body] of Object.entries(await bench.failureArtifacts(show.session.token))) {
        await testInfo.attach(name, { body: Buffer.from(body), contentType: name.endsWith(".json") ? "application/json" : "text/plain" });
      }
    }
  }, { auto: true }],
  api: async ({ bench, show }, use) => {
    const api = new ApiDriver(bench.baseUrl);
    api.session = show.session;
    await use(api);
  },
  desk: async ({ page, api, bench }, use, testInfo) => {
    const driver = new DeskDriver(page, testInfo.title, api.session?.desk.id ?? null, () => bench.visualOscSummary());
    try { await use(driver); }
    finally { await driver.dispose(); }
  },
});

export { expect } from "@playwright/test";
