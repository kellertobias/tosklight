import { test as base } from "@playwright/test";
import { ApiDriver } from "./api";
import { DeskDriver } from "./desk";
import { LightBench, type TestShow } from "./lightBench";

interface TestFixtures { api: ApiDriver; desk: DeskDriver; show: TestShow }
interface WorkerFixtures { bench: LightBench }

export const test = base.extend<TestFixtures, WorkerFixtures>({
  bench: [async ({}, use, workerInfo) => {
    const bench = new LightBench();
    await bench.start(workerInfo.workerIndex);
    try { await use(bench); }
    finally { await bench.stop(); }
  }, { scope: "worker" }],
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
  desk: async ({ page }, use) => { await use(new DeskDriver(page)); },
});

export { expect } from "@playwright/test";
